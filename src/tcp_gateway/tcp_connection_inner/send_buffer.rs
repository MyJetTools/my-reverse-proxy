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

    pub fn get_payload_to_send(&mut self) -> Option<Vec<u8>> {
        self.buffer.take()
    }

    pub fn disconnect(&mut self) {
        self.disconnected = true;
        self.buffer.take();
    }
}
