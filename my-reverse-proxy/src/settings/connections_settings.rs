use std::time::Duration;

use serde::*;

const DEFAULT_BUFFER_SIZE: usize = 1024 * 512;
const DEFAULT_CONNECT_TO_REMOTE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnectionsSettings {
    pub buffer_size: Option<String>,
    /// Connect timeout for `type: tcp` port-forward endpoints, in milliseconds.
    pub connect_to_remote_timeout: Option<u64>,
    pub session_key: Option<String>,
}

impl ConnectionsSettings {
    pub fn get_buffer_size(&self) -> usize {
        match &self.buffer_size {
            Some(buffer_size) => {
                super::parse_buffer_size(buffer_size).unwrap_or_else(|err| panic!("{}", err))
            }
            None => DEFAULT_BUFFER_SIZE,
        }
    }

    pub fn get_connect_to_remote_timeout(&self) -> Duration {
        match self.connect_to_remote_timeout {
            Some(timeout) => Duration::from_millis(timeout),
            None => DEFAULT_CONNECT_TO_REMOTE_TIMEOUT,
        }
    }
}

pub struct ConnectionsSettingsModel {
    pub buffer_size: usize,
    pub remote_connect_timeout: Duration,
}

impl ConnectionsSettingsModel {
    pub fn default() -> Self {
        Self {
            buffer_size: DEFAULT_BUFFER_SIZE,
            remote_connect_timeout: DEFAULT_CONNECT_TO_REMOTE_TIMEOUT,
        }
    }
    pub fn new(src: &ConnectionsSettings) -> Self {
        Self {
            buffer_size: src.get_buffer_size(),
            remote_connect_timeout: src.get_connect_to_remote_timeout(),
        }
    }
}
