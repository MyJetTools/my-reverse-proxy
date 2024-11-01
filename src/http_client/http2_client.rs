use std::sync::Arc;

use http_body_util::Full;
use hyper::{body::Bytes, client::conn::http2::SendRequest};
use my_ssh::SshCredentials;

use crate::ssh_to_http_port_forward::SshToHttpPortForwardConfiguration;
use crate::{app::AppContext, http_proxy_pass::ProxyPassError};

use crate::configurations::*;

use super::{HttpClientError, HTTP_CLIENT_TIMEOUT};

pub struct Http2Client {
    pub send_request: SendRequest<Full<Bytes>>,
    pub _port_forward: Option<Arc<SshToHttpPortForwardConfiguration>>,
}

impl Http2Client {
    pub async fn connect_to_http2_int(
        remote_host: &RemoteHost,
    ) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
        let is_https = remote_host.is_https();
        if is_https {
            panic!("TLS not supported yet");
            /*
            let future = super::connect_to_tls_endpoint(proxy_pass);

            let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

            if result.is_err() {
                return Err(HttpClientError::TimeOut);
            }

            result.unwrap()

             */
        } else {
            let future = super::connect_to_http2_endpoint(remote_host);
            let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;
            if result.is_err() {
                return Err(HttpClientError::TimeOut);
            }
            result.unwrap()
        }
    }

    pub async fn connect(proxy_pass: &RemoteHost) -> Result<Self, HttpClientError> {
        let send_request = Self::connect_to_http2_int(proxy_pass).await?;

        let result = Self {
            send_request,
            _port_forward: None,
        };

        Ok(result)
    }

    pub async fn connect_over_ssh(
        app: &AppContext,
        ssh_credentials: &Arc<SshCredentials>,
        remote_host: &RemoteHost,
    ) -> Result<Self, ProxyPassError> {
        let (send_request, port_forward) =
            super::connect_to_http2_over_ssh(app, ssh_credentials, remote_host).await?;

        let result = Self {
            send_request,
            _port_forward: Some(port_forward),
        };

        Ok(result)
    }

    /*
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
     */
}
