use std::sync::atomic::{AtomicBool, AtomicU8};

use arc_swap::ArcSwapOption;
use my_http_client::{http1::MyHttpClient, MyHttpClientConnector};

pub struct H1Slot<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub client: ArcSwapOption<MyHttpClient<TStream, TConnector>>,
    pub rented: AtomicBool,
    #[allow(dead_code)]
    pub fail_count: AtomicU8,
}

impl<TStream, TConnector> H1Slot<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            client: ArcSwapOption::const_empty(),
            rented: AtomicBool::new(false),
            fail_count: AtomicU8::new(0),
        }
    }
}

impl<TStream, TConnector> Default for H1Slot<TStream, TConnector>
where
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}
