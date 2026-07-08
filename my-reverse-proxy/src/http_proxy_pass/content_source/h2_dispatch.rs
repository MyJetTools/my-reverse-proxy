use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Empty, Full};
use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector, MyHttpClientDisconnect};
use my_http_client::{HyperResponse, MyHttpClientError};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::http_proxy_pass::ProxyPassError;
use crate::http2_client_pool::Http2ClientPoolItem;
use crate::upstream_h2_pool::H2Pool;

use super::{HttpResponse, WebSocketUpgradeStream};

#[async_trait::async_trait]
pub trait H2Sender: Send + Sync {
    async fn do_request(
        &self,
        req: &hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<HyperResponse, MyHttpClientError>;

    async fn do_extended_connect(
        &self,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError>;
}

#[async_trait::async_trait]
impl<TStream, TConnector> H2Sender for Http2ClientPoolItem<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    async fn do_request(
        &self,
        req: &hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<HyperResponse, MyHttpClientError> {
        Http2ClientPoolItem::do_request(self, req, request_timeout).await
    }

    async fn do_extended_connect(
        &self,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError> {
        Http2ClientPoolItem::do_extended_connect(self, path, headers, request_timeout).await
    }
}

// UDS-specific impl: `:authority` for Unix sockets must be a placeholder, not the
// filesystem path. Routed via `do_extended_connect_unix` on the inner client.
#[async_trait::async_trait]
impl<TConnector> H2Sender for Arc<MyHttp2Client<tokio::net::UnixStream, TConnector>>
where
    TConnector: MyHttpClientConnector<tokio::net::UnixStream> + Send + Sync + 'static,
{
    async fn do_request(
        &self,
        req: &hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<HyperResponse, MyHttpClientError> {
        MyHttp2Client::do_request(self.as_ref(), req, request_timeout).await
    }

    async fn do_extended_connect(
        &self,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError> {
        MyHttp2Client::do_extended_connect_unix(self.as_ref(), path, headers, request_timeout)
            .await
    }
}

#[async_trait::async_trait]
impl<TConnector> H2Sender for Arc<MyHttp2Client<tokio::net::TcpStream, TConnector>>
where
    TConnector: MyHttpClientConnector<tokio::net::TcpStream> + Send + Sync + 'static,
{
    async fn do_request(
        &self,
        req: &hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<HyperResponse, MyHttpClientError> {
        MyHttp2Client::do_request(self.as_ref(), req, request_timeout).await
    }

    async fn do_extended_connect(
        &self,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError> {
        MyHttp2Client::do_extended_connect(self.as_ref(), path, headers, request_timeout).await
    }
}

#[async_trait::async_trait]
impl<TConnector> H2Sender
    for Arc<
        MyHttp2Client<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>, TConnector>,
    >
where
    TConnector: MyHttpClientConnector<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>
        + Send
        + Sync
        + 'static,
{
    async fn do_request(
        &self,
        req: &hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<HyperResponse, MyHttpClientError> {
        MyHttp2Client::do_request(self.as_ref(), req, request_timeout).await
    }

    async fn do_extended_connect(
        &self,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError> {
        MyHttp2Client::do_extended_connect(self.as_ref(), path, headers, request_timeout).await
    }
}

struct H2NoopDisconnect;

impl MyHttpClientDisconnect for H2NoopDisconnect {
    fn disconnect(&self) {}
    fn web_socket_disconnect(&self) {}
    fn get_connection_id(&self) -> u64 {
        0
    }
}

/// RAII guard for an on-demand h2 WS upstream connection. Holds the client Arc
/// alive for the duration of the WS session and increments/decrements the
/// `h2_ws_active{endpoint}` Prometheus gauge.
pub struct H2WsActiveGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    endpoint_label: String,
    _client: Arc<MyHttp2Client<TStream, TConnector>>,
}

impl<TStream, TConnector> H2WsActiveGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new(endpoint_label: String, client: Arc<MyHttp2Client<TStream, TConnector>>) -> Self {
        crate::app::APP_CTX
            .prometheus
            .inc_h2_ws_active(&endpoint_label);
        Self {
            endpoint_label,
            _client: client,
        }
    }
}

impl<TStream, TConnector> MyHttpClientDisconnect for H2WsActiveGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn disconnect(&self) {}
    fn web_socket_disconnect(&self) {}
    fn get_connection_id(&self) -> u64 {
        0
    }
}

impl<TStream, TConnector> Drop for H2WsActiveGuard<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn drop(&mut self) {
        crate::app::APP_CTX
            .prometheus
            .dec_h2_ws_active(&self.endpoint_label);
    }
}

pub fn is_h2_extended_connect(req: &hyper::Request<Full<Bytes>>) -> bool {
    if req.method() != hyper::Method::CONNECT {
        return false;
    }
    match req.extensions().get::<hyper::ext::Protocol>() {
        Some(p) => p.as_ref().eq_ignore_ascii_case(b"websocket"),
        None => false,
    }
}

pub async fn execute_h2(
    sender: &impl H2Sender,
    req: &hyper::Request<Full<Bytes>>,
    request_timeout: Duration,
) -> Result<HttpResponse, ProxyPassError> {
    if is_h2_extended_connect(req) {
        let path = req
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());

        let headers = req.headers().clone();

        let upgraded = sender
            .do_extended_connect(&path, headers, request_timeout)
            .await?;

        let response = hyper::Response::builder()
            .status(hyper::StatusCode::OK)
            .body(Empty::<Bytes>::new().map_err(|never| match never {}).boxed())
            .unwrap();

        let disconnection: Arc<dyn MyHttpClientDisconnect + Send + Sync + 'static> =
            Arc::new(H2NoopDisconnect);

        return Ok(HttpResponse::WebSocketUpgrade {
            stream: WebSocketUpgradeStream::H2Upgraded(upgraded),
            response,
            disconnection,
        });
    }

    let response = sender.do_request(req, request_timeout).await?;
    Ok(HttpResponse::Response(response))
}

/// Full request flow through an `H2Pool`, shared by every h2 content source
/// (tcp / tls / uds — they differ only in stream & connector type).
///
/// - **WebSocket** (extended CONNECT): a dedicated off-pool connection via
///   `create_connection`, kept alive by `H2WsActiveGuard` for the session.
/// - **Regular request**: pick a live entry and run it. On a
///   **connection-level** error mark that entry dead (only if the failing
///   client is still the entry's current one — a straggler must not re-kill
///   a just-revived entry), kick a background revive, and — **only for an
///   idempotent method** — **fail over** to a different live entry (one
///   retry). A single dead upstream connection is then invisible to the
///   client: it silently lands on a healthy one.
///
///   A `RequestTimeout` is NOT a connection failure — the upstream is slow,
///   the connection is fine: the entry stays in rotation and the request is
///   not replayed (replaying a slow request doubles the load exactly when
///   the upstream is degraded).
///
///   The body is fully buffered (`Full<Bytes>`), so replay is safe
///   payload-wise. POST/PATCH are **not** retried at this layer — a lost
///   reply must not double-execute. The guarantee holds end-to-end:
///   `MyHttp2Client::do_request` replays a non-idempotent request only when
///   it provably never reached the wire (`Disconnected` before send /
///   hyper-canceled before dispatch). Replayed PUT/DELETE may observably
///   substitute the response (e.g. a replayed DELETE returns 404 after the
///   first one succeeded) — same policy as nginx's default for idempotent
///   methods. 4xx/5xx are not errors — they return Ok.
pub async fn execute_pooled_h2<TStream, TConnector>(
    pool: &Arc<H2Pool<TStream, TConnector>>,
    endpoint_label: &str,
    req: hyper::Request<Full<Bytes>>,
    request_timeout: Duration,
) -> Result<HttpResponse, ProxyPassError>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    Arc<MyHttp2Client<TStream, TConnector>>: H2Sender,
{
    if is_h2_extended_connect(&req) {
        let client = pool
            .create_connection()
            .await
            .map_err(|_| ProxyPassError::UpstreamUnavailable)?;
        let mut response = execute_h2(&client, &req, request_timeout).await?;
        if let HttpResponse::WebSocketUpgrade { disconnection, .. } = &mut response {
            *disconnection = Arc::new(H2WsActiveGuard::new(endpoint_label.to_string(), client));
        }
        return Ok(response);
    }

    // Non-WS: fail over to another live entry on a transport error, but only
    // for idempotent methods — a POST/PATCH may already have been processed
    // upstream, so re-sending it is not safe. Non-idempotent → single attempt.
    // The request is passed by reference through every attempt (do_request
    // borrows it and clones internally only on the actual send), so failover
    // costs no extra request clone.
    let max_attempts = if req.method().is_idempotent() { 2 } else { 1 };
    let mut last_err: Option<ProxyPassError> = None;

    for _ in 0..max_attempts {
        let entry = match pool.get_connection().await {
            Ok(entry) => entry,
            Err(_) => break,
        };

        let client = entry.client.load_full();
        match execute_h2(&client, &req, request_timeout).await {
            Ok(response) => {
                entry.last_success.update(DateTimeAsMicroseconds::now());
                return Ok(response);
            }
            Err(err) => {
                if !is_connection_level_error(&err) {
                    // Slow upstream (timeout) or a non-transport error: the
                    // connection is healthy — no dead-marking, no failover.
                    return Err(err);
                }
                // A background revive may have already swapped in a fresh
                // client; only the entry's CURRENT client may kill it.
                if Arc::ptr_eq(&client, &entry.client.load_full()) {
                    entry.dead.store(true, Ordering::Relaxed);
                    pool.spawn_revive(entry.clone());
                }
                last_err = Some(err);
            }
        }
    }

    Err(last_err.unwrap_or(ProxyPassError::UpstreamUnavailable))
}

/// True for errors that mean the upstream **connection** is unusable (dial
/// failure, disconnect, transport error) — the trigger for dead-marking and
/// failover. `RequestTimeout` is excluded on purpose: the server is slow but
/// the connection works; killing it would also churn every other stream
/// multiplexed on it.
fn is_connection_level_error(err: &ProxyPassError) -> bool {
    match err {
        ProxyPassError::MyHttpClientError(e) => !matches!(
            e,
            MyHttpClientError::RequestTimeout(_) | MyHttpClientError::UpgradedToWebSocket
        ),
        _ => false,
    }
}
