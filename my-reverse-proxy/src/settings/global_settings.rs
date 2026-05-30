use serde::*;

use super::{ConnectionsSettings, ModifyHttpHeadersSettings, TimeoutsSettings};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GlobalSettings {
    pub connection_settings: Option<ConnectionsSettings>,
    pub all_http_endpoints: Option<AllHttpEndpointsGlobalSettings>,
    pub show_error_description_on_error_page: Option<bool>,
    pub http_control_port: Option<u16>,
    pub default_h2_livness_url: Option<String>,
    /// How often the (single, global) supervisor sweeps every H1/H2 upstream
    /// pool, in milliseconds. Defaults to 10000 (10s). Global-only — a single
    /// timer drives every pool, so it is not part of the cascade.
    pub pool_supervisor_interval: Option<u64>,
    /// Lowest level of the timeout cascade — overridden by the endpoint, then
    /// the location.
    #[serde(flatten)]
    pub timeouts: TimeoutsSettings,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AllHttpEndpointsGlobalSettings {
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
}
