use crate::http_client::HttpClientError;

#[derive(Debug)]
pub enum ProxyPassError {
    HttpClientError(HttpClientError),
    HyperError(hyper::Error),
    SshSessionError(my_ssh::SshSessionError),
    NoConfigurationFound,
    NoLocationFound,
    ConnectionIsDisposed,
    Timeout,
}

impl ProxyPassError {
    pub fn is_timeout(&self) -> bool {
        match self {
            ProxyPassError::HttpClientError(src) => src.is_timeout(),
            ProxyPassError::Timeout => true,
            _ => false,
        }
    }
    pub fn is_disposed(&self) -> bool {
        match self {
            ProxyPassError::ConnectionIsDisposed => true,
            _ => false,
        }
    }
}

impl From<HttpClientError> for ProxyPassError {
    fn from(src: HttpClientError) -> Self {
        Self::HttpClientError(src)
    }
}

impl From<hyper::Error> for ProxyPassError {
    fn from(src: hyper::Error) -> Self {
        Self::HyperError(src)
    }
}

impl From<my_ssh::SshSessionError> for ProxyPassError {
    fn from(src: my_ssh::SshSessionError) -> Self {
        Self::SshSessionError(src)
    }
}
