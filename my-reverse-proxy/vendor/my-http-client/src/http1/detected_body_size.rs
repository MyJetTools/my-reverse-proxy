pub enum DetectedBodySize {
    Unknown,
    Known(usize),
    Chunked,
    WebSocketUpgrade,
}

impl DetectedBodySize {
    pub fn is_unknown(&self) -> bool {
        matches!(self, DetectedBodySize::Unknown)
    }
}
