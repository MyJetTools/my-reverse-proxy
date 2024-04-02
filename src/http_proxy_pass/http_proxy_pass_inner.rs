use std::{sync::Arc, time::Duration};

use rust_extensions::{placeholders::PlaceholdersIterator, StrOrString};

use crate::{
    app_configuration::{HttpEndpointInfo, HttpListenPortInfo},
    populate_variable::{PLACEHOLDER_CLOSE_TOKEN, PLACEHOLDER_OPEN_TOKEN},
};

use super::{HostPort, HttpProxyPassIdentity, ProxyPassLocations};

#[derive(Debug)]
pub enum RetryType {
    Retry(Option<Duration>),
    NoRetry,
}

pub struct HttpProxyPassInner {
    pub endpoint_info: Arc<HttpEndpointInfo>,
    pub disposed: bool,
    pub identity: HttpProxyPassIdentity,
    pub locations: ProxyPassLocations,
    pub http_listen_port_info: HttpListenPortInfo,
}

impl HttpProxyPassInner {
    pub fn new(
        endpoint_info: Arc<HttpEndpointInfo>,
        identity: HttpProxyPassIdentity,
        locations: ProxyPassLocations,
        http_listen_port_info: HttpListenPortInfo,
    ) -> Self {
        Self {
            endpoint_info,
            disposed: false,
            identity,
            locations,
            http_listen_port_info,
        }
    }

    pub fn populate_value<'s, THostPort: HostPort + Send + Sync + 'static>(
        &'s self,
        value: &'s str,
        req_host_port: &THostPort,
    ) -> StrOrString<'s> {
        if !value.contains(PLACEHOLDER_OPEN_TOKEN) {
            return value.into();
        }

        let mut result = String::new();

        for token in
            PlaceholdersIterator::new(value, PLACEHOLDER_OPEN_TOKEN, PLACEHOLDER_CLOSE_TOKEN)
        {
            match token {
                rust_extensions::placeholders::ContentToken::Text(text) => result.push_str(text),
                rust_extensions::placeholders::ContentToken::Placeholder(placeholder) => {
                    match placeholder {
                        "HOST" => {
                            if let Some(host) = req_host_port.get_host() {
                                result.push_str(host);
                            }
                        }
                        "ENDPOINT_IP" => {
                            result.push_str(
                                format!("{}", self.http_listen_port_info.socket_addr.ip()).as_str(),
                            );
                        }

                        "PATH_AND_QUERY" => {
                            if let Some(value) = req_host_port.get_path_and_query() {
                                result.push_str(value);
                            }
                        }

                        "HOST_PORT" => {
                            if let Some(host) = req_host_port.get_host() {
                                result.push_str(host);
                                if let Some(port) = req_host_port.get_port() {
                                    result.push(':');
                                    result.push_str(port.to_string().as_str());
                                }
                            }
                        }

                        "CLIENT_CERT_CN" => {
                            if let Some(value) = self.identity.get_identity() {
                                result.push_str(value);
                            }
                        }

                        "ENDPOINT_SCHEMA" => {
                            if self.http_listen_port_info.http_type.is_https() {
                                result.push_str("https");
                            } else {
                                result.push_str("http");
                            }
                        }
                        _ => {
                            if let Ok(value) = std::env::var(placeholder) {
                                result.push_str(&value);
                            }
                        }
                    }
                }
            }
        }

        result.into()
    }
}
