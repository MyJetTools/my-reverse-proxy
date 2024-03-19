use std::{
    future::Future,
    sync::{atomic::AtomicI64, Arc},
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Incoming, Response, Uri};
use my_ssh::SshSession;
use rust_extensions::{date_time::DateTimeAsMicroseconds, StopWatch};

use crate::{app::AppContext, http_client::HttpClient, settings::ProxyPassRemoteEndpoint};

use super::ProxyPassError;

static CONNECTIONS: AtomicI64 = AtomicI64::new(0);

pub struct ProxyPassConfiguration {
    ssh_session: Option<Arc<SshSession>>,
    http_client: HttpClient,
    pub remote_endpoint: ProxyPassRemoteEndpoint,
    pub location: String,
    pub id: i64,
}

impl ProxyPassConfiguration {
    pub fn new(location: String, remote_endpoint: ProxyPassRemoteEndpoint, id: i64) -> Self {
        CONNECTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            location,
            http_client: HttpClient::new(),
            remote_endpoint,
            id,
            ssh_session: None,
        }
    }
    pub async fn connect_if_require(&mut self, app: &AppContext) -> Result<(), ProxyPassError> {
        if self.http_client.has_connection() {
            return Ok(());
        }

        match &self.remote_endpoint {
            ProxyPassRemoteEndpoint::Http(uri) => {
                self.http_client.connect_to_http1(uri).await?;
            }

            ProxyPassRemoteEndpoint::Http2(uri) => {
                self.http_client.connect_to_http2(uri).await?;
            }

            ProxyPassRemoteEndpoint::Http1OverSsh(ssh_configuration) => {
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

            ProxyPassRemoteEndpoint::Http2OverSsh(ssh_configuration) => {
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
            self.location.as_str(),
        );

        result
    }

    pub fn send_http1_request(
        &mut self,
        req: hyper::Request<Full<Bytes>>,
    ) -> impl Future<Output = Result<Response<Incoming>, hyper::Error>> {
        self.http_client
            .unwrap_as_http1_mut()
            .unwrap()
            .send_request
            .send_request(req.clone())
    }

    pub fn send_http2_request(
        &mut self,
        req: hyper::Request<Full<Bytes>>,
    ) -> impl Future<Output = Result<Response<Incoming>, hyper::Error>> {
        self.http_client
            .unwrap_as_http2_mut()
            .unwrap()
            .send_request
            .send_request(req.clone())
    }

    pub fn get_connected_moment(&self) -> Option<DateTimeAsMicroseconds> {
        self.http_client.get_connected_moment()
    }

    pub fn dispose(&mut self) {
        self.http_client.dispose();
    }
}

impl Drop for ProxyPassConfiguration {
    fn drop(&mut self) {
        CONNECTIONS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        let connections_remain = CONNECTIONS.load(std::sync::atomic::Ordering::SeqCst);
        println!(
            "[{}]. --------- Dropping ProxyPassConfiguration. Connections remain: {}",
            self.id, connections_remain
        )
    }
}
