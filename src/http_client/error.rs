#[derive(Debug)]
pub enum HttpClientError {
    InvalidHttp1HandShake(String),
    CanNotEstablishConnection(String),
    HyperError(hyper::Error),
    IoError(std::io::Error),
    TimeOut,
}

impl HttpClientError {
    pub fn is_timeout(&self) -> bool {
        match self {
            HttpClientError::TimeOut => true,
            _ => false,
        }
    }
}

impl From<hyper::Error> for HttpClientError {
    fn from(src: hyper::Error) -> Self {
        Self::HyperError(src)
    }
}

impl From<std::io::Error> for HttpClientError {
    fn from(src: std::io::Error) -> Self {
        Self::IoError(src)
    }
}
