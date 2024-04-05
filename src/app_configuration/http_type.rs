#[derive(Debug, Clone, Copy)]
pub enum HttpType {
    Http1,
    Https1,
    Http2,
    Https2,
}

impl HttpType {
    pub fn is_http1(&self) -> bool {
        match self {
            HttpType::Http1 | HttpType::Https1 => true,
            _ => false,
        }
    }

    pub fn is_https(&self) -> bool {
        match self {
            HttpType::Https1 | HttpType::Https2 => true,
            _ => false,
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            HttpType::Http1 => "http1",
            HttpType::Https1 => "https1",
            HttpType::Http2 => "http2",
            HttpType::Https2 => "https2",
        }
    }
}
