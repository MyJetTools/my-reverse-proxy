use rust_extensions::MyTimerTick;

use crate::app::APP_CTX;

/// Drives one supervisor pass over every h1 and h2 upstream pool — fills empty
/// slots and runs the optional active health-check on h2 pools. Wrapped by
/// MyTimer so a panic inside one pool's tick does not stop future ticks.
pub struct PoolSupervisorTimer;

#[async_trait::async_trait]
impl MyTimerTick for PoolSupervisorTimer {
    async fn tick(&self) {
        for pool in APP_CTX.h1_tcp_pools.list_pools() {
            pool.supervisor_tick().await;
        }
        for pool in APP_CTX.h1_tls_pools.list_pools() {
            pool.supervisor_tick().await;
        }
        for pool in APP_CTX.h1_uds_pools.list_pools() {
            pool.supervisor_tick().await;
        }
        for pool in APP_CTX.h2_tcp_pools.list_pools() {
            pool.supervisor_tick().await;
        }
        for pool in APP_CTX.h2_tls_pools.list_pools() {
            pool.supervisor_tick().await;
        }
        for pool in APP_CTX.h2_uds_pools.list_pools() {
            pool.supervisor_tick().await;
        }
    }
}
