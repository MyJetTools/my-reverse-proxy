use std::{
    marker::PhantomData,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{MyHttpClientConnector, MyHttpClientError};

use super::{MyHttp2ClientInner, MyHttp2ConnectionState};
use crate::hyper::*;

pub struct MyHttp2Client<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    connector: TConnector,
    stream: PhantomData<TStream>,
    inner: Arc<MyHttp2ClientInner>,
    connect_timeout: Duration,
    connection_id: AtomicU64,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > MyHttp2Client<TStream, TConnector>
{
    pub fn new(connector: TConnector) -> Self {
        Self {
            inner: Arc::new(MyHttp2ClientInner::new(
                connector.get_remote_endpoint().get_host_port().to_string(),
                None,
            )),
            connector,

            stream: PhantomData,
            connect_timeout: Duration::from_secs(5),
            connection_id: AtomicU64::new(0),
        }
    }

    pub fn new_with_metrics(
        connector: TConnector,
        metrics: Arc<dyn MyHttpHyperClientMetrics + Send + Sync + 'static>,
    ) -> Self {
        Self {
            inner: Arc::new(MyHttp2ClientInner::new(
                connector.get_remote_endpoint().get_host_port().to_string(),
                Some(metrics),
            )),
            connector,

            stream: PhantomData,
            connect_timeout: Duration::from_secs(5),
            connection_id: AtomicU64::new(0),
        }
    }

    pub fn set_connect_timeout(&mut self, connection_timeout: Duration) {
        self.connect_timeout = connection_timeout;
    }

    pub async fn do_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<hyper::Response<BoxBody<Bytes, String>>, MyHttpClientError> {
        let mut retry_no = 0;
        loop {
            let err = match self.inner.send_payload(&req, request_timeout).await {
                Ok(response) => {
                    return Ok(response);
                }
                Err(err) => err,
            };

            match err {
                SendHyperPayloadError::Disconnected => {
                    self.connect().await?;
                }
                SendHyperPayloadError::RequestTimeout(duration) => {
                    if retry_no > 3 {
                        return Err(MyHttpClientError::RequestTimeout(duration));
                    }

                    self.inner.force_disconnect().await;
                    self.connect().await?;
                    retry_no += 1;
                    continue;
                }
                SendHyperPayloadError::HyperError { connected, err } => {
                    if err.is_canceled() {
                        let now = DateTimeAsMicroseconds::now();

                        if now.duration_since(connected).as_positive_or_zero() < HYPER_INIT_TIMEOUT
                        {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            continue;
                        }
                    }

                    if retry_no > 3 {
                        return Err(MyHttpClientError::CanNotExecuteRequest(err.to_string()));
                    }

                    retry_no += 1;

                    self.inner.force_disconnect().await;
                    self.connect().await?;
                }
                SendHyperPayloadError::Disposed => {
                    return Err(MyHttpClientError::Disposed);
                }
                SendHyperPayloadError::UpgradedToWebsocket => {
                    return Err(MyHttpClientError::UpgradedToWebSocket);
                }
            }
        }
    }

    pub async fn do_extended_connect(
        &self,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError> {
        let authority = self
            .connector
            .get_remote_endpoint()
            .get_host_port()
            .to_string();

        self.do_extended_connect_inner(&authority, path, headers, request_timeout)
            .await
    }

    /// Extended CONNECT for Unix Domain Socket transports.
    ///
    /// `:authority` over UDS is just metadata in the h2 HEADERS frame — actual routing
    /// is already done by the UDS connect. The connector's `host_port` is a filesystem
    /// path which produces an empty `:authority` (`http:///path/...`), and hyper's
    /// CONNECT validator rejects that with "invalid format". This method substitutes
    /// `localhost` as a placeholder authority.
    pub async fn do_extended_connect_unix(
        &self,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError> {
        self.do_extended_connect_inner("localhost", path, headers, request_timeout)
            .await
    }

    async fn do_extended_connect_inner(
        &self,
        authority: &str,
        path: &str,
        headers: hyper::HeaderMap,
        request_timeout: Duration,
    ) -> Result<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>, MyHttpClientError> {
        self.connect().await?;

        let mut req = hyper::Request::builder()
            .method(hyper::Method::CONNECT)
            .uri(format!("http://{}{}", authority, path))
            .body(Full::new(Bytes::new()))
            .map_err(|err| MyHttpClientError::CanNotExecuteRequest(err.to_string()))?;

        *req.headers_mut() = headers;

        req.extensions_mut()
            .insert(hyper::ext::Protocol::from_static("websocket"));

        let (send_fut, current_connection_id) = {
            let mut state = self.inner.state.lock().await;
            match &mut *state {
                MyHttp2ConnectionState::Disconnected => {
                    return Err(MyHttpClientError::Disconnected);
                }
                MyHttp2ConnectionState::Connected {
                    send_request,
                    current_connection_id,
                    ..
                } => (send_request.send_request(req), *current_connection_id),
                MyHttp2ConnectionState::Disposed => {
                    return Err(MyHttpClientError::Disposed);
                }
            }
        };

        let resp_result = tokio::time::timeout(request_timeout, send_fut).await;

        let resp = match resp_result {
            Err(_) => {
                self.inner.disconnect(current_connection_id).await;
                return Err(MyHttpClientError::RequestTimeout(request_timeout));
            }
            Ok(Err(err)) => {
                self.inner.disconnect(current_connection_id).await;
                return Err(MyHttpClientError::CanNotExecuteRequest(err.to_string()));
            }
            Ok(Ok(resp)) => resp,
        };

        if !resp.status().is_success() {
            return Err(MyHttpClientError::CanNotExecuteRequest(format!(
                "Extended CONNECT failed with status: {}",
                resp.status()
            )));
        }

        let upgraded = hyper::upgrade::on(resp).await.map_err(|err| {
            MyHttpClientError::CanNotExecuteRequest(format!(
                "Extended CONNECT upgrade failed: {}",
                err
            ))
        })?;

        Ok(hyper_util::rt::TokioIo::new(upgraded))
    }

    pub async fn connect(&self) -> Result<(), MyHttpClientError> {
        let connection_id = self
            .connection_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut state = self.inner.state.lock().await;

        if state.is_connected() {
            return Ok(());
        }

        let feature = self.connector.connect();

        let connect_result = tokio::time::timeout(self.connect_timeout, feature).await;

        let remote_host_port = self.connector.get_remote_endpoint().get_host_port();

        if connect_result.is_err() {
            return Err(MyHttpClientError::CanNotConnectToRemoteHost(format!(
                "Can not connect to Http2 remote endpoint: '{}' Timeout: {:?}",
                remote_host_port.as_str(),
                self.connect_timeout
            )));
        }

        let stream = connect_result.unwrap()?;

        let send_request = super::wrap_http2_endpoint::wrap_http2_endpoint(
            stream,
            remote_host_port.as_str(),
            self.inner.clone(),
            connection_id,
        )
        .await?;

        *state = MyHttp2ConnectionState::Connected {
            connected: DateTimeAsMicroseconds::now(),
            send_request,
            current_connection_id: connection_id,
        };

        if let Some(metrics) = self.inner.metrics.as_ref() {
            metrics.connected(&self.inner.name);
        }

        Ok(())
    }
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > Drop for MyHttp2Client<TStream, TConnector>
{
    fn drop(&mut self) {
        let inner = self.inner.clone();

        crate::spawn_named("myhttp_h2_client_drop_dispose", async move {
            inner.dispose().await;
        });
    }
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > From<TConnector> for MyHttp2Client<TStream, TConnector>
{
    fn from(value: TConnector) -> Self {
        Self::new(value)
    }
}
