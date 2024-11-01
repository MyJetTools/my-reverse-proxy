pub enum DetectedBodySize {
    Unknown,
    Known(usize),
    Chunked,
    WebSocketUpgrade,
}

impl DetectedBodySize {
    pub fn is_unknown(&self) -> bool {
        match self {
            DetectedBodySize::Unknown => true,
            _ => false,
        }
    }
}
