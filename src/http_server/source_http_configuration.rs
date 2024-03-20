use std::net::SocketAddr;

use rust_extensions::{placeholders::PlaceholdersIterator, StrOrString};

pub struct SourceHttpConfiguration {
    pub is_https: bool,
    pub host: Option<String>,
    pub socket_addr: SocketAddr,
    pub client_cert_cn: Option<String>,
}

impl SourceHttpConfiguration {
    pub fn new(socket_addr: SocketAddr) -> Self {
        Self {
            is_https: false,
            host: None,
            socket_addr,
            client_cert_cn: None,
        }
    }

    pub fn populate_value<'s>(&self, value: &'s str) -> StrOrString<'s> {
        if !value.contains("${") {
            return value.into();
        }

        let mut result = String::new();

        for token in PlaceholdersIterator::new(value) {
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

                        "ENDPOINT_SCHEMA" => {
                            if self.is_https {
                                result.push_str("https");
                            } else {
                                result.push_str("http");
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        result.into()
    }
}
