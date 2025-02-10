use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use my_http_server::macros::MyHttpObjectStructure;
use serde::*;

use crate::{app::AppContext, configurations::*};

use super::*;

#[derive(MyHttpObjectStructure, Serialize)]
pub struct CurrentConfigurationHttpModel {
    pub ports: Vec<PortConfigurationHttpModel>,
    pub users: BTreeMap<String, Vec<String>>,
    pub ip_lists: BTreeMap<String, Vec<String>>,
    pub errors: BTreeMap<String, String>,
    pub remote_connections: HashMap<String, usize>,
    pub gateway_server: Option<GatewayServerStatus>,
}

impl CurrentConfigurationHttpModel {
    pub async fn new(app: &AppContext) -> Self {
        let (mut ports, ip_lists, errors) = app
            .current_configuration
            .get(|config| {
                let mut ports = Vec::new();

                for (port, listen_port_config) in &config.listen_endpoints {
                    let item = PortConfigurationHttpModel::new(*port, listen_port_config);
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
            let users_access = app.allowed_users_list.data.read().await;

            for (id, list) in users_access.iter() {
                users.insert(id.clone(), list.iter().cloned().collect());
            }
        }

        ports.sort_by(|a, b| a.port.cmp(&b.port));

        let mut remote_connections = HashMap::new();

        app.http_clients_pool
            .fill_connections_amount(&mut remote_connections)
            .await;

        app.http2_clients_pool
            .fill_connections_amount(&mut remote_connections)
            .await;

        app.https_clients_pool
            .fill_connections_amount(&mut remote_connections)
            .await;

        app.http_over_ssh_clients_pool
            .fill_connections_amount(&mut remote_connections)
            .await;

        app.http2_over_ssh_clients_pool
            .fill_connections_amount(&mut remote_connections)
            .await;

        Self {
            ports,
            users,
            ip_lists,
            errors,
            remote_connections,
            gateway_server: GatewayServerStatus::new(app).await,
        }
    }
}

#[derive(MyHttpObjectStructure, Serialize)]
pub struct PortConfigurationHttpModel {
    pub port: u16,
    pub r#type: String,
    pub endpoints: Vec<HttpEndpointInfoModel>,
}

impl PortConfigurationHttpModel {
    pub fn new(port: u16, listen_config: &ListenConfiguration) -> Self {
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
        };

        Self {
            port,
            r#type: r#type.to_string(),
            endpoints,
        }
    }
}

#[derive(MyHttpObjectStructure, Serialize)]
pub struct HttpEndpointInfoModel {
    pub host: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub locations: Vec<HttpProxyPassLocationModel>,
    pub debug: bool,
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
        Self {
            host: endpoint.host_endpoint.as_str().to_string(),
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
            locations: vec![HttpProxyPassLocationModel {
                path: "".to_string(),
                to: config.remote_host.to_string(),
                r#type: "tcp".to_string(),
            }],
            allowed_user_list_id: None,
            ssl_cert_id: None,
            client_cert_id: None,
            g_auth: None,
        }
    }
}

#[derive(MyHttpObjectStructure, Serialize, Debug)]
pub struct HttpProxyPassLocationModel {
    pub path: String,
    pub to: String,
    #[serde(rename = "type")]
    pub r#type: String,
}

impl HttpProxyPassLocationModel {
    pub fn new(src: &Arc<ProxyPassLocationConfig>) -> Self {
        let result = Self {
            path: src.path.to_string(),
            to: src.get_proxy_pass_to_as_string(),
            r#type: src.proxy_pass_to.get_type_as_str().to_string(),
        };

        result
    }
}
