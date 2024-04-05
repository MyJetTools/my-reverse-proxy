use std::sync::Arc;

use my_http_server::macros::MyHttpObjectStructure;
use serde::*;

use crate::app_configuration::{AppConfiguration, HttpEndpointInfo, ProxyPassLocationConfig};

#[derive(MyHttpObjectStructure, Serialize)]
pub struct CurrentConfigurationHttpModel {
    pub http: Vec<HttpConfigurationHttpModel>,
}

impl CurrentConfigurationHttpModel {
    pub fn new(config: &AppConfiguration) -> Self {
        let mut http = Vec::new();

        for (port, listen_port_config) in &config.http_endpoints {
            http.push(HttpConfigurationHttpModel::new(
                *port,
                listen_port_config.endpoint_info.as_slice(),
            ))
        }

        Self { http }
    }
}

#[derive(MyHttpObjectStructure, Serialize)]
pub struct HttpConfigurationHttpModel {
    pub port: u16,
    pub endpoints: Vec<HttpEndpointInfoModel>,
}

impl HttpConfigurationHttpModel {
    pub fn new(port: u16, endpoints: &[Arc<HttpEndpointInfo>]) -> Self {
        Self {
            port,
            endpoints: endpoints
                .iter()
                .map(|itm| HttpEndpointInfoModel::new(itm))
                .collect(),
        }
    }
}

#[derive(MyHttpObjectStructure, Serialize)]
pub struct HttpEndpointInfoModel {
    pub host: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub locations: Vec<HttpProxyPassLocationModel>,
}

impl HttpEndpointInfoModel {
    pub fn new(endpoint: &HttpEndpointInfo) -> Self {
        Self {
            host: endpoint.host_endpoint.as_str().to_string(),
            r#type: endpoint.http_type.to_str().to_string(),
            locations: endpoint
                .locations
                .iter()
                .map(|itm| HttpProxyPassLocationModel::new(itm))
                .collect(),
        }
    }
}

#[derive(MyHttpObjectStructure, Serialize)]
pub struct HttpProxyPassLocationModel {
    pub path: String,
    pub to: String,
}

impl HttpProxyPassLocationModel {
    pub fn new(src: &Arc<ProxyPassLocationConfig>) -> Self {
        Self {
            path: src.path.to_string(),
            to: src.get_proxy_pass_to_as_string(),
        }
    }
}
