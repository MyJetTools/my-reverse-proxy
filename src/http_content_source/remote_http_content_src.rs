use std::{
    future::Future,
    sync::{atomic::AtomicI64, Arc},
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{body::Incoming, Response};
use my_ssh::SshSession;
use rust_extensions::{date_time::DateTimeAsMicroseconds, StopWatch};

use crate::{
    app::AppContext, http_client::HttpClient, http_proxy_pass::ProxyPassError,
    settings::HttpProxyPassRemoteEndpoint,
};

static CONNECTIONS: AtomicI64 = AtomicI64::new(0);

pub struct RemoteHttpContentSource {
    ssh_session: Option<Arc<SshSession>>,
    http_client: HttpClient,
    pub remote_endpoint: HttpProxyPassRemoteEndpoint,
    id: i64,
}

impl RemoteHttpContentSource {
    pub fn new(id: i64, remote_endpoint: HttpProxyPassRemoteEndpoint) -> Self {
        CONNECTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            http_client: HttpClient::new(),
            ssh_session: None,
            remote_endpoint,
            id,
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

            HttpProxyPassRemoteEndpoint::Http1OverSsh {
                ssh_credentials,
                remote_host,
            } => {
                let mut sw = StopWatch::new();

                sw.start();
                println!(
                    "[{}]. Http1OverSsh. Connecting to remote endpoint: {}",
                    self.id,
                    ssh_credentials.to_string(),
                );
                let ssh_session = self
                    .http_client
                    .connect_to_http1_over_ssh(app, ssh_credentials, remote_host)
                    .await?;
                sw.pause();
                self.ssh_session = Some(ssh_session);
                println!(
                    "[{}]. Http1OverSsh. Connected to remote endpoint: {}@{} in {}",
                    self.id,
                    ssh_credentials.to_string(),
                    remote_host.to_string(),
                    sw.duration_as_string()
                );
            }

            HttpProxyPassRemoteEndpoint::Http2OverSsh {
                ssh_credentials,
                remote_host,
            } => {
                let mut sw = StopWatch::new();

                sw.start();
                println!(
                    "[{}]. Http2OverSsh. Connecting to remote endpoint: {}@{}",
                    self.id,
                    ssh_credentials.to_string(),
                    remote_host.to_string()
                );
                let ssh_session = self
                    .http_client
                    .connect_to_http2_over_ssh(app, ssh_credentials, remote_host)
                    .await?;
                sw.pause();
                self.ssh_session = Some(ssh_session);
                println!(
                    "[{}]. Http2OverSsh. Connected to remote endpoint: {}@{} in {}",
                    self.id,
                    ssh_credentials.to_string(),
                    remote_host.to_string(),
                    sw.duration_as_string()
                );
            }
        }

        Ok(())
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

impl Drop for RemoteHttpContentSource {
    fn drop(&mut self) {
        CONNECTIONS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        let connections_remain = CONNECTIONS.load(std::sync::atomic::Ordering::SeqCst);
        println!(
            "[{}]. --------- Dropping ProxyPassConfiguration. Connections remain: {}",
            self.id, connections_remain
        )
    }
}
