#![allow(warnings)]
use std::time::Duration;

#[derive(Debug)]
pub enum NetworkError {
    Timeout(Duration),
    Disconnected,
    IoError(std::io::Error),
    OtherStr(&'static str),
    Other(String),
    MyHttpClientError(my_http_client::MyHttpClientError),
}

impl NetworkError {
    pub fn as_timeout(&self) -> Option<Duration> {
        match self {
            NetworkError::Timeout(duration) => Some(*duration),
            _ => None,
        }
    }
}

impl From<std::io::Error> for NetworkError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<my_http_client::MyHttpClientError> for NetworkError {
    fn from(value: my_http_client::MyHttpClientError) -> Self {
        Self::MyHttpClientError(value)
    }
}
