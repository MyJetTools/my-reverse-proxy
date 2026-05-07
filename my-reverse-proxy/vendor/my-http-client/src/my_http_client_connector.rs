use rust_extensions::remote_endpoint::RemoteEndpoint;
use tokio::io::{ReadHalf, WriteHalf};

use crate::MyHttpClientError;

#[async_trait::async_trait]
pub trait MyHttpClientConnector<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite> {
    async fn connect(&self) -> Result<TStream, MyHttpClientError>;
    fn get_remote_endpoint(&self) -> RemoteEndpoint<'_>;
    fn is_debug(&self) -> bool;

    fn reunite(read: ReadHalf<TStream>, write: WriteHalf<TStream>) -> TStream;
}
