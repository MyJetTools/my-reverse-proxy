#![allow(warnings)]
use crate::{google_auth::GoogleAuthError, network_stream::NetworkError};

#[derive(Debug)]
pub enum ProxyServerError {
    NetworkError(NetworkError),
    ParsingPayloadError(&'static str),
    BufferAllocationFail,
    ChunkHeaderParseError,
    HeadersParseError(&'static str),
    CanNotConnectToRemoteResource(NetworkError),
    CanNotWriteContentToRemoteConnection(NetworkError),
    HttpConfigurationIsNotFound,
    LocationIsNotFound,
    NotAuthorized,
    HttpResponse(Vec<u8>),
}

impl From<NetworkError> for ProxyServerError {
    fn from(value: NetworkError) -> Self {
        Self::NetworkError(value)
    }
}

impl From<&'static str> for ProxyServerError {
    fn from(value: &'static str) -> Self {
        Self::ParsingPayloadError(value)
    }
}
