use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use hyper::Uri;
use serde::*;

use super::SshConfiguration;

pub const BUFFER_SIZE: usize = 1024 * 512;

pub enum ProxyPassRemoteEndpoint {
    Http(Uri),
    Ssh(SshConfiguration),
}

#[derive(my_settings_reader::SettingsModel, Serialize, Deserialize, Debug, Clone)]
pub struct SettingsModel {
    pub hosts: HashMap<String, Vec<Location>>,
}

impl SettingsReader {
    pub async fn get_configurations(&self, host: &str) -> Vec<(String, ProxyPassRemoteEndpoint)> {
        let mut result = Vec::new();
        let read_access = self.settings.read().await;

        for (settings_host, locations) in &read_access.hosts {
            if rust_extensions::str_utils::compare_strings_case_insensitive(settings_host, host) {
                for location in locations {
                    if location.location.is_none() {
                        panic!(
                            "Location is not defined for host: '{}' with proxy pass to '{}'",
                            host, location.proxy_pass_to
                        );
                    }

                    if location.proxy_pass_to.trim().starts_with("ssh") {
                        result.push((
                            location.location.as_ref().unwrap().to_string(),
                            ProxyPassRemoteEndpoint::Ssh(SshConfiguration::parse(
                                &location.proxy_pass_to,
                            )),
                        ));
                    } else {
                        result.push((
                            location.location.as_ref().unwrap().to_string(),
                            ProxyPassRemoteEndpoint::Http(
                                Uri::from_str(&location.proxy_pass_to).unwrap(),
                            ),
                        ));
                    }
                }
                break;
            }
        }

        result
    }

    pub async fn get_listen_ports(&self) -> BTreeMap<u16, EndpointType> {
        let read_access = self.settings.read().await;

        let mut result: BTreeMap<u16, EndpointType> = BTreeMap::new();

        for (host, locations) in &read_access.hosts {
            let host_port = host.split(':');

            let endpoint_type = check_and_get_endpoint_type(host, locations);

            match host_port.last().unwrap().parse::<u16>() {
                Ok(port) => {
                    if let Some(current_endpoint_type) = result.get(&port) {
                        if !current_endpoint_type.are_same(&endpoint_type) {
                            panic!(
                                "Host '{}' has different endpoint types {:?} and {:?} for the same port {}",
                                host,
                                endpoint_type,
                                current_endpoint_type,
                                port
                            );
                        }
                    }

                    result.insert(port, endpoint_type);
                }
                Err(_) => {
                    panic!("Can not read port from host: '{}'", host);
                }
            }
        }

        result
    }
}

#[derive(Debug)]
pub enum EndpointType {
    Http1,
    Tcp(std::net::SocketAddr),
}

impl EndpointType {
    pub fn as_u8(&self) -> u8 {
        match self {
            EndpointType::Http1 => 0,
            EndpointType::Tcp(_) => 1,
        }
    }

    pub fn are_same(&self, other: &EndpointType) -> bool {
        self.as_u8() == other.as_u8()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Location {
    pub location: Option<String>,
    pub proxy_pass_to: String,
    #[serde(rename = "type")]
    pub endpoint_type: String,
}

impl Location {
    pub fn get_endpoint_type(&self, host: &str) -> EndpointType {
        match self.endpoint_type.as_str() {
            "http1" => EndpointType::Http1,
            "tcp" => {
                let remote_addr = std::net::SocketAddr::from_str(self.proxy_pass_to.as_str());

                if remote_addr.is_err() {
                    panic!(
                        "Can not parse remote address: '{}' for tcp listen host {}",
                        self.proxy_pass_to, host
                    );
                }

                EndpointType::Tcp(remote_addr.unwrap())
            }
            _ => panic!("Unknown location type: '{}'", self.endpoint_type),
        }
    }
}

fn check_and_get_endpoint_type(host: &str, locations: &[Location]) -> EndpointType {
    let mut tcp_location = None;
    let mut http_count = 0;

    for location in locations {
        match location.get_endpoint_type(host) {
            EndpointType::Http1 => http_count += 1,
            EndpointType::Tcp(proxy_pass) => {
                if tcp_location.is_some() {
                    panic!(
                        "Host '{}' has more than one tcp location. It must be only single location",
                        host
                    );
                }

                tcp_location = Some(proxy_pass);
            }
        }
    }
    if tcp_location.is_some() && http_count > 0 {
        panic!("Host '{}' has both http and tcp locations", host);
    }

    if let Some(tcp_location) = tcp_location {
        return EndpointType::Tcp(tcp_location);
    }

    if http_count > 0 {
        return EndpointType::Http1;
    }

    panic!("Host '{}' has no locations", host);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::SettingsModel;

    #[test]
    fn test() {
        let mut hosts = HashMap::new();

        hosts.insert(
            "localhost:9000".to_string(),
            vec![super::Location {
                location: Some("/".to_owned()),
                proxy_pass_to: "https://www.google.com".to_owned(),
                endpoint_type: "http1".to_owned(),
            }],
        );

        let model = SettingsModel { hosts };

        let json = serde_yaml::to_string(&model).unwrap();

        println!("{}", json);
    }
}
