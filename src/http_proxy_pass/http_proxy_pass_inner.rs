use rust_common::placeholders::*;
use rust_extensions::StrOrString;

use crate::types::{ConnectionIp, HttpRequestReader};

use super::{HttpListenPortInfo, HttpProxyPassIdentity, ProxyPassLocations};

pub struct HttpProxyPassInner {
    pub identity: Option<HttpProxyPassIdentity>,
    pub locations: ProxyPassLocations,
    pub http_listen_port_info: HttpListenPortInfo,
    pub connection_ip: ConnectionIp,
}

impl HttpProxyPassInner {
    pub fn new(
        identity: Option<HttpProxyPassIdentity>,
        locations: ProxyPassLocations,
        http_listen_port_info: HttpListenPortInfo,
        connection_ip: ConnectionIp,
    ) -> Self {
        Self {
            identity,
            locations,
            http_listen_port_info,
            connection_ip,
        }
    }

    pub fn populate_value<'s>(
        &'s self,
        value: &'s str,
        req: &impl HttpRequestReader,
    ) -> StrOrString<'s> {
        if !value.contains("${") {
            return value.into();
        }

        let mut result = String::new();

        for token in PlaceholdersIterator::new(value, "${", "}") {
            match token {
                ContentToken::Text(text) => result.push_str(text),
                ContentToken::Placeholder(placeholder) => match placeholder {
                    "ENDPOINT_IP" => {
                        if let Some(ip) = self.connection_ip.get_ip_addr() {
                            result.push_str(format!("{}", ip).as_str());
                        }
                    }

                    "PATH_AND_QUERY" => {
                        if let Some(path_and_query) = req.get_path_and_query() {
                            result.push_str(path_and_query);
                        }
                    }

                    "HOST_PORT" => {
                        if let Some(host) = req.get_host() {
                            result.push_str(host);
                        }
                    }

                    "CLIENT_CERT_CN" => {
                        if let Some(identity) = self.identity.as_ref() {
                            result.push_str(identity.as_str());
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
