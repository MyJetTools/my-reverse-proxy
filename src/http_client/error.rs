#[derive(Debug)]
pub enum HttpClientError {
    InvalidHttp1HandShake(String),
    CanNotEstablishConnection(String),
    HyperError(hyper::Error),
    IoError(std::io::Error),
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
