use std::time::Duration;

#[derive(Clone, Copy)]
pub struct HttpTimeouts {
    pub connect_timeout: Duration,
    pub write_timeout: Duration,
    pub read_timeout: Duration,
}

impl Default for HttpTimeouts {
    fn default() -> Self {
        Self {
            connect_timeout: crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT,
            write_timeout: crate::consts::WRITE_TIMEOUT,
            read_timeout: crate::consts::READ_TIMEOUT,
        }
    }
}
