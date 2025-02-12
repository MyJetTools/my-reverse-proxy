use rust_extensions::MyTimerTick;

pub struct MetricsTimer;

#[async_trait::async_trait]
impl MyTimerTick for MetricsTimer {
    async fn tick(&self) {
        if let Some(server_gateway) = crate::app::APP_CTX.gateway_server.as_ref() {
            server_gateway.timer_1s().await;
        }

        for connection in crate::app::APP_CTX.gateway_clients.values() {
            connection.timer_1s().await;
        }
    }
}
