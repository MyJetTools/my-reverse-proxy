use rust_extensions::MyTimerTick;

use crate::app::APP_CTX;

pub struct IpBlocklistGcTimer;

#[async_trait::async_trait]
impl MyTimerTick for IpBlocklistGcTimer {
    async fn tick(&self) {
        let blocked = APP_CTX.ip_blocklist.cleanup();
        APP_CTX.prometheus.set_ip_blocklist_size(blocked as i64);
    }
}
