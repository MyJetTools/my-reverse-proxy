use std::future::Future;
use tokio::task::JoinHandle;

use crate::app::APP_CTX;

pub fn spawn_named<F>(name: &'static str, future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    APP_CTX.prometheus.inc_tokio_task_spawned(name);
    let guard = SpawnGuard { name };
    tokio::spawn(async move {
        let _g = guard;
        future.await
    })
}

struct SpawnGuard {
    name: &'static str,
}

impl Drop for SpawnGuard {
    fn drop(&mut self) {
        APP_CTX.prometheus.dec_tokio_task_spawned(self.name);
    }
}
