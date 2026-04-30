use super::HttpParseError;

const CRLF: &[u8] = b"\r\n";

const BUFFER_SIZE: usize = 1024 * 512;
pub struct TcpBuffer {
    buffer: Vec<u8>,
    pub read_pos: usize,
    pub consumed_pos: usize,
}

impl Default for TcpBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl TcpBuffer {
    pub fn new() -> Self {
        Self {
            buffer: vec![0u8; BUFFER_SIZE],
            read_pos: 0,
            consumed_pos: 0,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.read_pos == self.consumed_pos
    }

    fn compact(&mut self) {
        const TEMP_BUFFER_SIZE: usize = 1024;
        let mut buffer_to_move = [0u8; TEMP_BUFFER_SIZE];

        let mut pos = self.consumed_pos;
        let mut dest_pos = 0;

        let mut remains_to_move = self.read_pos - pos;

        let size = remains_to_move;

        while remains_to_move >= TEMP_BUFFER_SIZE {
            let buffer_to_copy = &self.buffer[pos..pos + TEMP_BUFFER_SIZE];
            buffer_to_move.copy_from_slice(buffer_to_copy);

            self.buffer[dest_pos..dest_pos + TEMP_BUFFER_SIZE].copy_from_slice(&buffer_to_move);

            pos += TEMP_BUFFER_SIZE;
            dest_pos += TEMP_BUFFER_SIZE;
            remains_to_move -= TEMP_BUFFER_SIZE;
        }

        if remains_to_move > 0 {
            let buffer_to_copy = &self.buffer[pos..pos + remains_to_move];
            buffer_to_move[..remains_to_move].copy_from_slice(buffer_to_copy);

            self.buffer[dest_pos..dest_pos + remains_to_move]
                .copy_from_slice(&buffer_to_move[..remains_to_move]);
        }

        self.consumed_pos = 0;
        self.read_pos = size;
    }

    pub fn get_total_buffer_size(&self) -> usize {
        self.buffer.len()
    }
    pub fn get_write_buf(&mut self) -> Option<&mut [u8]> {
        if self.consumed_pos == 0 {
            if self.read_pos == self.buffer.len() {
                return None;
            }
            return Some(&mut self.buffer[self.read_pos..]);
        }

        if self.consumed_pos < self.read_pos {
            self.compact();
        } else {
            self.read_pos = 0;
            self.consumed_pos = 0;
        }

        Some(&mut self.buffer[self.read_pos..])
    }

    pub fn add_read_amount(&mut self, pos: usize) {
        self.read_pos += pos;
    }

    pub fn read_until_crlf(&mut self) -> Option<&[u8]> {
        let mut pos = self.consumed_pos;

        while pos < self.read_pos - 1 {
            if &self.buffer[pos..pos + 2] == CRLF {
                let result = &self.buffer[self.consumed_pos..pos];
                self.consumed_pos = pos + 2;
                return Some(result);
            }

            pos += 1;
        }

        None
    }

    pub fn skip_exactly(&mut self, size_to_skip: usize) -> Result<(), HttpParseError> {
        if self.consumed_pos + size_to_skip > self.read_pos {
            return Err(HttpParseError::GetMoreData);
        }

        self.consumed_pos += size_to_skip;
        Ok(())
    }

    pub fn get_as_much_as_possible(&mut self, max_size: usize) -> Option<&[u8]> {
        if self.read_pos == self.consumed_pos {
            return None;
        }

        let has_amount = self.read_pos - self.consumed_pos;

        let result = if has_amount >= max_size {
            &self.buffer[self.consumed_pos..self.consumed_pos + max_size]
        } else {
            &self.buffer[self.consumed_pos..self.consumed_pos + has_amount]
        };

        self.consumed_pos += result.len();

        Some(result)
    }

    pub fn get_buf(&self) -> &[u8] {
        &self.buffer[self.consumed_pos..self.read_pos]
    }
}
