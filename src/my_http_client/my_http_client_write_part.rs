use bytes::Bytes;

use http_body_util::{BodyExt, Full};

use rust_extensions::UnsafeValue;
use std::{fmt::Write, sync::Arc};
use tokio::{
    io::{AsyncWriteExt, WriteHalf},
    sync::Mutex,
};

use crate::{http_client::HTTP_CLIENT_TIMEOUT, http_proxy_pass::HostPort};

pub struct MyHttpClientWritePart<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite> {
    connected: UnsafeValue<bool>,
    writer: Mutex<Option<WriteHalf<TStream>>>,
    to_send: Mutex<Option<Vec<u8>>>,
    write_signal: tokio::sync::mpsc::Sender<()>,
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite> MyHttpClientWritePart<TStream> {
    pub fn new(writer: WriteHalf<TStream>, write_signal: tokio::sync::mpsc::Sender<()>) -> Self {
        Self {
            writer: Mutex::new(Some(writer)),
            to_send: Mutex::new(None),
            connected: UnsafeValue::new(true),
            write_signal,
        }
    }

    /*
       pub async fn is_connected(&self) -> bool {
           self.connected.get_value()
       }
    */
    pub async fn send(&self, req: hyper::Request<Full<Bytes>>) -> bool {
        if !self.connected.get_value() {
            self.to_send.lock().await.take();
            return false;
        }

        let mut serialized = String::new();

        write!(
            &mut serialized,
            "{} {} {:?}\r\n",
            req.method(),
            req.uri()
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or("/"),
            req.version()
        )
        .unwrap();

        let (parts, body) = req.into_parts();

        for (name, value) in parts.get_headers() {
            write!(&mut serialized, "{}: {}\r\n", name, value.to_str().unwrap()).unwrap();
        }

        // End headers section
        serialized.push_str("\r\n");

        let body_as_bytes = body.collect().await.unwrap().to_bytes();

        let mut to_send = self.to_send.lock().await;

        match to_send.as_mut() {
            Some(vec) => {
                vec.extend_from_slice(serialized.as_bytes());
                vec.extend_from_slice(&body_as_bytes);
            }
            None => {
                let mut vec = serialized.into_bytes();
                vec.extend_from_slice(&body_as_bytes);
                *to_send = Some(vec);
            }
        }

        let _ = self.write_signal.send(()).await;

        true
    }

    async fn get_payload_to_send(&self) -> Option<Vec<u8>> {
        self.to_send.lock().await.take()
    }

    pub async fn get_write_part(&self) -> Option<WriteHalf<TStream>> {
        self.connected.set_value(false);
        self.writer.lock().await.take()
    }

    async fn send_to_socket(&self, to_send: Vec<u8>) -> bool {
        let mut writer = self.writer.lock().await;
        if writer.is_none() {
            return false;
        }

        let writer = writer.as_mut().unwrap();

        for chunk in to_send.chunks(1024 * 1024) {
            let future = writer.write_all(chunk);

            let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

            if result.is_err() {
                return false;
            }

            let result = result.unwrap();

            if result.is_err() {
                return false;
            }
        }

        true
    }

    pub async fn flush(&self) {
        if let Some(to_send) = self.get_payload_to_send().await {
            if !self.send_to_socket(to_send).await {
                self.connected.set_value(false);
            }
        }
    }
}

pub async fn write_loop<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite>(
    write_part: Arc<MyHttpClientWritePart<TStream>>,
    mut receiver: tokio::sync::mpsc::Receiver<()>,
) {
    while let Some(_) = receiver.recv().await {
        write_part.flush().await;
    }
}
