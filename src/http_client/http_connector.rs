use my_http_client::{MyHttpClientConnector, MyHttpClientError};
use rust_extensions::StrOrString;
use tokio::net::TcpStream;

use crate::configurations::RemoteHost;

pub struct HttpConnector {
    pub remote_host: RemoteHost,
    pub debug: bool,
}

#[async_trait::async_trait]
impl MyHttpClientConnector<TcpStream> for HttpConnector {
    fn get_remote_host(&self) -> StrOrString {
        self.remote_host.as_str().into()
    }

    fn is_debug(&self) -> bool {
        self.debug
    }

    async fn connect(&self) -> Result<TcpStream, MyHttpClientError> {
        match TcpStream::connect(self.remote_host.get_host_port()).await {
            Ok(tcp_stream) => Ok(tcp_stream),
            Err(err) => Err(
                my_http_client::MyHttpClientError::CanNotConnectToRemoteHost(format!(
                    "{}. Err:{}",
                    self.remote_host.as_str(),
                    err
                )),
            ),
        }
    }
}
