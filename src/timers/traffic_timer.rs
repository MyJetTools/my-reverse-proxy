use rust_extensions::MyTimerTick;

use crate::app::APP_CTX;

pub struct TrafficTimer;

#[async_trait::async_trait]
impl MyTimerTick for TrafficTimer {
    async fn tick(&self) {
        let snapshot = APP_CTX.traffic.snapshot_and_reset();
        for (domain, stats) in snapshot {
            APP_CTX.prometheus.set_traffic(
                &domain,
                stats.c2s_events as i64,
                stats.c2s_bytes as i64,
                stats.s2c_events as i64,
                stats.s2c_bytes as i64,
            );
        }
    }
}
