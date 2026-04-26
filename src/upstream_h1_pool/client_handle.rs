use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use my_http_client::{
    http1::{MyHttpClient, MyHttpRequest, MyHttpResponse},
    MyHttpClientConnector, MyHttpClientError,
};
use rust_extensions::date_time::DateTimeAsMicroseconds;

use super::{H1Entry, DISPOSABLE_COUNTER};

/// h1 client handle returned by `H1Pool::get_connection` / `create_ws_connection`.
///
/// - **Reusable** — the client lives in the pool. Drop releases the rent flag
///   so the next request can pick this entry. `do_request` updates the entry's
///   `last_success` / `dead` based on result.
/// - **Disposable** — overflow client (or Phase 0 race-lost). Drop decrements
///   the global `DISPOSABLE_COUNTER`. Underlying TCP closes when Arc dies.
/// - **Ws** — fresh client created for a WebSocket session. Drop is a no-op;
///   the WS-upgraded TCP keeps the Arc alive until the session closes.
pub enum H1ClientHandle<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    Reusable {
        client: Arc<MyHttpClient<TStream, TConnector>>,
        entry: Arc<H1Entry<TStream, TConnector>>,
    },
    Disposable {
        client: Arc<MyHttpClient<TStream, TConnector>>,
    },
    Ws {
        client: Arc<MyHttpClient<TStream, TConnector>>,
    },
}

impl<TStream, TConnector> H1ClientHandle<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub(super) fn reusable(
        client: Arc<MyHttpClient<TStream, TConnector>>,
        entry: Arc<H1Entry<TStream, TConnector>>,
    ) -> Self {
        Self::Reusable { client, entry }
    }

    pub(super) fn disposable(client: Arc<MyHttpClient<TStream, TConnector>>) -> Self {
        Self::Disposable { client }
    }

    pub(super) fn ws(client: Arc<MyHttpClient<TStream, TConnector>>) -> Self {
        Self::Ws { client }
    }

    fn client(&self) -> &Arc<MyHttpClient<TStream, TConnector>> {
        match self {
            Self::Reusable { client, .. } => client,
            Self::Disposable { client } => client,
            Self::Ws { client } => client,
        }
    }

    pub async fn do_request(
        &self,
        req: &MyHttpRequest,
        request_timeout: Duration,
    ) -> Result<MyHttpResponse<TStream>, MyHttpClientError> {
        let result = self.client().do_request(req, request_timeout).await;
        if let Self::Reusable { entry, .. } = self {
            match &result {
                Ok(_) => entry.last_success.update(DateTimeAsMicroseconds::now()),
                Err(_) => entry.dead.store(true, Ordering::Relaxed),
            }
        }
        result
    }
}

impl<TStream, TConnector> Drop for H1ClientHandle<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn drop(&mut self) {
        match self {
            Self::Reusable { entry, .. } => entry.release_rent(),
            Self::Disposable { .. } => {
                DISPOSABLE_COUNTER.fetch_sub(1, Ordering::Relaxed);
            }
            Self::Ws { .. } => {}
        }
    }
}
