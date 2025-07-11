use std::time::Duration;

use serde::*;

use super::*;

pub enum LocationType {
    Http,
    Http2,
    Https1,
    Https2,
    Files,
    StaticContent,
    UnixSocketHttp,
    UnixSocketHttp2,
}

impl LocationType {
    pub fn detect_from_location_settings(
        location_settings: &LocationSettings,
    ) -> Result<Self, String> {
        match location_settings.proxy_pass_to.as_ref() {
            Some(src) => {
                if src.eq_ignore_ascii_case("static") {
                    return Ok(Self::StaticContent);
                }

                let mut splitted = src.split("->");

                let mut left_part = splitted.next().unwrap();

                if let Some(right_part) = splitted.next() {
                    left_part = right_part;
                }

                if left_part.starts_with("unix+http") {
                    return Ok(Self::UnixSocketHttp);
                };

                if left_part.starts_with("unix+http2") {
                    return Ok(Self::UnixSocketHttp2);
                };

                if left_part.starts_with("https") {
                    return Ok(Self::Https1);
                };

                if left_part.starts_with("http") {
                    return Ok(Self::Http);
                };

                Ok(Self::Files)
            }
            None => Ok(Self::StaticContent),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LocationSettings {
    pub path: Option<String>,
    pub proxy_pass_to: Option<String>,
    #[serde(rename = "type")]
    pub location_type: Option<String>,
    pub domain_name: Option<String>,
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
    pub default_file: Option<String>,
    pub status_code: Option<u16>,
    pub content_type: Option<String>,
    pub body: Option<String>,
    pub whitelisted_ip: Option<String>,
    pub compress: Option<bool>,
    pub connect_timeout: Option<u64>,
    pub request_timeout: Option<u64>,
    pub trace_payload: Option<bool>,
}

impl LocationSettings {
    pub fn get_location_type(&self) -> Result<Option<LocationType>, String> {
        if let Some(location_type) = self.location_type.as_ref() {
            match location_type.as_str() {
                "unix+http" => return Ok(LocationType::UnixSocketHttp.into()),
                "unix+http2" => return Ok(LocationType::UnixSocketHttp2.into()),
                "http" => return Ok(LocationType::Http.into()),
                "http2" => return Ok(LocationType::Http2.into()),
                "https" => return Ok(LocationType::Https1.into()),
                "https1" => return Ok(LocationType::Https1.into()),
                "https2" => return Ok(LocationType::Https2.into()),
                "static" => return Ok(LocationType::StaticContent.into()),
                _ => return Err(format!("Unknown remote location type {}", location_type)),
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_request_timeout(&self) -> Duration {
        if let Some(request_timeout) = self.request_timeout {
            return Duration::from_millis(request_timeout);
        }

        crate::consts::DEFAULT_HTTP_REQUEST_TIMEOUT
    }
    pub fn get_connect_timeout(&self) -> Duration {
        if let Some(connection_timeout) = self.connect_timeout {
            return Duration::from_millis(connection_timeout);
        }

        crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT
    }

    pub fn get_trace_payload(&self) -> bool {
        self.trace_payload.unwrap_or(false)
    }

    pub fn get_compress(&self) -> bool {
        self.trace_payload.unwrap_or(false)
    }
}
