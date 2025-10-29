use crate::h1_proxy_server::ProxyServerError;

const BUFFER_CAPACITY: usize = 1024 * 1024;
pub struct LoopBuffer {
    data: Vec<u8>,
    read_from: usize,
    read_to: usize,
}

impl LoopBuffer {
    pub fn new() -> Self {
        let mut result = Self {
            data: Vec::with_capacity(BUFFER_CAPACITY),
            read_from: 0,
            read_to: 0,
        };

        unsafe {
            result.data.set_len(BUFFER_CAPACITY);
        }

        result
    }

    pub fn advance(&mut self, size: usize) {
        self.read_to += size;
    }

    fn gc(&mut self) {
        if self.read_from == self.read_to && self.read_from > 0 {
            self.read_from = 0;
            self.read_to = 0;
            return;
        }

        if self.read_from == 0 {
            return;
        }

        let mut to = 0;
        for i in self.read_from..self.read_to {
            let b = self.data[i];
            self.data[to] = b;
            to += 1;
        }
    }

    pub fn get_mut(&mut self) -> Result<&mut [u8], ProxyServerError> {
        self.gc();

        if self.read_to >= self.data.len() {
            return Err(ProxyServerError::BufferAllocationFail);
        }

        Ok(&mut self.data[self.read_to..])
    }

    pub fn get_data(&self) -> &[u8] {
        &self.data[self.read_from..self.read_to]
    }

    pub fn commit_read(&mut self, size: usize) {
        self.read_from += size;
    }
}
