pub struct SendBuffer {
    pub buffer: Option<Vec<u8>>,
    pub disconnected: bool,
}

impl SendBuffer {
    pub fn new() -> Self {
        Self {
            buffer: None,
            disconnected: false,
        }
    }

    pub fn push(&mut self, payload: &[u8]) {
        if let Some(buffer) = &mut self.buffer {
            buffer.extend_from_slice(payload);
        } else {
            self.buffer = Some(payload.to_vec());
        }
    }

    pub fn get_payload_to_send(&mut self, max_size: usize) -> Option<Vec<u8>> {
        let buffer = match self.buffer.take() {
            Some(buffer) => buffer,
            None => return None,
        };

        if buffer.len() <= max_size {
            return Some(buffer);
        }

        let to_send = buffer.as_slice()[..max_size].to_vec();

        self.buffer = Some(buffer.as_slice()[max_size..].to_vec());

        Some(to_send)
    }

    pub fn disconnect(&mut self) {
        self.disconnected = true;
        self.buffer.take();
    }
}
