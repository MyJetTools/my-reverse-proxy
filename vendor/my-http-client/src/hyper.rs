use std::time::Duration;

use rust_extensions::date_time::DateTimeAsMicroseconds;

#[derive(Debug)]
pub enum SendHyperPayloadError {
    Disconnected,
    Disposed,
    UpgradedToWebsocket,
    RequestTimeout(Duration),
    HyperError {
        connected: DateTimeAsMicroseconds,
        err: hyper::Error,
    },
}

pub const HYPER_INIT_TIMEOUT: Duration = Duration::from_secs(5);

pub trait MyHttpHyperClientMetrics {
    fn instance_created(&self, name: &str);
    fn instance_disposed(&self, name: &str);
    fn connected(&self, name: &str);
    fn disconnected(&self, name: &str);
}
