use std::sync::atomic::AtomicI64;

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use hyper::{body::Incoming, Response};
use rust_extensions::{date_time::DateTimeAsMicroseconds, StopWatch};

use crate::{
    app::AppContext,
    http_client::HTTP_CLIENT_TIMEOUT,
    http_proxy_pass::{HttpProxyPassRemoteEndpoint, ProxyPassError},
};

static CONNECTIONS: AtomicI64 = AtomicI64::new(0);

pub struct RemoteHttpContentSource {
    http_client: HttpClient,
    pub remote_endpoint: HttpProxyPassRemoteEndpoint,
    id: i64,
    debug: bool,
}

impl RemoteHttpContentSource {
    pub fn new(id: i64, remote_endpoint: HttpProxyPassRemoteEndpoint, debug: bool) -> Self {
        CONNECTIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self {
            http_client: HttpClient::new(),
            remote_endpoint,
            id,
            debug,
        }
    }

    pub async fn connect_if_require(
        &mut self,
        app: &AppContext,
        domain_name: &Option<String>,
        debug: bool,
    ) -> Result<(), ProxyPassError> {
        if self.http_client.has_connection() {
            return Ok(());
        }

        match &self.remote_endpoint {
            HttpProxyPassRemoteEndpoint::Http(uri) => {
                if debug {
                    println!("Connecting to Http remote endpoint: {:?}", uri);
                }
                self.http_client
                    .connect_to_http1(uri, domain_name, debug)
                    .await?;
            }

            HttpProxyPassRemoteEndpoint::Http2(uri) => {
                if debug {
                    println!("Connecting to Http2 remote endpoint: {:?}", uri);
                }
                self.http_client.connect_to_http2(uri).await?;
            }

            HttpProxyPassRemoteEndpoint::Http1OverSsh {
                ssh_credentials,
                remote_host,
            } => {
                let mut sw = StopWatch::new();

                sw.start();

                if debug {
                    println!(
                        "[{}]. Http1OverSsh. Connecting to remote endpoint: {}",
                        self.id,
                        ssh_credentials.to_string(),
                    );
                }
                self.http_client
                    .connect_to_http1_over_ssh(app, ssh_credentials, remote_host)
                    .await?;
                sw.pause();

                if debug {
                    println!(
                        "[{}]. Http1OverSsh. Connected to remote endpoint: {}@{} in {}",
                        self.id,
                        ssh_credentials.to_string(),
                        remote_host.to_string(),
                        sw.duration_as_string()
                    );
                }
            }

            HttpProxyPassRemoteEndpoint::Http2OverSsh {
                ssh_credentials,
                remote_host,
            } => {
                let mut sw = StopWatch::new();

                sw.start();

                if debug {
                    println!(
                        "[{}]. Http2OverSsh. Connecting to remote endpoint: {}@{}",
                        self.id,
                        ssh_credentials.to_string(),
                        remote_host.to_string()
                    );
                }
                self.http_client
                    .connect_to_http2_over_ssh(app, ssh_credentials, remote_host)
                    .await?;
                sw.pause();

                if debug {
                    println!(
                        "[{}]. Http2OverSsh. Connected to remote endpoint: {}@{} in {}",
                        self.id,
                        ssh_credentials.to_string(),
                        remote_host.to_string(),
                        sw.duration_as_string()
                    );
                }
            }
        }

        Ok(())
    }

    pub async fn send_http1_request(
        &mut self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<Result<Response<Incoming>, hyper::Error>, ProxyPassError> {
        let client = self.http_client.unwrap_as_http1(self.id)?;

        let feature = {
            let mut write_access = client.lock().await;
            write_access.send_request.send_request(req)
        };

        let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, feature).await;

        if result.is_err() {
            return Err(ProxyPassError::Timeout);
        }

        Ok(result.unwrap())
    }

    pub async fn send_http1_over_ssh_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<Result<hyper::Response<BoxBody<Bytes, String>>, ProxyPassError>, ProxyPassError>
    {
        let client = self.http_client.unwrap_as_http1_over_ssh(self.id)?;

        let feature = client.http_client.send(req);

        let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, feature).await;
        if result.is_err() {
            return Err(ProxyPassError::Timeout);
        }

        Ok(result.unwrap())
    }

    pub async fn send_http2_request(
        &mut self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<Result<Response<Incoming>, hyper::Error>, ProxyPassError> {
        let http2_client = self.http_client.unwrap_as_http2(self.id)?;

        let feature = {
            let mut write_access = http2_client.lock().await;
            write_access.send_request.send_request(req.clone())
        };

        let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, feature).await;
        if result.is_err() {
            return Err(ProxyPassError::Timeout);
        }

        Ok(result.unwrap())
    }

    pub fn get_connected_moment(&self) -> Option<DateTimeAsMicroseconds> {
        self.http_client.get_connected_moment()
    }

    pub fn dispose(&mut self) {
        if self.debug {
            println!("Disposing ProxyPassConfiguration: {}", self.id);
        }
        self.http_client.dispose();
    }

    pub fn disconnect(&mut self) {
        self.http_client.dispose();
    }
}

impl Drop for RemoteHttpContentSource {
    fn drop(&mut self) {
        CONNECTIONS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        let connections_remain = CONNECTIONS.load(std::sync::atomic::Ordering::SeqCst);

        if self.debug {
            println!(
                "[{}]. --------- Dropping ProxyPassConfiguration. Connections remain: {}",
                self.id, connections_remain
            )
        }
    }
}
