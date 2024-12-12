use std::{collections::HashMap, sync::Arc};

use my_http_client::MyHttpClientConnector;
use rust_extensions::StrOrString;

use super::{HttpClientPoolInner, HttpClientPoolItem};

pub struct HttpClientPool<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    inner: Arc<HttpClientPoolInner<TStream, TConnector>>,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > HttpClientPool<TStream, TConnector>
{
    pub fn new() -> Self {
        let inner = Arc::new(HttpClientPoolInner::new());
        Self { inner }
    }

    pub async fn fill_connections_amount(&self, dest: &mut HashMap<String, usize>) {
        self.inner.fill_connections_amount(dest).await;
    }

    pub async fn gc(&self) {
        self.inner.gc().await;
    }

    pub async fn get<'s>(
        &self,
        remote_endpoint: StrOrString<'s>,
        create_connector: impl FnOnce() -> TConnector,
    ) -> HttpClientPoolItem<TStream, TConnector> {
        let my_http_client = self
            .inner
            .get_or_create(remote_endpoint.as_str(), create_connector)
            .await;

        HttpClientPoolItem::new(
            my_http_client,
            self.inner.clone(),
            remote_endpoint.to_string(),
        )
    }
}
