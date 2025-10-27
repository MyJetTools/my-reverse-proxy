use crate::network_stream::NetworkError;

#[derive(Debug)]
pub enum ProxyServerError {
    NetworkError(NetworkError),
    ParsingPayloadError(&'static str),
    BufferAllocationFail,
    ChunkHeaderParseError,
    HeadersParseError(&'static str),
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
