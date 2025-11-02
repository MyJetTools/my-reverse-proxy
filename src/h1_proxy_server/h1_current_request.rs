#[derive(Debug)]
pub struct H1CurrentRequest {
    pub connection_id: u64,
    pub buffer: Vec<u8>,
    pub done: bool,
}

impl H1CurrentRequest {
    pub fn new(connection_id: u64) -> Self {
        Self {
            connection_id,
            buffer: vec![],
            done: false,
        }
    }
}
