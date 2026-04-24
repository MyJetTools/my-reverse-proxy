use std::io;

use encryption::{aes::AesKey, AesEncryptedDataRef};

use crate::network_stream::MyOwnedReadHalf;

pub const MAX_PAYLOAD_SIZE: usize = 5 * 1024 * 1024;

const READ_CHUNK_SIZE: usize = 8192;

pub struct FrameReader {
    buffer: Vec<u8>,
}

impl FrameReader {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(READ_CHUNK_SIZE * 2),
        }
    }

    pub async fn pump_bytes(&mut self, read_half: &mut MyOwnedReadHalf) -> io::Result<usize> {
        let mut chunk = [0u8; READ_CHUNK_SIZE];
        let n = read_half.read(&mut chunk).await?;
        if n > 0 {
            self.buffer.extend_from_slice(&chunk[..n]);
        }
        Ok(n)
    }

    pub fn try_next_frame(&mut self, aes: &AesKey) -> Result<Option<Vec<u8>>, String> {
        if self.buffer.len() < 4 {
            return Ok(None);
        }

        let len = u32::from_le_bytes([
            self.buffer[0],
            self.buffer[1],
            self.buffer[2],
            self.buffer[3],
        ]) as usize;

        if len > MAX_PAYLOAD_SIZE {
            return Err(format!(
                "Frame size {} exceeds MAX_PAYLOAD_SIZE {}",
                len, MAX_PAYLOAD_SIZE
            ));
        }

        if self.buffer.len() < 4 + len {
            return Ok(None);
        }

        let encrypted = &self.buffer[4..4 + len];
        let aes_ref = AesEncryptedDataRef::new(encrypted);
        let decrypted = aes
            .decrypt(&aes_ref)
            .map_err(|_| "Decryption failed".to_string())?;
        let body = decrypted.as_slice().to_vec();

        self.buffer.drain(..4 + len);
        Ok(Some(body))
    }
}
