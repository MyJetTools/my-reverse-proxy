use serde::*;

use super::ModifyHttpHeadersSettings;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EndpointTemplateSettings {
    pub ssl_certificate: Option<String>,
    pub client_certificate_ca: Option<String>,
    pub google_auth: Option<String>,
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
    pub whitelisted_ip: Option<String>,
}
