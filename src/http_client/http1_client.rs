use std::sync::Arc;

use http_body_util::Full;
use hyper::{body::Bytes, client::conn::http1::SendRequest};
use my_ssh::{SshCredentials, SshSession};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{app::AppContext, http_proxy_pass::ProxyPassError, settings::RemoteHost};

use super::{HttpClientError, HTTP_CLIENT_TIMEOUT};

pub struct Http1Client {
    pub connected: DateTimeAsMicroseconds,
    pub send_request: SendRequest<Full<Bytes>>,
}

impl Http1Client {
    pub async fn connect(
        remote_host: &RemoteHost,
        domain_name: &Option<String>,
    ) -> Result<Self, HttpClientError> {
        let send_request = Self::connect_to_http(remote_host, domain_name).await?;

        let result = Self {
            send_request,
            connected: DateTimeAsMicroseconds::now(),
        };

        Ok(result)
    }

    pub async fn connect_over_ssh(
        app: &AppContext,
        ssh_credentials: &Arc<SshCredentials>,
        remote_host: &RemoteHost,
    ) -> Result<(Self, Arc<SshSession>), ProxyPassError> {
        let (ssh_session, send_request) =
            Self::connect_to_http_over_ssh(app, ssh_credentials, remote_host).await?;

        let result = Self {
            send_request,
            connected: DateTimeAsMicroseconds::now(),
        };

        Ok((result, ssh_session))
    }

    async fn connect_to_http(
        remote_host: &RemoteHost,
        domain_name: &Option<String>,
    ) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
        if remote_host.is_https() {
            let future = super::connect_to_tls_endpoint(remote_host, domain_name);

            let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

            if result.is_err() {
                return Err(HttpClientError::TimeOut);
            }

            result.unwrap()
        } else {
            let future = super::connect_to_http_endpoint(remote_host);

            let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

            if result.is_err() {
                return Err(HttpClientError::TimeOut);
            }

            result.unwrap()
        }
    }

    async fn connect_to_http_over_ssh(
        app: &AppContext,
        credentials: &Arc<SshCredentials>,
        remote_host: &RemoteHost,
    ) -> Result<(Arc<SshSession>, SendRequest<Full<Bytes>>), ProxyPassError> {
        let result = super::connect_to_http_over_ssh(app, credentials, remote_host).await?;

        Ok(result)
    }
}
