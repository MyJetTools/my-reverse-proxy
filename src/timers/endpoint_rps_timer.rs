use rust_extensions::MyTimerTick;

use crate::app::APP_CTX;

pub struct EndpointRpsTimer;

#[async_trait::async_trait]
impl MyTimerTick for EndpointRpsTimer {
    async fn tick(&self) {
        let snapshot = APP_CTX.rps.snapshot_and_reset();
        for (domain, count) in snapshot {
            APP_CTX
                .prometheus
                .set_domain_rps(&domain, count as i64);
        }
    }
}
