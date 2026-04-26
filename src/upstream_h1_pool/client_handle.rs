use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use my_http_client::{
    http1::{MyHttpClient, MyHttpRequest, MyHttpResponse},
    MyHttpClientConnector, MyHttpClientError,
};

use super::H1Slot;

/// RAII wrapper around an h1 upstream client.
///
/// `rental = Some(slot)` — reusable: on `Drop` the slot's `rented` flag is reset
/// and the client stays in the pool for the next request.
///
/// `rental = None` — disposable: on `Drop` the inner `MyHttpClient` is the only
/// reference left, so the underlying TCP connection is closed via
/// `MyHttpClient::Drop`.
pub struct H1ClientHandle<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    client: Arc<MyHttpClient<TStream, TConnector>>,
    rental: Option<Arc<H1Slot<TStream, TConnector>>>,
}

impl<TStream, TConnector> H1ClientHandle<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub(super) fn reusable(
        client: Arc<MyHttpClient<TStream, TConnector>>,
        slot: Arc<H1Slot<TStream, TConnector>>,
    ) -> Self {
        Self {
            client,
            rental: Some(slot),
        }
    }

    pub(super) fn disposable(client: Arc<MyHttpClient<TStream, TConnector>>) -> Self {
        Self {
            client,
            rental: None,
        }
    }

    pub async fn do_request(
        &self,
        req: &MyHttpRequest,
        request_timeout: Duration,
    ) -> Result<MyHttpResponse<TStream>, MyHttpClientError> {
        self.client.do_request(req, request_timeout).await
    }

    /// Marks the underlying connection as transitioned to WebSocket. After this
    /// the connection cannot serve further HTTP requests, so a reusable handle
    /// drops its rental link — the slot stays empty until the supervisor
    /// reconnects it.
    pub fn upgraded_to_websocket(&mut self) {
        if let Some(slot) = self.rental.as_ref() {
            // Take the live client out of the slot — once the WS-upgraded TCP
            // is gone, this MyHttpClient is no longer reusable.
            slot.client.store(None);
        }
        self.rental = None;
    }
}

impl<TStream, TConnector> Drop for H1ClientHandle<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn drop(&mut self) {
        if let Some(slot) = self.rental.take() {
            slot.rented.store(false, Ordering::Release);
        }
    }
}
