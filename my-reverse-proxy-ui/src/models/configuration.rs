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
    #[serde(default)]
    pub ssl_certs: Vec<SslCertificateInfoModel>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SslCertificateInfoModel {
    pub id: String,
    pub expires_at: String,
    pub days_left: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PortConfigurationModel {
    pub port: u16,
    /// Set for unix-socket listeners; `port` is 0 in that case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unix_socket: Option<String>,
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
    #[serde(default)]
    pub inbound_connections: i64,
    /// IP(s) the endpoint domain currently resolves to, shown next to the host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_ip: Option<String>,
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
    #[serde(default)]
    pub debug: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_alive: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool_total: Option<usize>,
    /// 0 = unknown, 1 = ok, 2 = error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_status: Option<i64>,
}

/// Mirror of `GatewayServerStatus` — the local server-side gateway endpoint and
/// the connections currently established to it by remote gateway clients.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GatewayServerStatusModel {
    #[serde(default)]
    pub connections: Vec<GatewayConnectionModel>,
}

/// Mirror of `GatewayClientStatus` — one configured outbound gateway link and
/// its live connection(s) to the remote gateway server.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GatewayClientStatusModel {
    pub name: String,
    #[serde(default)]
    pub connections: Vec<GatewayConnectionModel>,
}

/// Mirror of `GatewayConnection` — a single live gateway TCP connection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GatewayConnectionModel {
    pub name: String,
    pub forward_connections: usize,
    pub proxy_connections: usize,
    /// Active forward / proxy connections grouped by remote target (route -> count).
    #[serde(default)]
    pub forward_routes: HashMap<String, usize>,
    #[serde(default)]
    pub proxy_routes: HashMap<String, usize>,
    pub ping_time: String,
    pub is_incoming_forward_connection_allowed: bool,
    #[serde(default)]
    pub in_history: Vec<usize>,
    #[serde(default)]
    pub out_history: Vec<usize>,
    pub timestamp: String,
}
