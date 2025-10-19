use serde::*;

use super::*;

const HTTP1_ENDPOINT_TYPE: &str = "http";
const HTTP2_ENDPOINT_TYPE: &str = "http2";

const HTTPS1_ENDPOINT_TYPE: &str = "https";
const HTTPS2_ENDPOINT_TYPE: &str = "https2";

const TCP_ENDPOINT_TYPE: &str = "tcp";

const MCP_ENDPOINT_TYPE: &str = "mcp";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EndpointSettings {
    #[serde(rename = "type")]
    pub endpoint_type: String,
    pub ssl_certificate: Option<String>,
    pub client_certificate_ca: Option<String>,
    pub google_auth: Option<String>,
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
    pub debug: Option<bool>,
    pub whitelisted_ip: Option<String>,
    pub template_id: Option<String>,
    pub allowed_users: Option<String>,
}

impl EndpointSettings {
    pub fn get_debug(&self) -> bool {
        self.debug.unwrap_or(false)
    }

    pub fn get_endpoint_type(&self) -> Result<EndpointTypeSettings, String> {
        let result = match self.endpoint_type.as_str() {
            HTTP1_ENDPOINT_TYPE => EndpointTypeSettings::Http1,
            HTTP2_ENDPOINT_TYPE => EndpointTypeSettings::Http2,
            HTTPS1_ENDPOINT_TYPE => EndpointTypeSettings::Https1,
            "http1" => EndpointTypeSettings::Https1,
            HTTPS2_ENDPOINT_TYPE => EndpointTypeSettings::Https2,
            TCP_ENDPOINT_TYPE => EndpointTypeSettings::Tcp,
            MCP_ENDPOINT_TYPE => EndpointTypeSettings::Mcp,
            _ => return Err(format!("Unknown endpoint type: '{}'", self.endpoint_type)),
        };

        Ok(result)
    }
}
