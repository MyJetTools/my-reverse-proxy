use std::time::Duration;

#[derive(Debug)]
pub enum MyHttpClientError {
    CanNotConnectToRemoteHost(String),
    UpgradedToWebSocket,
    Disconnected,
    Disposed,
    RequestTimeout(Duration),
    CanNotExecuteRequest(String),
    InvalidHttpHandshake(String),
    HyperWebsocket(hyper_tungstenite::HyperWebsocket),
}

impl MyHttpClientError {
    pub fn is_web_socket_upgraded(&self) -> bool {
        matches!(self, MyHttpClientError::UpgradedToWebSocket)
    }

    pub fn is_retirable(&self) -> bool {
        matches!(self, MyHttpClientError::Disconnected)
    }
}
