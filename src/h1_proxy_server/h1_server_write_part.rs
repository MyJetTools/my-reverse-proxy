use std::{sync::Arc, time::Duration};

use tokio::sync::Mutex;

use super::*;
use crate::network_stream::*;

pub struct H1ServerWritePartInner<WritePart: NetworkStreamWritePart + Send + Sync + 'static> {
    pub server_write_part: Option<WritePart>,
    pub current_requests: Vec<H1CurrentRequest>,
}

pub struct H1ServerWritePart<WritePart: NetworkStreamWritePart + Send + Sync + 'static> {
    inner: Arc<Mutex<H1ServerWritePartInner<WritePart>>>,
}

impl<WritePart: NetworkStreamWritePart + Send + Sync + 'static> H1ServerWritePart<WritePart> {
    pub fn new(server_write_part: WritePart) -> Self {
        let inner = H1ServerWritePartInner {
            server_write_part: Some(server_write_part),
            current_requests: vec![],
        };

        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
    pub async fn into_write_part(self) -> WritePart {
        let mut write_access = self.inner.lock().await;
        write_access.server_write_part.take().unwrap()
    }

    pub async fn add_current_request(&self, request_id: u64) {
        let mut write_access = self.inner.lock().await;
        write_access
            .current_requests
            .push(H1CurrentRequest::new(request_id));
    }

    pub async fn request_is_done(&self, request_id: u64) {
        let mut write_access = self.inner.lock().await;

        for itm in write_access.current_requests.iter_mut() {
            if itm.request_id == request_id {
                itm.done = true;
                break;
            }
        }

        loop {
            let done = match write_access.current_requests.get(0) {
                Some(itm) => itm.done,
                None => {
                    break;
                }
            };

            if done {
                let done_item = write_access.current_requests.remove(0);

                if done_item.buffer.len() > 0 {
                    write_access
                        .server_write_part
                        .as_mut()
                        .unwrap()
                        .write_all_with_timeout(&done_item.buffer, crate::consts::WRITE_TIMEOUT)
                        .await
                        .unwrap();
                }
            }
        }

        //println!("Requests: {}", write_access.current_requests.len());
    }

    pub async fn write_http_payload_with_timeout(
        &self,
        request_id: u64,
        buffer: &[u8],
        timeout: Duration,
    ) -> Result<(), NetworkError> {
        let mut write_access = self.inner.lock().await;

        if write_access.current_requests.len() == 0 {
            if let Some(write_part) = write_access.server_write_part.as_mut() {
                return write_part.write_all_with_timeout(buffer, timeout).await;
            }
            return Err(NetworkError::Disconnected);
        }

        if write_access.current_requests.get(0).unwrap().request_id == request_id {
            if let Some(write_part) = write_access.server_write_part.as_mut() {
                return write_part.write_all_with_timeout(buffer, timeout).await;
            }
            return Err(NetworkError::Disconnected);
        }

        for itm in write_access.current_requests.iter_mut() {
            if itm.request_id == request_id {
                itm.buffer.extend_from_slice(buffer);
                return;
            }
        }

        println!("Somehow nowhere to write");

        Ok(())
    }
}

#[async_trait::async_trait]
impl<WritePart: NetworkStreamWritePart + Send + Sync + 'static> H1Writer
    for H1ServerWritePart<WritePart>
{
    async fn write_http_payload(
        &mut self,
        request_id: u64,
        buffer: &[u8],
        timeout: Duration,
    ) -> Result<(), NetworkError> {
        self.write_http_payload_with_timeout(request_id, buffer, timeout)
            .await
    }
}

#[async_trait::async_trait]
impl<WritePart: NetworkStreamWritePart + Send + Sync + 'static> NetworkStreamWritePart
    for H1ServerWritePart<WritePart>
{
    async fn shutdown_socket(&mut self) {
        let mut write_access = self.inner.lock().await;
        if let Some(inner) = write_access.server_write_part.as_mut() {
            inner.shutdown_socket().await;
        }
    }

    async fn write_to_socket(&mut self, _buffer: &[u8]) -> Result<(), std::io::Error> {
        panic!("Should not be used. Instead  write_http_payload should be used");
    }
}
