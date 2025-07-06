use std::{collections::HashMap, sync::Arc};

use my_http_client::{http2::MyHttp2Client, MyHttpClientConnector};
use tokio::sync::Mutex;

pub struct Http2Clients<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    data: Mutex<HashMap<i64, Arc<MyHttp2Client<TStream, TConnector>>>>,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > Http2Clients<TStream, TConnector>
{
    pub fn new() -> Self {
        Self {
            data: Default::default(),
        }
    }

    pub async fn get_or_create(
        &self,
        id: i64,
        crate_client: impl Fn() -> MyHttp2Client<TStream, TConnector>,
    ) -> Arc<MyHttp2Client<TStream, TConnector>> {
        {
            let read_access = self.data.lock().await;

            if let Some(client) = read_access.get(&id) {
                return client.clone();
            }
        }

        let client = crate_client();

        let client = Arc::new(client);

        let mut write_access = self.data.lock().await;

        write_access.insert(id, client.clone());

        client
    }
}
