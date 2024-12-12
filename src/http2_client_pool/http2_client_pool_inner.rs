use std::{collections::HashMap, sync::Arc};

use my_http_client::{
    http2::{MyHttp2Client, MyHttp2ClientMetrics},
    MyHttpClientConnector,
};
use rust_extensions::remote_endpoint::{RemoteEndpoint, RemoteEndpointOwned};
use tokio::sync::Mutex;

pub struct Http2ClientPoolInner<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    items: Mutex<HashMap<String, Vec<MyHttp2Client<TStream, TConnector>>>>,
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

    pub async fn get_or_create<'s>(
        &self,
        remote_endpoint: RemoteEndpoint<'s>,
        create_connector: impl Fn() -> (
            TConnector,
            Arc<dyn MyHttp2ClientMetrics + Send + Sync + 'static>,
        ),
    ) -> MyHttp2Client<TStream, TConnector> {
        let mut items_access = self.items.lock().await;

        match items_access.get_mut(remote_endpoint.as_str()) {
            Some(pool) => {
                let (connector, metrics) = create_connector();
                if pool.is_empty() {
                    return MyHttp2Client::new(connector, metrics);
                }

                pool.pop().unwrap()
            }
            None => {
                let (connector, metrics) = create_connector();
                MyHttp2Client::new(connector, metrics)
            }
        }
    }

    pub async fn return_back(
        &self,
        remote_endpoint: RemoteEndpointOwned,
        my_http_client: MyHttp2Client<TStream, TConnector>,
    ) {
        let mut items_access = self.items.lock().await;

        match items_access.get_mut(remote_endpoint.as_str()) {
            Some(pool) => pool.push(my_http_client),
            None => {
                items_access.insert(remote_endpoint.as_str().to_string(), vec![my_http_client]);
            }
        }
    }
}
