pub struct H1CurrentRequest {
    pub request_id: u64,
    pub buffer: Vec<u8>,
    pub done: bool,
}

impl H1CurrentRequest {
    pub fn new(request_id: u64) -> Self {
        Self {
            request_id,
            buffer: vec![],
            done: false,
        }
    }
}
