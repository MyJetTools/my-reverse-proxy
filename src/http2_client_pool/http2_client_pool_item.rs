use std::sync::{atomic::AtomicBool, Arc};

use bytes::Bytes;
use http_body_util::{combinators::BoxBody, Full};
use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector, MyHttpClientError};

use super::Http2ClientPoolInner;

pub struct Http2ClientPoolItem<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    my_http_client: Option<MyHttp2Client<TStream, TConnector>>,
    pool: Option<Arc<Http2ClientPoolInner<TStream, TConnector>>>,
    end_point: Option<String>,
    disposed: AtomicBool,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > Http2ClientPoolItem<TStream, TConnector>
{
    pub fn new(
        my_http_client: MyHttp2Client<TStream, TConnector>,
        pool: Arc<Http2ClientPoolInner<TStream, TConnector>>,
        end_point: String,
    ) -> Self {
        Self {
            my_http_client: Some(my_http_client),
            pool: Some(pool),
            end_point: Some(end_point),
            disposed: AtomicBool::new(false),
        }
    }

    pub async fn do_request(
        &self,
        req: hyper::Request<Full<Bytes>>,
        request_timeout: std::time::Duration,
    ) -> Result<hyper::Response<BoxBody<Bytes, String>>, MyHttpClientError> {
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
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > Drop for Http2ClientPoolItem<TStream, TConnector>
{
    fn drop(&mut self) {
        if self.disposed.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        let http_client = self.my_http_client.take().unwrap();

        let pool = self.pool.take().unwrap();

        let end_point = self.end_point.take().unwrap();
        tokio::spawn(async move {
            pool.return_back(end_point, http_client).await;
        });
    }
}
