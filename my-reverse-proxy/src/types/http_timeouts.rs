use std::time::Duration;

#[derive(Clone, Copy)]
pub struct HttpTimeouts {
    pub read_timeout: Duration,
}

impl Default for HttpTimeouts {
    fn default() -> Self {
        Self {
            read_timeout: crate::consts::READ_TIMEOUT,
        }
    }
}
