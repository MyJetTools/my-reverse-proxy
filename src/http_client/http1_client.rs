use std::{str::FromStr, sync::Arc};

use http_body_util::Full;
use hyper::{body::Bytes, client::conn::http1::SendRequest, Uri};
use my_ssh::{SshCredentials, SshSession};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{app::AppContext, http_proxy_pass::ProxyPassError, settings::SshConfiguration};

use super::{HttpClientError, HTTP_CLIENT_TIMEOUT};

pub struct Http1Client {
    pub connected: DateTimeAsMicroseconds,
    pub send_request: SendRequest<Full<Bytes>>,
}

impl Http1Client {
    pub async fn connect(proxy_pass: &Uri) -> Result<Self, HttpClientError> {
        let send_request = Self::connect_to_http(proxy_pass).await?;

        let result = Self {
            send_request,
            connected: DateTimeAsMicroseconds::now(),
        };

        Ok(result)
    }

    pub async fn connect_over_ssh(
        app: &AppContext,
        configuration: &SshConfiguration,
    ) -> Result<(Self, Arc<SshSession>), ProxyPassError> {
        let (ssh_session, send_request) =
            Self::connect_to_http_over_ssh(app, configuration).await?;

        let result = Self {
            send_request,
            connected: DateTimeAsMicroseconds::now(),
        };

        Ok((result, ssh_session))
    }

    async fn connect_to_http(
        proxy_pass: &Uri,
    ) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
        let is_https = super::utils::is_https(proxy_pass);
        if is_https {
            let future = super::connect_to_tls_endpoint(proxy_pass);

            let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

            if result.is_err() {
                return Err(HttpClientError::TimeOut);
            }

            result.unwrap()
        } else {
            let future = super::connect_to_http_endpoint(proxy_pass);

            let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

            if result.is_err() {
                return Err(HttpClientError::TimeOut);
            }

            result.unwrap()
        }
    }

    async fn connect_to_http_over_ssh(
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
