use std::sync::Arc;

use my_ssh::{SshAsyncChannel, SshCredentials, SshSessionsPool};
use rust_extensions::StrOrString;
use tokio::io::{ReadHalf, WriteHalf};

use crate::configurations::RemoteHost;

use my_http_client::{MyHttpClientConnector, MyHttpClientError};

pub struct SshConnector {
    pub ssh_credentials: Arc<SshCredentials>,
    pub pool: Arc<SshSessionsPool>,
    pub remote_host: RemoteHost,
    pub debug: bool,
}

#[async_trait::async_trait]
impl MyHttpClientConnector<SshAsyncChannel> for SshConnector {
    fn is_debug(&self) -> bool {
        self.debug
    }

    fn get_remote_host(&self) -> StrOrString {
        format!(
            "ssh:{}@{}->{}",
            self.ssh_credentials.get_user_name(),
            self.ssh_credentials.get_host_port_as_string(),
            self.remote_host.as_str()
        )
        .into()
    }

    fn reunite(
        read: ReadHalf<SshAsyncChannel>,
        write: WriteHalf<SshAsyncChannel>,
    ) -> SshAsyncChannel {
        read.unsplit(write)
    }
    async fn connect(&self) -> Result<SshAsyncChannel, MyHttpClientError> {
        let ssh_session = self.pool.get_or_create(&self.ssh_credentials).await;
        let tcp_stream = ssh_session
            .connect_to_remote_host(
                self.remote_host.get_host(),
                self.remote_host.get_port(),
                crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT,
            )
            .await;

        let tcp_stream = match tcp_stream {
            Ok(tcp_stream) => tcp_stream,
            Err(err) => {
                return Err(MyHttpClientError::CanNotConnectToRemoteHost(format!(
                    "{}. Err: {:?}",
                    self.get_remote_host().as_str(),
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
