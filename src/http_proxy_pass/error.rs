use crate::http_client::HttpClientError;

#[derive(Debug)]
pub enum ProxyPassError {
    HttpClientError(HttpClientError),
    HyperError(hyper::Error),
    IoError(tokio::io::Error),
    SshSessionError(my_ssh::SshSessionError),
    CanNotReadSettingsConfiguration(String),
    WebSocketProtocolError(hyper_tungstenite::tungstenite::error::ProtocolError),
    NoConfigurationFound,
    NoLocationFound,
    ConnectionIsDisposed,
    Unauthorized,
    IpRestricted(String),
    Timeout,
}

impl ProxyPassError {
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

impl From<std::io::Error> for ProxyPassError {
    fn from(src: std::io::Error) -> Self {
        Self::IoError(src)
    }
}

impl From<my_ssh::SshSessionError> for ProxyPassError {
    fn from(src: my_ssh::SshSessionError) -> Self {
        Self::SshSessionError(src)
    }
}

impl From<hyper_tungstenite::tungstenite::error::ProtocolError> for ProxyPassError {
    fn from(src: hyper_tungstenite::tungstenite::error::ProtocolError) -> Self {
        Self::WebSocketProtocolError(src)
    }
}
