use std::time::Duration;

use rust_extensions::duration_utils::parse_duration;
use serde::*;

const DEFAULT_BUFFER_SIZE: usize = 1024 * 512;
const DEFAULT_CONNECT_TO_REMOTE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnectionsSettings {
    pub buffer_size: Option<String>,
    pub connect_to_remote_timeout: Option<String>,
}

impl ConnectionsSettings {
    pub fn get_buffer_size(&self) -> usize {
        if self.buffer_size.is_none() {
            return DEFAULT_BUFFER_SIZE;
        }

        let buffer_size = self.buffer_size.as_ref().unwrap();
        if buffer_size.ends_with("Kb") {
            return buffer_size[0..buffer_size.len() - 2]
                .parse::<usize>()
                .unwrap()
                * 1024;
        }

        if buffer_size.ends_with("Mb") {
            return buffer_size[0..buffer_size.len() - 2]
                .parse::<usize>()
                .unwrap()
                * 1024
                * 1024;
        }

        match buffer_size.parse::<usize>() {
            Ok(size) => size,
            Err(err) => panic!(
                "Can not parse buffer size value: '{}'. Error: {}",
                buffer_size, err
            ),
        }
    }

    pub fn get_connect_to_remote_timeout(&self) -> Duration {
        match &self.connect_to_remote_timeout {
            Some(timeout) => {
                let result = parse_duration(timeout);

                if result.is_err() {
                    panic!("Can not parse remote connect timeout value: '{}'", timeout);
                }

                result.unwrap()
            }
            None => return DEFAULT_CONNECT_TO_REMOTE_TIMEOUT,
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
