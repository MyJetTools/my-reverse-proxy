use std::{collections::HashMap, sync::Arc};

use my_http_client::{
    http2::{MyHttp2Client, MyHttp2ClientMetrics},
    MyHttpClientConnector,
};
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::sync::Mutex;

pub struct Http2ClientPoolInner<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    items:
        Mutex<HashMap<String, Vec<(DateTimeAsMicroseconds, MyHttp2Client<TStream, TConnector>)>>>,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > Http2ClientPoolInner<TStream, TConnector>
{
    pub fn new() -> Self {
        Self {
            items: Mutex::new(HashMap::new()),
        }
    }

    pub async fn fill_connections_amount(&self, dest: &mut HashMap<String, usize>) {
        let items = self.items.lock().await;
        for (key, value) in items.iter() {
            if value.len() > 0 {
                dest.insert(key.clone(), value.len());
            }
        }
    }

    pub async fn get_or_create(
        &self,
        remote_endpoint: &str,
        connect_timeout: std::time::Duration,
        create_connector: impl Fn() -> (
            TConnector,
            Arc<dyn MyHttp2ClientMetrics + Send + Sync + 'static>,
        ),
    ) -> MyHttp2Client<TStream, TConnector> {
        let mut items_access = self.items.lock().await;

        match items_access.get_mut(remote_endpoint) {
            Some(pool) => {
                let (connector, metrics) = create_connector();
                if pool.is_empty() {
                    let mut result = MyHttp2Client::new(connector, metrics);
                    result.set_connect_timeout(connect_timeout);
                    return result;
                }

                pool.pop().unwrap().1
            }
            None => {
                let (connector, metrics) = create_connector();
                let mut result = MyHttp2Client::new(connector, metrics);

                result.set_connect_timeout(connect_timeout);
                result
            }
        }
    }

    pub async fn return_back(
        &self,
        remote_endpoint: String,
        my_http_client: MyHttp2Client<TStream, TConnector>,
    ) {
        let now = DateTimeAsMicroseconds::now();
        let mut items_access = self.items.lock().await;

        match items_access.get_mut(remote_endpoint.as_str()) {
            Some(pool) => pool.push((now, my_http_client)),
            None => {
                items_access.insert(remote_endpoint, vec![(now, my_http_client)]);
            }
        }
    }

    pub async fn gc(&self) {
        let now = DateTimeAsMicroseconds::now();
        let mut items_access = self.items.lock().await;

        for v in items_access.values_mut() {
            while v.len() > 0 {
                let first = v.first().unwrap().0;
                if now.duration_since(first).get_full_minutes() > 2 {
                    v.remove(0);
                } else {
                    break;
                }
            }

            if v.len() < 32 {
                v.shrink_to(32);
            }
        }
    }
}
