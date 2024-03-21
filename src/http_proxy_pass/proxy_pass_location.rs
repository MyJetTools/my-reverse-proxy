use std::{
    future::Future,
    sync::{atomic::AtomicI64, Arc},
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Incoming, Response, Uri};
use my_ssh::SshSession;
use rust_extensions::{date_time::DateTimeAsMicroseconds, StopWatch};

use crate::{
    app::AppContext,
    http_client::HttpClient,
    settings::{HttpProxyPassRemoteEndpoint, ModifyHttpHeadersSettings},
};

use super::ProxyPassError;

static CONNECTIONS: AtomicI64 = AtomicI64::new(0);

pub struct ProxyPassLocation {
    ssh_session: Option<Arc<SshSession>>,
    http_client: HttpClient,
    pub remote_endpoint: HttpProxyPassRemoteEndpoint,
    pub path: String,
    pub id: i64,
    pub modify_headers: Option<ModifyHttpHeadersSettings>,
}

impl ProxyPassLocation {
    pub fn new(
        path: String,
        remote_endpoint: HttpProxyPassRemoteEndpoint,
        modify_headers: Option<ModifyHttpHeadersSettings>,
        id: i64,
    ) -> Self {
        CONNECTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            path,
            http_client: HttpClient::new(),
            remote_endpoint,
            id,
            ssh_session: None,
            modify_headers,
        }
    }
    pub async fn connect_if_require(&mut self, app: &AppContext) -> Result<(), ProxyPassError> {
        if self.http_client.has_connection() {
            return Ok(());
        }

        match &self.remote_endpoint {
            HttpProxyPassRemoteEndpoint::Http(uri) => {
                self.http_client.connect_to_http1(uri).await?;
            }

            HttpProxyPassRemoteEndpoint::Http2(uri) => {
                self.http_client.connect_to_http2(uri).await?;
            }

            HttpProxyPassRemoteEndpoint::Http1OverSsh(ssh_configuration) => {
                let mut sw = StopWatch::new();

                sw.start();
                println!(
                    "[{}]. Http1OverSsh. Connecting to remote endpoint: {}@{}:{}",
                    self.id,
                    ssh_configuration.ssh_user_name,
                    ssh_configuration.ssh_session_host,
                    ssh_configuration.ssh_session_port
                );
                let ssh_session = self
                    .http_client
                    .connect_to_http1_over_ssh(app, ssh_configuration)
                    .await?;
                sw.pause();
                self.ssh_session = Some(ssh_session);
                println!(
                    "[{}]. Http1OverSsh. Connected to remote endpoint: {}@{}:{} in {}",
                    self.id,
                    ssh_configuration.ssh_user_name,
                    ssh_configuration.ssh_session_host,
                    ssh_configuration.ssh_session_port,
                    sw.duration_as_string()
                );
            }

            HttpProxyPassRemoteEndpoint::Http2OverSsh(ssh_configuration) => {
                let mut sw = StopWatch::new();

                sw.start();
                println!(
                    "[{}]. Http2OverSsh. Connecting to remote endpoint: {}@{}:{}",
                    self.id,
                    ssh_configuration.ssh_user_name,
                    ssh_configuration.ssh_session_host,
                    ssh_configuration.ssh_session_port
                );
                let ssh_session = self
                    .http_client
                    .connect_to_http2_over_ssh(app, ssh_configuration)
                    .await?;
                sw.pause();
                self.ssh_session = Some(ssh_session);
                println!(
                    "[{}]. Http2OverSsh. Connected to remote endpoint: {}@{}:{} in {}",
                    self.id,
                    ssh_configuration.ssh_user_name,
                    ssh_configuration.ssh_session_host,
                    ssh_configuration.ssh_session_port,
                    sw.duration_as_string()
                );
            }
        }

        Ok(())
    }

    pub fn is_my_uri(&self, uri: &Uri) -> bool {
        let result = rust_extensions::str_utils::starts_with_case_insensitive(
            uri.path(),
            self.path.as_str(),
        );

        result
    }

    pub fn send_http1_request(
        &mut self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<impl Future<Output = Result<Response<Incoming>, hyper::Error>>, ProxyPassError>
    {
        let result = self
            .http_client
            .unwrap_as_http1_mut(self.id)?
            .send_request
            .send_request(req.clone());

        Ok(result)
    }

    pub fn send_http2_request(
        &mut self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<impl Future<Output = Result<Response<Incoming>, hyper::Error>>, ProxyPassError>
    {
        let result = self
            .http_client
            .unwrap_as_http2_mut(self.id)?
            .send_request
            .send_request(req.clone());

        Ok(result)
    }

    pub fn get_connected_moment(&self) -> Option<DateTimeAsMicroseconds> {
        self.http_client.get_connected_moment()
    }

    pub fn dispose(&mut self) {
        println!("Disposing ProxyPassConfiguration: {}", self.id);
        self.http_client.dispose();
    }
}

impl Drop for ProxyPassLocation {
    fn drop(&mut self) {
        CONNECTIONS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        let connections_remain = CONNECTIONS.load(std::sync::atomic::Ordering::SeqCst);
        println!(
            "[{}]. --------- Dropping ProxyPassConfiguration. Connections remain: {}",
            self.id, connections_remain
        )
    }
}
