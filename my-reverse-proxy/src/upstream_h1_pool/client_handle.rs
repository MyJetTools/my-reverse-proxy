use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
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
///   both the global `DISPOSABLE_COUNTER` and the owning pool's per-upstream
///   `live_disposables`. Underlying TCP closes when Arc dies.
/// - **Ws** — fresh client created for a WebSocket session. Drop is a no-op;
///   the WS-upgraded TCP keeps the Arc alive until the session closes.
/// - **Dedicated** — fresh non-pooled client for a single request whose
///   response may stream indefinitely (e.g. MCP SSE). Not rented, not counted
///   as a disposable. Drop is a no-op: the caller ties this handle to the
///   response body (see `attach_conn_guard`), so the last `Arc` drops — and the
///   connection is disposed — only when the body ends.
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
        /// The owning pool's per-upstream live-disposable counter; dec'd on Drop.
        live_disposables: Arc<AtomicUsize>,
    },
    Ws {
        client: Arc<MyHttpClient<TStream, TConnector>>,
    },
    Dedicated {
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

    pub(super) fn disposable(
        client: Arc<MyHttpClient<TStream, TConnector>>,
        live_disposables: Arc<AtomicUsize>,
    ) -> Self {
        Self::Disposable {
            client,
            live_disposables,
        }
    }

    pub(super) fn ws(client: Arc<MyHttpClient<TStream, TConnector>>) -> Self {
        Self::Ws { client }
    }

    pub(super) fn dedicated(client: Arc<MyHttpClient<TStream, TConnector>>) -> Self {
        Self::Dedicated { client }
    }

    fn client(&self) -> &Arc<MyHttpClient<TStream, TConnector>> {
        match self {
            Self::Reusable { client, .. } => client,
            Self::Disposable { client, .. } => client,
            Self::Ws { client } => client,
            Self::Dedicated { client } => client,
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
            Self::Disposable {
                live_disposables, ..
            } => {
                DISPOSABLE_COUNTER.fetch_sub(1, Ordering::Relaxed);
                live_disposables.fetch_sub(1, Ordering::Relaxed);
            }
            Self::Ws { .. } => {}
            Self::Dedicated { .. } => {}
        }
    }
}
