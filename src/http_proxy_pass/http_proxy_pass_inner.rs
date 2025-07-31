use rust_common::placeholders::*;
use rust_extensions::StrOrString;

use super::{HostPort, HttpListenPortInfo, HttpProxyPassIdentity, ProxyPassLocations};

pub struct HttpProxyPassInner {
    pub identity: HttpProxyPassIdentity,
    pub locations: ProxyPassLocations,
    pub http_listen_port_info: HttpListenPortInfo,
}

impl HttpProxyPassInner {
    pub fn new(
        identity: HttpProxyPassIdentity,
        locations: ProxyPassLocations,
        http_listen_port_info: HttpListenPortInfo,
    ) -> Self {
        Self {
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
        if !value.contains("${") {
            return value.into();
        }

        let mut result = String::new();

        for token in PlaceholdersIterator::new(value, "${", "}") {
            match token {
                ContentToken::Text(text) => result.push_str(text),
                ContentToken::Placeholder(placeholder) => match placeholder {
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
                        if self.http_listen_port_info.endpoint_type.is_https() {
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
                },
            }
        }

        result.into()
    }
}
