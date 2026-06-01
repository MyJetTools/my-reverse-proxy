use std::future::Future;
use tokio::task::JoinHandle;

pub fn spawn_named<F>(_name: &'static str, future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    // TEMP/DEBUG: plain tokio::spawn. The previous body touched
    // `APP_CTX.prometheus` here — but `spawn_named` is also called from
    // `AppContext::new`, which runs *inside* the `lazy_static! APP_CTX`
    // initializer. That re-entrant deref of APP_CTX (a `Once`) prevented the
    // gateway accept loop from ever being scheduled. Avoid touching APP_CTX.
    tokio::spawn(future)
}
