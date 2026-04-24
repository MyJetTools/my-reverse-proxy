use std::{collections::HashMap, sync::Arc};

use my_http_client::{http1::MyHttpClient, MyHttpClientConnector};
use parking_lot::Mutex;

pub struct HttpClients<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
    TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
> {
    data: Mutex<HashMap<i64, Arc<MyHttpClient<TStream, TConnector>>>>,
}

impl<
        TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
        TConnector: MyHttpClientConnector<TStream> + Send + Sync + 'static,
    > HttpClients<TStream, TConnector>
{
    pub fn new() -> Self {
        Self {
            data: Default::default(),
        }
    }

    pub fn get_or_create(
        &self,
        id: i64,
        crate_client: impl Fn() -> MyHttpClient<TStream, TConnector>,
    ) -> Arc<MyHttpClient<TStream, TConnector>> {
        {
            let read_access = self.data.lock();

            if let Some(client) = read_access.get(&id) {
                return client.clone();
            }
        }

        let client = crate_client();

        let client = Arc::new(client);

        let mut write_access = self.data.lock();

        write_access.insert(id, client.clone());

        client
    }

    pub fn remove(&self, id: i64) {
        let mut write_access = self.data.lock();
        write_access.remove(&id);
    }
}
