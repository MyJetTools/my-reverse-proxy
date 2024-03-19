use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

use crate::http_server::HostPort;

use super::{ConnectionsSettings, ConnectionsSettingsModel, SshConfiguration};
use hyper::Uri;
use rust_extensions::{duration_utils::DurationExtensions, StrOrString};
use serde::*;

pub enum ProxyPassRemoteEndpoint {
    Http(Uri),
    Http2(Uri),
    Http1OverSsh(SshConfiguration),
    Http2OverSsh(SshConfiguration),
}

#[derive(my_settings_reader::SettingsModel, Serialize, Deserialize, Debug, Clone)]
pub struct SettingsModel {
    pub hosts: HashMap<String, Vec<Location>>,
    pub connection_settings: Option<ConnectionsSettings>,
    pub variables: Option<HashMap<String, String>>,
}

impl SettingsReader {
    pub async fn get_connections_settings(&self) -> ConnectionsSettingsModel {
        let read_access = self.settings.read().await;

        let result = match &read_access.connection_settings {
            Some(connection_settings) => ConnectionsSettingsModel::new(connection_settings),
            None => ConnectionsSettingsModel::default(),
        };

        println!(
            "Each connection is going to use buffer: {}",
            format_mem(result.buffer_size)
        );

        println!(
            "Timeout to connect to remote endpoint is: {}",
            result.remote_connect_timeout.format_to_string()
        );

        result
    }
    pub async fn get_configurations<'s, T>(
        &self,
        host_port: &HostPort<'s, T>,
    ) -> Vec<(String, ProxyPassRemoteEndpoint)> {
        let mut result = Vec::new();
        let read_access = self.settings.read().await;

        for (settings_host, locations) in &read_access.hosts {
            if host_port.is_my_host_port(settings_host) {
                for location in locations {
                    if location.location.is_none() {
                        panic!(
                            "Location is not defined for host: {} with proxy pass to '{}'",
                            settings_host, location.proxy_pass_to
                        );
                    }

                    if location.proxy_pass_to.trim().starts_with("ssh") {
                        result.push((
                            location.location.as_ref().unwrap().to_string(),
                            if location
                                .get_endpoint_type(settings_host, &read_access.variables)
                                .is_http_1()
                            {
                                ProxyPassRemoteEndpoint::Http1OverSsh(SshConfiguration::parse(
                                    &location.proxy_pass_to,
                                ))
                            } else {
                                ProxyPassRemoteEndpoint::Http2OverSsh(SshConfiguration::parse(
                                    &location.proxy_pass_to,
                                ))
                            },
                        ));
                    } else {
                        result.push((
                            location.location.as_ref().unwrap().to_string(),
                            if location
                                .get_endpoint_type(settings_host, &read_access.variables)
                                .is_http_1()
                            {
                                ProxyPassRemoteEndpoint::Http(
                                    Uri::from_str(&location.proxy_pass_to).unwrap(),
                                )
                            } else {
                                ProxyPassRemoteEndpoint::Http2(
                                    Uri::from_str(&location.proxy_pass_to).unwrap(),
                                )
                            },
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

            let endpoint_type =
                check_and_get_endpoint_type(host, locations, &read_access.variables);

            match host_port.last().unwrap().parse::<u16>() {
                Ok(port) => {
                    if let Some(current_endpoint_type) = result.get(&port) {
                        if !current_endpoint_type.type_is_the_same(&endpoint_type) {
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
    Http2,
    Tcp(std::net::SocketAddr),
    TcpOverSsh(SshConfiguration),
}

impl EndpointType {
    pub fn is_http_1(&self) -> bool {
        match self {
            EndpointType::Http1 => true,
            _ => false,
        }
    }
    pub fn as_u8(&self) -> u8 {
        match self {
            EndpointType::Http1 => 0,
            EndpointType::Http2 => 1,
            EndpointType::Tcp(_) => 2,
            EndpointType::TcpOverSsh(_) => 3,
        }
    }

    pub fn type_is_the_same(&self, other: &EndpointType) -> bool {
        self.as_u8() == other.as_u8()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Location {
    pub location: Option<String>,
    proxy_pass_to: String,
    #[serde(rename = "type")]
    pub endpoint_type: String,
}

impl Location {
    pub fn get_proxy_pass<'s>(
        &'s self,
        variables: &Option<HashMap<String, String>>,
    ) -> StrOrString<'s> {
        super::populate_variable(self.proxy_pass_to.as_str(), variables)
    }

    /*
       pub fn is_ssh_proxy_pass(&self) -> bool {
           self.get_proxy_pass(variables).trim().starts_with("ssh")
       }
    */

    pub fn get_endpoint_type(
        &self,
        host: &str,
        variables: &Option<HashMap<String, String>>,
    ) -> EndpointType {
        let proxy_pass_to = self.get_proxy_pass(variables);

        let proxy_pass_to = proxy_pass_to.as_str().trim();

        let is_ssh_proxy_pass = proxy_pass_to.starts_with("ssh");

        match self.endpoint_type.as_str() {
            "http1" => EndpointType::Http1,
            "http2" => EndpointType::Http2,
            "tcp" => {
                if is_ssh_proxy_pass {
                    EndpointType::TcpOverSsh(SshConfiguration::parse(proxy_pass_to))
                } else {
                    let remote_addr = std::net::SocketAddr::from_str(self.proxy_pass_to.as_str());

                    if remote_addr.is_err() {
                        panic!(
                            "Can not parse remote address: '{}' for tcp listen host {}",
                            proxy_pass_to, host
                        );
                    }

                    EndpointType::Tcp(remote_addr.unwrap())
                }
            }
            _ => panic!("Unknown location type: '{}'", self.endpoint_type),
        }
    }
}

fn check_and_get_endpoint_type(
    host: &str,
    locations: &[Location],
    variables: &Option<HashMap<String, String>>,
) -> EndpointType {
    let mut tcp_location = None;
    let mut http_count = 0;
    let mut http2_count = 0;
    let mut tcp_over_ssh = None;

    for location in locations {
        match location.get_endpoint_type(host, variables) {
            EndpointType::Http1 => http_count += 1,
            EndpointType::Http2 => http2_count += 1,
            EndpointType::Tcp(proxy_pass) => {
                if tcp_location.is_some() {
                    panic!(
                        "Host '{}' has more than one tcp location. It must be only single location",
                        host
                    );
                }

                tcp_location = Some(proxy_pass);
            }
            EndpointType::TcpOverSsh(ssh_configuration) => {
                if tcp_over_ssh.is_some() {
                    panic!(
                        "Host '{}' has more than one tcp over SSH location. It must be only single location",
                        host
                    );
                }

                tcp_over_ssh = Some(ssh_configuration);
            }
        }
    }
    if tcp_location.is_some() && http_count > 0 && http2_count > 0 && tcp_over_ssh.is_some() {
        panic!(
            "Host '{}' has {} http, {} http2, '{:?}' tcp and {:?} tcp over ssh configurations",
            host, http_count, http2_count, tcp_location, tcp_over_ssh
        );
    }

    if let Some(tcp_location) = tcp_location {
        return EndpointType::Tcp(tcp_location);
    }

    if let Some(tcp_over_ssh) = tcp_over_ssh {
        return EndpointType::TcpOverSsh(tcp_over_ssh);
    }

    if http_count > 0 {
        return EndpointType::Http1;
    }

    if http2_count > 0 {
        return EndpointType::Http2;
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

        let model = SettingsModel {
            hosts,
            connection_settings: None,
            variables: None,
        };

        let json = serde_yaml::to_string(&model).unwrap();

        println!("{}", json);
    }
}

fn format_mem(size: usize) -> String {
    if size < 1024 {
        return format!("{}B", size);
    }

    let size = size as f64 / 1024.0;

    if size < 1024.0 {
        return format!("{:.2}KB", size);
    }

    let size = size as f64 / 1024.0;

    return format!("{:.2}Mb", size);
}
