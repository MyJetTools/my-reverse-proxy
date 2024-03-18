use std::{str::FromStr, sync::Arc, time::Duration};

use http_body_util::Full;
use hyper::{body::Bytes, client::conn::http1::SendRequest, Uri};
use my_ssh::{SshCredentials, SshSession};

use crate::{app::AppContext, http_server::ProxyPassError, settings::SshConfiguration};

use super::{HttpClientConnection, HttpClientError};

pub const TIMEOUT: Duration = Duration::from_secs(30);

pub struct HttpClient {
    pub connection: Option<HttpClientConnection>,
}

impl HttpClient {
    pub fn new() -> Self {
        Self { connection: None }
    }

    pub async fn connect_to_http(
        proxy_pass: &Uri,
    ) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
        let is_https = super::utils::is_https(proxy_pass);
        if is_https {
            let future = super::connect_to_tls_endpoint(proxy_pass);

            let result = tokio::time::timeout(TIMEOUT, future).await;

            if result.is_err() {
                return Err(HttpClientError::TimeOut);
            }

            result.unwrap()
        } else {
            let future = super::connect_to_http_endpoint(proxy_pass);

            let result = tokio::time::timeout(TIMEOUT, future).await;

            if result.is_err() {
                return Err(HttpClientError::TimeOut);
            }

            result.unwrap()
        }
    }

    pub async fn connect_to_http_over_ssh(
        app: &AppContext,
        configuration: &SshConfiguration,
    ) -> Result<(Arc<SshSession>, SendRequest<Full<Bytes>>), ProxyPassError> {
        let ssh_credentials = SshCredentials::SshAgent {
            ssh_host_port: std::net::SocketAddr::from_str(
                format!(
                    "{}:{}",
                    configuration.ssh_session_host, configuration.ssh_session_port
                )
                .as_str(),
            )
            .unwrap(),
            ssh_user_name: configuration.ssh_user_name.to_string(),
        };

        let ssh_credentials = Arc::new(ssh_credentials);

        let result = super::connect_to_http_over_ssh(
            app,
            ssh_credentials,
            &configuration.remote_host,
            configuration.remote_port,
        )
        .await?;

        Ok(result)
    }
}
