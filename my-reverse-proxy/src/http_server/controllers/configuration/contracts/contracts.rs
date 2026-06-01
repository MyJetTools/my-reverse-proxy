use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use my_http_server::macros::MyHttpObjectStructure;
use serde::*;

use crate::configurations::*;
use crate::upstream_status::UpstreamStatus;

use super::*;

#[derive(MyHttpObjectStructure, Serialize)]
pub struct CurrentConfigurationHttpModel {
    pub ports: Vec<PortConfigurationHttpModel>,
    pub users: BTreeMap<String, Vec<String>>,
    pub ip_lists: BTreeMap<String, Vec<String>>,
    pub errors: BTreeMap<String, String>,
    pub remote_connections: HashMap<String, usize>,
    pub ssl_certs: Vec<SslCertificateInfoModel>,
    pub gateway_server: Option<GatewayServerStatus>,
    pub gateway_clients: Vec<GatewayClientStatus>,
}

#[derive(MyHttpObjectStructure, Serialize)]
pub struct SslCertificateInfoModel {
    pub id: String,
    pub expires_at: String,
    pub days_left: i64,
}

impl CurrentConfigurationHttpModel {
    pub async fn new() -> Self {
        let (mut ports, ip_lists, errors) = crate::app::APP_CTX
            .current_configuration
            .get(|config| {
                let mut ports = Vec::new();

                for (port, listen_port_config) in &config.listen_tcp_endpoints {
                    let item = PortConfigurationHttpModel::new(*port, listen_port_config);
                    ports.push(item);
                }

                for (path, listen_config) in &config.listen_unix_socket_endpoints {
                    let item = PortConfigurationHttpModel::new_unix(path.as_str(), listen_config);
                    ports.push(item);
                }

                let ip_list = config
                    .white_list_ip_list
                    .get_all(|itm| itm.to_list_of_string());

                (ports, ip_list, config.error_configurations.clone())
            })
            .await;

        let mut users = BTreeMap::new();

        {
            let users_access = crate::app::APP_CTX.allowed_users_list.data.read().await;

            for (id, list) in users_access.iter() {
                users.insert(id.clone(), list.iter().cloned().collect());
            }
        }

        ports.sort_by(|a, b| a.port.cmp(&b.port));

        let ssl_certs = collect_ssl_certs().await;

        let mut remote_connections = HashMap::new();

        crate::app::APP_CTX
            .http_over_ssh_clients_pool
            .fill_connections_amount(&mut remote_connections);

        crate::app::APP_CTX
            .http2_over_ssh_clients_pool
            .fill_connections_amount(&mut remote_connections);

        for (name, ready, _total) in crate::app::APP_CTX.h1_tcp_pools.snapshot() {
            remote_connections.insert(name, ready);
        }
        for (name, ready, _total) in crate::app::APP_CTX.h1_tls_pools.snapshot() {
            remote_connections.insert(name, ready);
        }
        for (name, ready, _total) in crate::app::APP_CTX.h1_uds_pools.snapshot() {
            remote_connections.insert(name, ready);
        }
        for (name, ready, _total) in crate::app::APP_CTX.h2_tcp_pools.snapshot() {
            remote_connections.insert(name, ready);
        }
        for (name, ready, _total) in crate::app::APP_CTX.h2_tls_pools.snapshot() {
            remote_connections.insert(name, ready);
        }
        for (name, ready, _total) in crate::app::APP_CTX.h2_uds_pools.snapshot() {
            remote_connections.insert(name, ready);
        }

        Self {
            ports,
            users,
            ip_lists,
            errors,
            remote_connections,
            ssl_certs,
            gateway_server: GatewayServerStatus::new().await,
            gateway_clients: GatewayClientStatus::new().await,
        }
    }
}

