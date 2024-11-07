#![allow(unused)]

use std::future::Future;

use futures::future::IntoFuture;
use hyper::header::PRAGMA;

use crate::http_client::HttpClientError;

#[derive(Debug)]
pub enum ProxyPassError {
    HttpClientError(HttpClientError),
    HyperError(hyper::Error),
    IoError(tokio::io::Error),
    SshSessionError(my_ssh::SshSessionError),
    WebSocketProtocolError(hyper_tungstenite::tungstenite::error::ProtocolError),
    MyHttpClientError(my_http_client::MyHttpClientError),
    NoLocationFound,
    ConnectionIsDisposed,
    Unauthorized,
    UserIsForbidden,
    IpRestricted(String),
    Disconnected,
    Timeout,
    Disposed,
}

impl ProxyPassError {
    pub fn is_disposed(&self) -> bool {
        match self {
            ProxyPassError::ConnectionIsDisposed => true,
            _ => false,
        }
    }

    pub fn is_hyper_canceled(&self) -> bool {
        match self {
            ProxyPassError::HyperError(err) => err.is_canceled(),
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

impl From<my_http_client::MyHttpClientError> for ProxyPassError {
    fn from(src: my_http_client::MyHttpClientError) -> Self {
        Self::MyHttpClientError(src)
    }
}

#[derive(Debug)]
pub enum ExecuteWithTimeoutError {
    ReconnectAndRetry,
    ProxyPassError(ProxyPassError),
}

impl From<ProxyPassError> for ExecuteWithTimeoutError {
    fn from(src: ProxyPassError) -> Self {
        Self::ProxyPassError(src)
    }
}

impl From<hyper::Error> for ExecuteWithTimeoutError {
    fn from(src: hyper::Error) -> Self {
        Self::ProxyPassError(src.into())
    }
}
