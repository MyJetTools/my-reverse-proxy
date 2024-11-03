use std::sync::Arc;

use my_ssh::{SshAsyncChannel, SshCredentials, SshSession};
use rust_extensions::StrOrString;

use crate::configurations::RemoteHost;

use my_http_client::{MyHttpClientConnector, MyHttpClientError};

use super::HTTP_CLIENT_TIMEOUT;

/*
pub struct Http1OverSshClient {
    pub http_client: MyHttpClient<SshAsyncChannel>,
    pub _ssh_credentials: Arc<SshCredentials>,
    pub _ssh_session: Arc<SshSession>,
    pub _connected: DateTimeAsMicroseconds,
}

impl Http1OverSshClient {
    pub async fn connect(
        ssh_credentials: &Arc<SshCredentials>,
        remote_host: &RemoteHost,
    ) -> Result<Self, ProxyPassError> {
    }
}
 */
pub struct Ssh1Connector {
    pub ssh_credentials: Arc<SshCredentials>,
    pub remote_host: RemoteHost,
    pub debug: bool,
}

#[async_trait::async_trait]
impl MyHttpClientConnector<SshAsyncChannel> for Ssh1Connector {
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
    async fn connect(&self) -> Result<SshAsyncChannel, MyHttpClientError> {
        let ssh_session = SshSession::new(self.ssh_credentials.clone());

        let tcp_stream = ssh_session
            .connect_to_remote_host(
                self.remote_host.get_host(),
                self.remote_host.get_port(),
                HTTP_CLIENT_TIMEOUT,
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
