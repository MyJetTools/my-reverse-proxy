use std::sync::{atomic::AtomicBool, Arc};

use my_http_client::{
    http1::{MyHttpClient, MyHttpRequest, MyHttpResponse},
    MyHttpClientConnector, MyHttpClientError,
};

use super::HttpClientPoolInner;

pub struct HttpClientPoolItem<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    my_http_client: Option<MyHttpClient<TStream, TConnector>>,
    pool: Option<Arc<HttpClientPoolInner<TStream, TConnector>>>,
    end_point: Option<String>,
    disposed: AtomicBool,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > HttpClientPoolItem<TStream, TConnector>
{
    pub fn new(
        my_http_client: MyHttpClient<TStream, TConnector>,
        pool: Arc<HttpClientPoolInner<TStream, TConnector>>,
        end_point: String,
    ) -> Self {
        Self {
            my_http_client: Some(my_http_client),
            pool: Some(pool),
            end_point: Some(end_point),
            disposed: AtomicBool::new(false),
        }
    }

    pub fn upgraded_to_websocket(&mut self) {
        self.my_http_client.take();
    }

    pub async fn do_request(
        &self,
        req: &MyHttpRequest,
        request_timeout: std::time::Duration,
    ) -> Result<MyHttpResponse<TStream>, MyHttpClientError> {
        let result = self
            .my_http_client
            .as_ref()
            .unwrap()
            .do_request(req, request_timeout)
            .await;

        if result.is_err() {
            self.disposed
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

        result
    }
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > Drop for HttpClientPoolItem<TStream, TConnector>
{
    fn drop(&mut self) {
        if self.disposed.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        if let Some(http_client) = self.my_http_client.take() {
            let pool = self.pool.take().unwrap();

            let end_point = self.end_point.take().unwrap();
            tokio::spawn(async move {
                pool.return_back(end_point, http_client).await;
            });
        }
    }
}
