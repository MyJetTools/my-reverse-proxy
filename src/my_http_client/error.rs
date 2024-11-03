#[derive(Debug)]
pub enum MyHttpClientError {
    CanNotConnectToRemoteHost(String),
    UpgradedToWebSocket,
    Disconnected,
    Disposed,
}

impl MyHttpClientError {
    pub fn is_disconnected(&self) -> bool {
        match self {
            MyHttpClientError::Disconnected => true,
            _ => false,
        }
    }

    pub fn is_web_socket_upgraded(&self) -> bool {
        match self {
            MyHttpClientError::UpgradedToWebSocket => true,
            _ => false,
        }
    }
}
