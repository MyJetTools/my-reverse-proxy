use serde::*;

use super::{ConnectionsSettings, ModifyHttpHeadersSettings};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GlobalSettings {
    pub connection_settings: Option<ConnectionsSettings>,
    pub all_http_endpoints: Option<AllHttpEndpointsGlobalSettings>,
    pub show_error_description_on_error_page: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AllHttpEndpointsGlobalSettings {
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
}
