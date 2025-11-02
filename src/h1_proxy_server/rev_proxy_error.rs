#![allow(warnings)]
use my_ssh::ssh2::DisconnectCode::ProtocolError;

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

impl ProxyServerError {
    pub fn can_be_printed_as_debug(&self) -> bool {
        match self {
            Self::HttpResponse(_) => {
                return false;
            }
            _ => {
                return true;
            }
        }
    }
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
