use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

/// Mirror of `CurrentConfigurationHttpModel` returned by
/// `GET /api/configuration/Current` in `my-reverse-proxy`. Keep field names
/// in sync with the server contract.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CurrentConfigurationModel {
    pub ports: Vec<PortConfigurationModel>,
    pub users: BTreeMap<String, Vec<String>>,
    pub ip_lists: BTreeMap<String, Vec<String>>,
    pub errors: BTreeMap<String, String>,
    pub remote_connections: HashMap<String, usize>,
    pub gateway_server: Option<GatewayServerStatusModel>,
    #[serde(default)]
    pub gateway_clients: Vec<GatewayClientStatusModel>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PortConfigurationModel {
    pub port: u16,
    #[serde(rename = "type")]
    pub r#type: String,
    pub endpoints: Vec<HttpEndpointInfoModel>,
    #[serde(default)]
    pub inbound_connections: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HttpEndpointInfoModel {
    pub host: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub locations: Vec<HttpProxyPassLocationModel>,
    pub debug: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_list: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_user_list_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssl_cert_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_cert_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub g_auth: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HttpProxyPassLocationModel {
    pub path: String,
    pub to: String,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_kind: Option<String>,
    pub location_id: i64,
    pub id_string: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_alive: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_total: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GatewayServerStatusModel {
    #[serde(default)]
    pub clients: Vec<GatewayServerClientModel>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GatewayServerClientModel {
    pub id: String,
    pub addr: String,
    pub since: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GatewayClientStatusModel {
    pub id: String,
    pub remote: String,
    pub status: String,
}
