use std::collections::HashMap;

use my_http_client::{http1::MyHttpClient, MyHttpClientConnector};
use tokio::sync::Mutex;

pub struct HttpClientPoolInner<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    items: Mutex<HashMap<String, Vec<MyHttpClient<TStream, TConnector>>>>,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > HttpClientPoolInner<TStream, TConnector>
{
    pub fn new() -> Self {
        Self {
            items: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get_or_create(
        &self,
        remote_endpoint: &str,
        create_connector: impl FnOnce() -> TConnector,
    ) -> MyHttpClient<TStream, TConnector> {
        let mut items_access = self.items.lock().await;

        match items_access.get_mut(remote_endpoint) {
            Some(pool) => {
                if pool.is_empty() {
                    return MyHttpClient::new(create_connector());
                }

                pool.pop().unwrap()
            }
            None => MyHttpClient::new(create_connector()),
        }
    }

    pub async fn fill_connections_amount(&self, dest: &mut HashMap<String, usize>) {
        let items = self.items.lock().await;
        for (key, value) in items.iter() {
            dest.insert(key.clone(), value.len());
        }
    }

    pub async fn return_back(
        &self,
        remote_endpoint: String,
        my_http_client: MyHttpClient<TStream, TConnector>,
    ) {
        let mut items_access = self.items.lock().await;

        match items_access.get_mut(remote_endpoint.as_str()) {
            Some(pool) => pool.push(my_http_client),
            None => {
                items_access.insert(remote_endpoint, vec![my_http_client]);
            }
        }
    }
}
