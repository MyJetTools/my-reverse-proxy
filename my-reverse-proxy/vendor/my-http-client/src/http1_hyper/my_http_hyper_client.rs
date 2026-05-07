use std::{
    marker::PhantomData,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use bytes::Bytes;
use http::StatusCode;
use http_body_util::{combinators::BoxBody, Full};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{MyHttpClientConnector, MyHttpClientDisconnect, MyHttpClientError};

use super::*;
use crate::hyper::*;

pub struct MyHttpHyperClient<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    connector: TConnector,
    stream: PhantomData<TStream>,
    inner: Arc<MyHttpHyperClientInner>,
    connect_timeout: Duration,
    connection_id: AtomicU64,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > MyHttpHyperClient<TStream, TConnector>
{
    pub fn new(connector: TConnector) -> Self {
        Self {
            inner: Arc::new(MyHttpHyperClientInner::new(
                connector
                    .get_remote_endpoint()
                    .get_host_port()
                    .to_string()
                    .into(),
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
            inner: Arc::new(MyHttpHyperClientInner::new(
                connector
                    .get_remote_endpoint()
                    .get_host_port()
                    .to_string()
                    .into(),
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

    async fn get_response(
        &self,
        req: hyper::Request<Full<Bytes>>,
        response: hyper::Response<BoxBody<Bytes, String>>,
    ) -> Result<HyperHttpResponse, MyHttpClientError> {
        if response.status() == StatusCode::SWITCHING_PROTOCOLS {
            self.inner.upgrade_to_websocket().await?;
            let response = hyper_tungstenite::upgrade(req, None).unwrap();
            let result = HyperHttpResponse::WebSocketUpgrade {
                response: crate::utils::into_full_body_response(response.0),
                web_socket: response.1,
            };

            return Ok(result);
        }

        Ok(HyperHttpResponse::Response(response))
    }

    pub async fn do_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
        request_timeout: Duration,
    ) -> Result<HyperHttpResponse, MyHttpClientError> {
        let mut retry_no = 0;
        loop {
            let err = match self.inner.send_payload(&req, request_timeout).await {
                Ok(response) => return self.get_response(req, response).await,
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

        let send_request = super::wrap_http1_endpoint::wrap_http1_endpoint(
            stream,
            remote_host_port.as_str(),
            self.inner.clone(),
            connection_id,
        )
        .await?;

        *state = MyHttpHyperConnectionState::Connected {
            connected: DateTimeAsMicroseconds::now(),
            send_request,
            current_connection_id: connection_id,
            upgraded_to_websocket: false,
        };

        if let Some(metrics) = self.inner.metrics.as_ref() {
            metrics.connected(&self.inner.name);
        }

        Ok(())
    }
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > Drop for MyHttpHyperClient<TStream, TConnector>
{
    fn drop(&mut self) {
        let inner = self.inner.clone();
        crate::spawn_named("myhttp_hyper_client_drop_dispose", async move {
            inner.dispose().await;
        });
    }
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > From<TConnector> for MyHttpHyperClient<TStream, TConnector>
{
    fn from(value: TConnector) -> Self {
        Self::new(value)
    }
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > MyHttpClientDisconnect for MyHttpHyperClient<TStream, TConnector>
{
    fn disconnect(&self) {
        let inner = self.inner.clone();
        let connection_id = self
            .connection_id
            .load(std::sync::atomic::Ordering::Relaxed);
        crate::spawn_named("myhttp_hyper_disconnect", async move {
            inner.disconnect(connection_id).await
        });
    }
    fn web_socket_disconnect(&self) {
        let inner = self.inner.clone();
        let connection_id = self
            .connection_id
            .load(std::sync::atomic::Ordering::Relaxed);
        crate::spawn_named("myhttp_hyper_websocket_disconnect", async move {
            inner.disconnect(connection_id).await
        });
    }
    fn get_connection_id(&self) -> u64 {
        self.connection_id
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}