async fn collect_ssl_certs() -> Vec<SslCertificateInfoModel> {
    use rust_extensions::date_time::DateTimeAsMicroseconds;

    let now = DateTimeAsMicroseconds::now();
    crate::app::APP_CTX
        .ssl_certificates_cache
        .read(|cache| {
            cache
                .ssl_certs
                .get_list()
                .into_iter()
                .map(|(id, holder)| {
                    let expires = holder.ssl_cert.get_cert_info().expires;
                    let days_left = expires.duration_since(now).get_full_seconds() / 86_400;
                    SslCertificateInfoModel {
                        id,
                        expires_at: expires.to_rfc3339(),
                        days_left,
                    }
                })
                .collect()
        })
        .await
}

#[derive(MyHttpObjectStructure, Serialize)]
pub struct PortConfigurationHttpModel {
    pub port: u16,
    // Set for unix-socket listeners (host key starting with `/` or `~/`);
    // `port` is 0 in that case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unix_socket: Option<String>,
    pub r#type: String,
    pub endpoints: Vec<HttpEndpointInfoModel>,
    pub inbound_connections: i64,
}

impl PortConfigurationHttpModel {
    pub fn new(port: u16, listen_config: &ListenConfiguration) -> Self {
        let (r#type, endpoints) = build_type_and_endpoints(listen_config);

        // Live count of inbound TCP connections currently open on this port —
        // maintained by `connection_by_port.inc()/dec()` calls in the listener
        // accept paths (see `tcp_listener::http`, `http2`, `https`).
        let inbound_connections =
            crate::app::APP_CTX.metrics.get(|m| m.connection_by_port.get(&port)) as i64;

        Self {
            port,
            unix_socket: None,
            r#type,
            endpoints,
            inbound_connections,
        }
    }

    pub fn new_unix(path: &str, listen_config: &ListenConfiguration) -> Self {
        let (r#type, endpoints) = build_type_and_endpoints(listen_config);

        Self {
            port: 0,
            unix_socket: Some(path.to_string()),
            r#type,
            endpoints,
            // No per-port connection metric for unix sockets; per-endpoint
            // counts are still surfaced inside `endpoints`.
            inbound_connections: 0,
        }
    }
}

fn build_type_and_endpoints(
    listen_config: &ListenConfiguration,
) -> (String, Vec<HttpEndpointInfoModel>) {
    let mut endpoints = Vec::new();

    let r#type = match listen_config {
        ListenConfiguration::Http(config) => {
            for endpoint in &config.endpoints {
                endpoints.push(HttpEndpointInfoModel::from_http_endpoint(endpoint))
            }
            config.listen_endpoint_type.as_str()
        }
        ListenConfiguration::Tcp(config) => {
            endpoints.push(HttpEndpointInfoModel::from_tcp_config(config.as_ref()));
            "tcp"
        }
        ListenConfiguration::Mcp(config) => {
            for endpoint in &config.endpoints {
                endpoints.push(HttpEndpointInfoModel::from_http_endpoint(endpoint))
            }
            config.listen_endpoint_type.as_str()
        }
    };

    (r#type.to_string(), endpoints)
}

#[derive(MyHttpObjectStructure, Serialize)]
pub struct HttpEndpointInfoModel {
    pub host: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub locations: Vec<HttpProxyPassLocationModel>,
    pub debug: bool,
    pub inbound_connections: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_user_list_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssl_cert_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_cert_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub g_auth: Option<String>,
}

impl HttpEndpointInfoModel {
    pub fn from_http_endpoint(endpoint: &HttpEndpointInfo) -> Self {
        let host_key = endpoint.host_endpoint.as_str().to_string();
        let inbound_connections = crate::app::APP_CTX
            .metrics
            .get(|m| m.connection_by_endpoint.get(&host_key))
            as i64;

        Self {
            host: host_key,
            r#type: endpoint.listen_endpoint_type.as_str().to_string(),
            debug: endpoint.debug,
            allowed_user_list_id: endpoint.allowed_user_list_id.clone(),
            ip_list: endpoint.whitelisted_ip_list_id.clone(),
            ssl_cert_id: endpoint
                .ssl_certificate_id
                .as_ref()
                .map(|itm| itm.as_str().to_string()),
            client_cert_id: endpoint
                .client_certificate_id
                .as_ref()
                .map(|itm| itm.as_str().to_string()),
            g_auth: endpoint.g_auth.clone(),
            inbound_connections,
            locations: endpoint
                .locations
                .iter()
                .map(|itm| HttpProxyPassLocationModel::new(itm))
                .collect(),
        }
    }

    pub fn from_tcp_config(config: &TcpEndpointHostConfig) -> Self {
        Self {
            host: config.host_endpoint.as_str().to_string(),
            r#type: "tcp".to_string(),
            debug: config.debug,
            ip_list: config.ip_white_list_id.clone(),
            inbound_connections: 0,
            locations: vec![HttpProxyPassLocationModel {
                path: "".to_string(),
                to: config.remote_host.to_string(),
                r#type: "tcp".to_string(),
                remote_kind: Some(config.remote_host.kind_as_str().to_string()),
                location_id: 0,
                id_string: String::new(),
                pool_alive: None,
                pool_total: None,
                last_status: None,
            }],
            allowed_user_list_id: None,
            ssl_cert_id: None,
            client_cert_id: None,
            g_auth: None,
        }
    }

    /*
    pub fn from_mcp_endpoint(config: &McpEndpointHostConfig) -> Self {
        Self {
            host: config.host_endpoint.as_str().to_string(),
            r#type: "mcp".to_string(),
            debug: config.debug,
            ip_list: None,
            locations: vec![HttpProxyPassLocationModel {
                path: "".to_string(),
                to: config.remote_host.to_string(),
                r#type: "mcp".to_string(),
            }],
            allowed_user_list_id: None,
            ssl_cert_id: None,
            client_cert_id: None,
            g_auth: None,
        }
    }
     */
}

#[derive(MyHttpObjectStructure, Serialize, Debug)]
pub struct HttpProxyPassLocationModel {
    pub path: String,
    pub to: String,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_kind: Option<String>,
    pub location_id: i64,
    pub id_string: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool_alive: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool_total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<i64>,
}

impl HttpProxyPassLocationModel {
    pub fn new(src: &Arc<ProxyPassLocationConfig>) -> Self {
        let (pool_alive, pool_total, last_status) = match lookup_pool_state(src.id) {
            Some((a, t, s)) => (Some(a), Some(t), Some(s.as_u8() as i64)),
            None => (None, None, None),
        };

        let result = Self {
            path: src.path.to_string(),
            to: src.get_proxy_pass_to_as_string(),
            r#type: src.proxy_pass_to.get_type_as_str().to_string(),
            remote_kind: src
                .proxy_pass_to
                .remote_endpoint_kind()
                .map(|s| s.to_string()),
            location_id: src.id,
            id_string: src.id_string.clone(),
            pool_alive,
            pool_total,
            last_status,
        };

        result
    }
}

fn lookup_pool_state(location_id: i64) -> Option<(usize, usize, UpstreamStatus)> {
    let ctx = &crate::app::APP_CTX;
    if let Some(p) = ctx.h1_tcp_pools.get(location_id) {
        return Some((p.alive_count(), p.total_count(), p.last_status()));
    }
    if let Some(p) = ctx.h1_tls_pools.get(location_id) {
        return Some((p.alive_count(), p.total_count(), p.last_status()));
    }
    if let Some(p) = ctx.h1_uds_pools.get(location_id) {
        return Some((p.alive_count(), p.total_count(), p.last_status()));
    }
    if let Some(p) = ctx.h2_tcp_pools.get(location_id) {
        return Some((p.alive_count(), p.total_count(), p.last_status()));
    }
    if let Some(p) = ctx.h2_tls_pools.get(location_id) {
        return Some((p.alive_count(), p.total_count(), p.last_status()));
    }
    if let Some(p) = ctx.h2_uds_pools.get(location_id) {
        return Some((p.alive_count(), p.total_count(), p.last_status()));
    }
    None
}
