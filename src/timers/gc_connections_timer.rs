use rust_extensions::MyTimerTick;

pub struct GcConnectionsTimer;

#[async_trait::async_trait]
impl MyTimerTick for GcConnectionsTimer {
    async fn tick(&self) {
        crate::app::APP_CTX.http_clients_pool.gc().await;
        crate::app::APP_CTX.http_over_ssh_clients_pool.gc().await;
        crate::app::APP_CTX.https_clients_pool.gc().await;
        crate::app::APP_CTX.http2_clients_pool.gc().await;
        crate::app::APP_CTX.https2_clients_pool.gc().await;
        crate::app::APP_CTX.http2_over_ssh_clients_pool.gc().await;
    }
}
