use std::sync::Arc;

use my_ssh::{SshAsyncChannel, SshCredentials, SshSession};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{
    configurations::RemoteHost, http_proxy_pass::ProxyPassError, my_http_client::MyHttpClient,
};

use super::HTTP_CLIENT_TIMEOUT;

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
        let ssh_session = SshSession::new(ssh_credentials.clone());

        let tcp_stream = ssh_session
            .connect_to_remote_host(
                remote_host.get_host(),
                remote_host.get_port(),
                HTTP_CLIENT_TIMEOUT,
            )
            .await?;

        let http_client = MyHttpClient::new(tcp_stream);

        let result = Self {
            http_client,
            _connected: DateTimeAsMicroseconds::now(),
            _ssh_credentials: ssh_credentials.clone(),
            _ssh_session: Arc::new(ssh_session),
        };

        Ok(result)
    }
}
