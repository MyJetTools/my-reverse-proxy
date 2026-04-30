use std::future::Future;
use std::sync::{Arc, OnceLock};

use tokio::task::JoinHandle;

pub trait TaskMetricsHook: Send + Sync + 'static {
    fn inc(&self, name: &'static str);
    fn dec(&self, name: &'static str);
}

static HOOK: OnceLock<Arc<dyn TaskMetricsHook>> = OnceLock::new();

pub fn set_task_metrics_hook(hook: Arc<dyn TaskMetricsHook>) {
    let _ = HOOK.set(hook);
}

fn inc(name: &'static str) {
    if let Some(h) = HOOK.get() {
        h.inc(name);
    }
}

fn dec(name: &'static str) {
    if let Some(h) = HOOK.get() {
        h.dec(name);
    }
}

pub fn spawn_named<F>(name: &'static str, future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    inc(name);
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
        dec(self.name);
    }
}
