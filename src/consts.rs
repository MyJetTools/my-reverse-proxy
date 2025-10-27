use std::time::Duration;

pub const DEFAULT_HTTP_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

pub const DEFAULT_HTTP_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub const READ_CAPACITY: usize = 1024 * 1024;

pub const READ_TIMEOUT: Duration = Duration::from_secs(30);
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

pub const HTTP_CR_LF: &[u8] = b"\r\n";
