use std::{sync::Arc, time::Duration};

use encryption::aes::AesKey;
use rust_extensions::UnsafeValue;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf, sync::Mutex};

use super::SendBuffer;

const SEND_TIMEOUT: Duration = Duration::from_secs(30);

pub struct TcpConnectionInner {
    pub connection: Mutex<Option<OwnedWriteHalf>>,
    pub buffer: Mutex<SendBuffer>,
    sender: Arc<tokio::sync::mpsc::Sender<()>>,
    is_connected: Box<UnsafeValue<bool>>,
    pub aes_key: Arc<AesKey>,
}

impl TcpConnectionInner {
    pub fn new(
        connection: OwnedWriteHalf,
        aes_key: Arc<AesKey>,
    ) -> (Self, tokio::sync::mpsc::Receiver<()>) {
        let (sender, receiver) = tokio::sync::mpsc::channel(1024);
        let result = Self {
            connection: Mutex::new(Some(connection)),
            buffer: Mutex::new(SendBuffer::new()),
            sender: Arc::new(sender),
            is_connected: Box::new(true.into()),
            aes_key,
        };

        (result, receiver)
    }

    pub async fn send_payload(&self, payload: &[u8]) -> bool {
        {
            let mut buffer_access = self.buffer.lock().await;

            if buffer_access.disconnected {
                return false;
            }

            buffer_access.push(payload);
        }

        let sender = self.sender.clone();

        tokio::spawn(async move {
            let _ = sender.send(()).await;
        });

        true
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.get_value()
    }

    pub async fn disconnect(&self) -> bool {
        self.is_connected.set_value(false);

        let connection = {
            let mut write_access = self.connection.lock().await;
            write_access.take()
        };

        if connection.is_some() {
            let mut buffer_access = self.buffer.lock().await;
            buffer_access.disconnect();
        }
        connection.is_some()
    }

    pub async fn push_payload_to_socket(&self) -> bool {
        loop {
            let payload_to_send = {
                let mut write_access = self.buffer.lock().await;

                if write_access.disconnected {
                    return false;
                }

                write_access.get_payload_to_send()
            };

            if payload_to_send.is_none() {
                return true;
            }

            let payload_to_send = payload_to_send.unwrap();

            let mut connection_access = self.connection.lock().await;
            if let Some(connection) = &mut *connection_access {
                let write_future = connection.write_all(&payload_to_send);

                let write_result = tokio::time::timeout(SEND_TIMEOUT, write_future).await;

                let Ok(write_result) = write_result else {
                    println!(
                        "Timeout sending payload to socket with size {}",
                        payload_to_send.len()
                    );
                    return false;
                };

                if write_result.is_err() {
                    return false;
                }
            }
        }
    }

    pub async fn flush(&self) -> Result<(), std::io::Error> {
        let mut connection_access = self.connection.lock().await;
        if let Some(connection) = &mut *connection_access {
            return connection.flush().await;
        }

        Ok(())
    }
}
