use std::{collections::HashMap, str::FromStr};

use serde::*;

use super::{EndpointType, LocationSettings, SslCertificateId};

const HTTP1_ENDPOINT_TYPE: &str = "http";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EndpointSettings {
    #[serde(rename = "type")]
    pub endpoint_type: String,
    pub ssl_certificate: Option<String>,
    pub client_certificate_ca: Option<String>,
}

impl EndpointSettings {
    pub fn get_type(
        &self,
        host: &str,
        locations: &[LocationSettings],
        variables: &Option<HashMap<String, String>>,
    ) -> EndpointType {
        match self.endpoint_type.as_str() {
            HTTP1_ENDPOINT_TYPE => EndpointType::Http1,
            "https" => {
                if let Some(ssl_certificate) = &self.ssl_certificate {
                    EndpointType::Https {
                        ssl_id: SslCertificateId::new(ssl_certificate.to_string()),
                        client_ca_id: self
                            .client_certificate_ca
                            .as_ref()
                            .map(|x| SslCertificateId::new(x.to_string())),
                    }
                } else {
                    panic!("Host '{}' has https location without ssl certificate", host);
                }
            }
            "http2" => EndpointType::Http2,
            "tcp" => {
                if locations.len() != 1 {
                    panic!(
                        "Tcp Host '{}' has {} locations to proxy_pass. Tcp Host must have 1 location",
                        host,
                        locations.len()
                    );
                }

                let proxy_pass_to = locations.get(0).unwrap().get_proxy_pass(variables);

                if proxy_pass_to.is_ssh() {
                    EndpointType::TcpOverSsh(proxy_pass_to.to_ssh_configuration())
                } else {
                    let remote_addr = std::net::SocketAddr::from_str(proxy_pass_to.as_str());

                    if remote_addr.is_err() {
                        panic!(
                            "Can not parse remote address: '{}' for tcp listen host {}",
                            proxy_pass_to.as_str(),
                            host
                        );
                    }

                    EndpointType::Tcp(remote_addr.unwrap())
                }
            }
            _ => panic!("Unknown location type: '{}'", self.endpoint_type),
        }
    }
}
