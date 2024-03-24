use std::net::SocketAddr;

use rust_extensions::{placeholders::PlaceholdersIterator, StrOrString};

use crate::populate_variable::{PLACEHOLDER_CLOSE_TOKEN, PLACEHOLDER_OPEN_TOKEN};

use super::HostPort;

pub struct SourceHttpData {
    pub is_https: bool,
    pub host: Option<String>,
    pub socket_addr: SocketAddr,
    pub client_cert_cn: Option<String>,
}

impl SourceHttpData {
    pub fn new(socket_addr: SocketAddr) -> Self {
        Self {
            is_https: false,
            host: None,
            socket_addr,
            client_cert_cn: None,
        }
    }

    pub fn populate_value<'s>(&self, value: &'s str, req_uri: &HostPort<'s>) -> StrOrString<'s> {
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
                            if let Some(host) = &self.host {
                                result.push_str(host);
                            }
                        }
                        "ENDPOINT_IP" => {
                            result.push_str(format!("{}", self.socket_addr.ip()).as_str());
                        }

                        "PATH_AND_QUERY" => {
                            if let Some(value) = req_uri.get_path_and_query() {
                                result.push_str(value);
                            }
                        }

                        "HOST_PORT" => {
                            if let Some(host) = req_uri.get_host() {
                                result.push_str(host);
                                if let Some(port) = req_uri.get_port_opt() {
                                    result.push(':');
                                    result.push_str(port.to_string().as_str());
                                }
                            }
                        }

                        "CLIENT_CERT_CN" => {
                            if let Some(value) = self.client_cert_cn.as_ref() {
                                result.push_str(value.as_str());
                            }
                        }

                        "ENDPOINT_SCHEMA" => {
                            if self.is_https {
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
