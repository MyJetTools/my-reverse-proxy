use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
    sync::Arc,
};

use crate::{
    app::SslCertificate,
    http_server::{ClientCertificateCa, HostPort},
};

use super::{
    ClientCertificateCaSettings, ConnectionsSettings, ConnectionsSettingsModel, EndpointType,
    FileName, HttpProxyPassRemoteEndpoint, ProxyPassSettings, SslCertificateId,
    SslCertificatesSettingsModel,
};
use hyper::Uri;
use rust_extensions::duration_utils::DurationExtensions;
use serde::*;

#[derive(my_settings_reader::SettingsModel, Serialize, Deserialize, Debug, Clone)]
pub struct SettingsModel {
    pub hosts: HashMap<String, ProxyPassSettings>,
    pub connection_settings: Option<ConnectionsSettings>,
    pub variables: Option<HashMap<String, String>>,
    pub ssl_certificates: Option<Vec<SslCertificatesSettingsModel>>,
    pub client_certificate_ca: Option<Vec<ClientCertificateCaSettings>>,
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

    pub async fn get_client_certificate_ca(&self, id: &str) -> Option<ClientCertificateCa> {
        let read_access = self.settings.read().await;

        if let Some(certs) = &read_access.client_certificate_ca {
            for ca in certs {
                if ca.id != id {
                    continue;
                }

                let ca_file = FileName::new(ca.ca.as_str());
                return Some(ClientCertificateCa::new(&ca_file));
            }
        }

        None
    }

    pub async fn get_ssl_certificate(&self, id: &SslCertificateId) -> Option<SslCertificate> {
        let read_access = self.settings.read().await;

        if let Some(certs) = &read_access.ssl_certificates {
            for cert in certs {
                if cert.id != id.as_str() {
                    continue;
                }

                let cert_file = FileName::new(cert.certificate.as_str());
                let certificates = super::certificates::load_certs(&cert_file);

                let pk_file = FileName::new(cert.private_key.as_str());
                let private_key = super::certificates::load_private_key(&pk_file);

                let result = SslCertificate {
                    certificates,
                    private_key: Arc::new(private_key),
                };

                return Some(result);
            }
        }

        None
    }

    pub async fn get_configurations<'s, T>(
        &self,
        host_port: &HostPort<'s, T>,
    ) -> Vec<(String, HttpProxyPassRemoteEndpoint)> {
        let mut result = Vec::new();
        let read_access = self.settings.read().await;

        for (settings_host, proxy_pass_settings) in &read_access.hosts {
            if host_port.is_my_host_port(settings_host) {
                for location in &proxy_pass_settings.locations {
                    let proxy_pass_path = if let Some(location) = &location.path {
                        location.to_string()
                    } else {
                        "/".to_string()
                    };

                    let proxy_pass_to = location.get_proxy_pass(&read_access.variables);

                    if proxy_pass_to.is_ssh() {
                        result.push((
                            proxy_pass_path,
                            if location.is_http1() {
                                HttpProxyPassRemoteEndpoint::Http1OverSsh(
                                    proxy_pass_to.to_ssh_configuration(),
                                )
                            } else {
                                HttpProxyPassRemoteEndpoint::Http2OverSsh(
                                    proxy_pass_to.to_ssh_configuration(),
                                )
                            },
                        ));
                    } else {
                        result.push((
                            proxy_pass_path,
                            if location.is_http1() {
                                HttpProxyPassRemoteEndpoint::Http(
                                    Uri::from_str(proxy_pass_to.as_str()).unwrap(),
                                )
                            } else {
                                HttpProxyPassRemoteEndpoint::Http2(
                                    Uri::from_str(proxy_pass_to.as_str()).unwrap(),
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

        for (host, proxy_pass) in &read_access.hosts {
            let host_port = host.split(':');

            match host_port.last().unwrap().parse::<u16>() {
                Ok(port) => {
                    result.insert(
                        port,
                        proxy_pass.endpoint.get_type(
                            host,
                            proxy_pass.locations.as_slice(),
                            &read_access.variables,
                        ),
                    );
                }
                Err(_) => {
                    panic!("Can not read port from host: '{}'", host);
                }
            }
        }

        result
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

/*
fn check_and_get_endpoint_type(
    host: &str,
    locations: &[Location],
    variables: &Option<HashMap<String, String>>,
) -> EndpointType {
    let mut tcp_location = None;
    let mut http_count = 0;

    let mut https_location = None;
    let mut http2_count = 0;
    let mut tcp_over_ssh = None;

    for location in locations {
        match location.get_endpoint_type(host, variables) {
            EndpointType::Http1 => http_count += 1,
            EndpointType::Https1(ssl_certificate_id) => {
                if https_location.is_some() {
                    panic!(
                        "Host '{}' has more than one tcp location. It must be only single location",
                        host
                    );
                }

                https_location = Some(ssl_certificate_id);
            }
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
            "Host '{}' has {} http, {:?} https, {} http2, '{:?}' tcp and {:?} tcp over ssh configurations",
            host, http_count, https_location, http2_count, tcp_location, tcp_over_ssh
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

    if let Some(https) = https_location {
        return EndpointType::Https1(https);
    }

    if http2_count > 0 {
        return EndpointType::Http2;
    }

    panic!("Host '{}' has no locations", host);
}
 */

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::settings::{EndpointSettings, LocationSettings, ProxyPassSettings};

    use super::SettingsModel;

    #[test]
    fn test() {
        let mut hosts = HashMap::new();

        hosts.insert(
            "localhost:9000".to_string(),
            ProxyPassSettings {
                endpoint: EndpointSettings {
                    endpoint_type: "http1".to_owned(),
                    ssl_certificate: None,
                    client_certificate_ca: None,
                },
                locations: vec![LocationSettings {
                    path: Some("/".to_owned()),
                    proxy_pass_to: "https://www.google.com".to_owned(),
                    location_type: Some("http".to_owned()),
                }],
            },
        );

        let model = SettingsModel {
            hosts,
            connection_settings: None,
            variables: None,
            ssl_certificates: None,
            client_certificate_ca: None,
        };

        let json = serde_yaml::to_string(&model).unwrap();

        println!("{}", json);
    }
}
