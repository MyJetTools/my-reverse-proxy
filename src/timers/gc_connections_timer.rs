use std::sync::Arc;

use rust_extensions::MyTimerTick;

use crate::app::AppContext;

pub struct GcConnectionsTimer {
    app: Arc<AppContext>,
}

impl GcConnectionsTimer {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for GcConnectionsTimer {
    async fn tick(&self) {
        self.app.http_clients_pool.gc().await;
        self.app.http_over_ssh_clients_pool.gc().await;
        self.app.https_clients_pool.gc().await;
        self.app.http2_clients_pool.gc().await;
        self.app.https2_clients_pool.gc().await;
        self.app.http2_over_ssh_clients_pool.gc().await;
    }
}
