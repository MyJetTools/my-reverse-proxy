use std::sync::atomic::{AtomicI64, AtomicU8};

use arc_swap::ArcSwapOption;
use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector};

pub struct H2Slot<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub client: ArcSwapOption<MyHttp2Client<TStream, TConnector>>,
    pub fail_count: AtomicU8,
    // Reserved for active health-check (currently disabled — see PoolParams::default).
    #[allow(dead_code)]
    pub last_health_check: AtomicI64,
}

impl<TStream, TConnector> H2Slot<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            client: ArcSwapOption::const_empty(),
            fail_count: AtomicU8::new(0),
            last_health_check: AtomicI64::new(0),
        }
    }
}

impl<TStream, TConnector> Default for H2Slot<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
