use std::{collections::HashMap, sync::Arc};

use my_http_client::{http2::MyHttp2ClientMetrics, MyHttpClientConnector};
use rust_extensions::StrOrString;

use super::{Http2ClientPoolInner, Http2ClientPoolItem};

pub struct Http2ClientPool<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    inner: Arc<Http2ClientPoolInner<TStream, TConnector>>,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > Http2ClientPool<TStream, TConnector>
{
    pub fn new() -> Self {
        let inner = Arc::new(Http2ClientPoolInner::new());
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
        connect_timeout: std::time::Duration,

        create_connector: impl Fn() -> (
            TConnector,
            Arc<dyn MyHttp2ClientMetrics + Send + Sync + 'static>,
        ),
    ) -> Http2ClientPoolItem<TStream, TConnector> {
        let my_http_client = self
            .inner
            .get_or_create(remote_endpoint.as_str(), connect_timeout, create_connector)
            .await;

        Http2ClientPoolItem::new(
            my_http_client,
            self.inner.clone(),
            remote_endpoint.to_string(),
        )
    }
}
