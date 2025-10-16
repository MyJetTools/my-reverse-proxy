use std::sync::Arc;

use my_ssh::{SshAsyncChannel, SshSession};
use rust_extensions::remote_endpoint::*;
use tokio::io::{ReadHalf, WriteHalf};

use my_http_client::{MyHttpClientConnector, MyHttpClientError};

pub struct HttpOverSshConnector {
    pub ssh_session: Arc<SshSession>,
    pub remote_endpoint: RemoteEndpointOwned,
    pub debug: bool,
    pub connect_timeout: std::time::Duration,
}

#[async_trait::async_trait]
impl MyHttpClientConnector<SshAsyncChannel> for HttpOverSshConnector {
    fn is_debug(&self) -> bool {
        self.debug
    }

    fn get_remote_endpoint<'s>(&'s self) -> RemoteEndpoint<'s> {
        self.remote_endpoint.to_ref()
    }

    fn reunite(
        read: ReadHalf<SshAsyncChannel>,
        write: WriteHalf<SshAsyncChannel>,
    ) -> SshAsyncChannel {
        read.unsplit(write)
    }
    async fn connect(&self) -> Result<SshAsyncChannel, MyHttpClientError> {
        let remote_host = self.remote_endpoint.get_host();

        let remote_port = self.remote_endpoint.get_port().unwrap_or(80);

        let tcp_stream = self
            .ssh_session
            .connect_to_remote_host(remote_host, remote_port, self.connect_timeout)
            .await;

        let tcp_stream = match tcp_stream {
            Ok(tcp_stream) => tcp_stream,
            Err(err) => {
                return Err(MyHttpClientError::CanNotConnectToRemoteHost(format!(
                    "{}. Err: {:?}",
                    self.get_remote_endpoint().as_str(),
                    err
                )))
            }
        };

        Ok(tcp_stream)

        /*

        let http_client = MyHttpClient::new(tcp_stream);

        let result = Self {
            http_client,
            _connected: DateTimeAsMicroseconds::now(),
            _ssh_credentials: ssh_credentials.clone(),
            _ssh_session: Arc::new(ssh_session),
        };

        Ok(result)
         */
    }
}
