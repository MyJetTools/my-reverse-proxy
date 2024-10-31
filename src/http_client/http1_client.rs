use std::sync::Arc;

use http_body_util::Full;
use hyper::{body::Bytes, client::conn::http1::SendRequest};
use my_ssh::SshCredentials;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::ssh_to_http_port_forward::SshToHttpPortForwardConfiguration;
use crate::{app::AppContext, http_proxy_pass::ProxyPassError};

use crate::configurations::*;

use super::{HttpClientError, HTTP_CLIENT_TIMEOUT};

pub struct Http1Client {
    pub connected: DateTimeAsMicroseconds,
    pub send_request: SendRequest<Full<Bytes>>,
    _port_forward: Option<Arc<SshToHttpPortForwardConfiguration>>,
}

impl Http1Client {
    pub async fn connect(
        remote_host: &RemoteHost,
        domain_name: &Option<String>,
        debug: bool,
    ) -> Result<Self, HttpClientError> {
        let send_request = Self::connect_to_http(remote_host, domain_name, debug).await?;

        let result = Self {
            send_request,
            connected: DateTimeAsMicroseconds::now(),
            _port_forward: None,
        };

        Ok(result)
    }

    pub async fn connect_over_ssh_with_tunnel(
        app: &AppContext,
        ssh_credentials: &Arc<SshCredentials>,
        remote_host: &RemoteHost,
    ) -> Result<Self, ProxyPassError> {
        let (send_request, port_forward) =
            super::connect_to_http_over_ssh_with_tunnel(app, ssh_credentials, remote_host).await?;

        let result = Self {
            send_request,
            connected: DateTimeAsMicroseconds::now(),
            _port_forward: Some(port_forward),
        };

        Ok(result)
    }

    async fn connect_to_http(
        remote_host: &RemoteHost,
        domain_name: &Option<String>,
        debug: bool,
    ) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
        if remote_host.is_https() {
            let future = super::connect_to_tls_endpoint(remote_host, domain_name, debug);

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
}
