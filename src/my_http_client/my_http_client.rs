use bytes::Bytes;

use http_body_util::{combinators::BoxBody, Full};

use std::sync::{atomic::AtomicU64, Arc};
use tokio::io::{ReadHalf, WriteHalf};

use super::{MyHttpClientConnector, MyHttpClientError, MyHttpClientInner, MyHttpRequest};

pub struct MyHttpClient<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    inner: Arc<MyHttpClientInner<TStream>>,
    connector: TConnector,
    connection_id: AtomicU64,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > MyHttpClient<TStream, TConnector>
{
    pub fn new(connector: TConnector) -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(1024);
        let inner = Arc::new(MyHttpClientInner::new(sender));

        let inner_cloned = inner.clone();
        tokio::spawn(async move {
            super::write_loop::write_loop(inner_cloned, receiver).await;
        });

        Self {
            inner,
            connector,
            connection_id: AtomicU64::new(0),
        }
    }

    async fn connect(&self) -> Result<(), MyHttpClientError> {
        let stream = self.connector.connect().await?;

        let current_connection_id = self
            .connection_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let (reader, writer) = tokio::io::split(stream);

        self.inner
            .new_connection(current_connection_id, writer)
            .await;

        let writer_cloned = self.inner.clone();
        tokio::spawn(async move {
            super::read_loop::read_loop(reader, current_connection_id, writer_cloned).await;
        });

        Ok(())
    }

    pub async fn send(
        &self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<hyper::Response<BoxBody<Bytes, String>>, MyHttpClientError> {
        let req = MyHttpRequest::new(req).await;

        loop {
            match self.inner.send(&req).await {
                Ok((awaiter, _)) => {
                    let result = awaiter.get_result().await?;
                    return Ok(result.unwrap_response());
                }
                Err(err) => {
                    if err.is_disconnected() {
                        self.connect().await?;
                        continue;
                    }

                    if err.is_web_socket_upgraded() {
                        self.connect().await?;
                        continue;
                    }

                    return Err(err);
                }
            }
        }
    }

    pub async fn upgrade_to_web_socket(
        &self,
        req: hyper::Request<Full<Bytes>>,
        reunite: impl Fn(ReadHalf<TStream>, WriteHalf<TStream>) -> TStream,
    ) -> Result<(TStream, hyper::Response<BoxBody<Bytes, String>>), MyHttpClientError> {
        let req = MyHttpRequest::new(req).await;

        loop {
            match self.inner.send(&req).await {
                Ok((awaiter, connection_id)) => {
                    let result = awaiter.get_result().await?;

                    let write_part = self.inner.upgrade_to_websocket(connection_id).await?;

                    let (response, read_part) = result.unwrap_websocket_upgrade();
                    let stream = reunite(read_part, write_part);

                    return Ok((stream, response));
                }
                Err(err) => {
                    if err.is_disconnected() {
                        self.connect().await?;
                        continue;
                    }

                    if err.is_web_socket_upgraded() {
                        self.connect().await?;
                        continue;
                    }

                    return Err(err);
                }
            }
        }
    }
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > Drop for MyHttpClient<TStream, TConnector>
{
    fn drop(&mut self) {
        let inner = self.inner.clone();
        tokio::spawn(async move {
            inner.dispose().await;
        });
    }
}
