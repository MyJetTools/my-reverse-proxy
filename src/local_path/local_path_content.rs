use rust_extensions::{file_utils::FilePath, SliceOrVec};

use crate::{h1_utils::Http1HeadersBuilder, network_stream::NetworkStreamReadPart};

pub struct LocalPathContent {
    files_path: FilePath,
    default_file: Option<String>,
    sender: tokio::sync::mpsc::Sender<SliceOrVec<'static, u8>>,
    receiver: Option<tokio::sync::mpsc::Receiver<SliceOrVec<'static, u8>>>,
}

impl LocalPathContent {
    pub fn new(files_path: &str, default_file: Option<String>) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(64);
        Self {
            files_path: FilePath::from_str(files_path),
            default_file,
            sender,
            receiver: Some(receiver),
        }
    }
    pub async fn send_headers(&self, h1_headers: &Http1HeadersBuilder) {
        let first_line = h1_headers.get_first_line();

        let (verb, path) = first_line.get_verb_and_path();

        if verb != "GET" {
            self.sender
                .send(crate::error_templates::NOT_FOUND.as_slice().into())
                .await
                .unwrap();
        }

        super::serve_file::serve_file(
            &self.files_path,
            path,
            self.default_file.as_deref(),
            &self.sender,
        )
        .await;
    }

    pub fn get_read_path(&mut self) -> LocalPathContentReader {
        LocalPathContentReader {
            receiver: self.receiver.take().unwrap(),
            temp_buffer: Default::default(),
        }
    }
}

pub struct LocalPathContentReader {
    receiver: tokio::sync::mpsc::Receiver<SliceOrVec<'static, u8>>,
    temp_buffer: Vec<u8>,
}

#[async_trait::async_trait]
impl NetworkStreamReadPart for LocalPathContentReader {
    async fn read_from_socket(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        if self.temp_buffer.len() == 0 {
            let Some(received) = self.receiver.recv().await else {
                return Ok(0);
            };

            let received_len = received.get_len();

            if received_len < buf.len() {
                buf[..received_len].copy_from_slice(received.as_slice());
                return Ok(received_len);
            }

            buf.copy_from_slice(&received.as_slice()[..buf.len()]);
            self.temp_buffer
                .extend_from_slice(&received.as_slice()[buf.len()..]);

            return Ok(buf.len());
        }

        if self.temp_buffer.len() < buf.len() {
            buf[..self.temp_buffer.len()].copy_from_slice(&self.temp_buffer);
            self.temp_buffer.clear();
            return Ok(self.temp_buffer.len());
        }

        let to_copy = self.temp_buffer.drain(..buf.len());
        buf.copy_from_slice(to_copy.as_slice());

        Ok(buf.len())
    }
}
