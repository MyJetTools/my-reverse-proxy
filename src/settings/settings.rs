use std::collections::{BTreeMap, HashMap};

use crate::{app::AppContext, http_proxy_pass::*};

use super::{
    ClientCertificateCaSettings, ConnectionsSettingsModel, EndpointType, FileSource,
    GlobalSettings, HttpEndpointModifyHeadersSettings, ProxyPassSettings, SslCertificateId,
    SslCertificatesSettingsModel,
};
use rust_extensions::duration_utils::DurationExtensions;
use serde::*;

#[derive(my_settings_reader::SettingsModel, Serialize, Deserialize, Debug, Clone)]
pub struct SettingsModel {
    pub hosts: HashMap<String, ProxyPassSettings>,

    pub variables: Option<HashMap<String, String>>,
    pub ssl_certificates: Option<Vec<SslCertificatesSettingsModel>>,
    pub client_certificate_ca: Option<Vec<ClientCertificateCaSettings>>,
    pub global_settings: Option<GlobalSettings>,
}

impl SettingsReader {
    pub async fn get_connections_settings(&self) -> ConnectionsSettingsModel {
        let read_access = self.settings.read().await;

        let result = if let Some(global_settings) = read_access.global_settings.as_ref() {
            match global_settings.connection_settings.as_ref() {
                Some(connection_settings) => ConnectionsSettingsModel::new(connection_settings),
                None => ConnectionsSettingsModel::default(),
            }
        } else {
            ConnectionsSettingsModel::default()
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

    pub async fn get_http_endpoint_modify_headers_settings(
        &self,
        host_str: &str,
    ) -> HttpEndpointModifyHeadersSettings {
        let mut result = HttpEndpointModifyHeadersSettings::default();
        let read_access = self.settings.read().await;

        if let Some(global_settings) = read_access.global_settings.as_ref() {
            if let Some(all_http_endpoints) = global_settings.all_http_endpoints.as_ref() {
                if let Some(modify_headers) = all_http_endpoints.modify_http_headers.as_ref() {
                    result.global_modify_headers_settings = Some(modify_headers.clone());
                }
            }
        }

        for (host, proxy_pass) in &read_access.hosts {
            if host != host_str {
                continue;
            }

            if let Some(modify_headers) = proxy_pass.endpoint.modify_http_headers.as_ref() {
                result.endpoint_modify_headers_settings = Some(modify_headers.clone());
            }
        }

        result
    }

    pub async fn get_client_certificate_ca(&self, id: &str) -> Option<FileSource> {
        let read_access = self.settings.read().await;

        if let Some(certs) = &read_access.client_certificate_ca {
            for ca in certs {
                if ca.id != id {
                    continue;
                }

                return Some(ca.get_ca(&read_access.variables));
            }
        }

        None
    }

    pub async fn get_ssl_certificate(
        &self,
        id: &SslCertificateId,
    ) -> Option<(FileSource, FileSource)> {
        let read_access = self.settings.read().await;

        if let Some(certs) = &read_access.ssl_certificates {
            for cert in certs {
                if cert.id != id.as_str() {
                    continue;
                }

                return Some((
                    cert.get_certificate(&read_access.variables),
                    cert.get_private_key(&read_access.variables),
                ));
            }
        }

        None
    }

    pub async fn get_locations<'s>(
        &self,
        app: &AppContext,
        host_port: &HostPort<'s>,
    ) -> Result<Vec<ProxyPassLocation>, ProxyPassError> {
        let read_access = self.settings.read().await;

        for (settings_host, proxy_pass_settings) in &read_access.hosts {
            if !host_port.is_my_host_port(settings_host) {
                continue;
            }
            let location_id = app.get_id();

            let mut result = Vec::new();
            for location_settings in &proxy_pass_settings.locations {
                let location_path = if let Some(location) = &location_settings.path {
                    location.to_string()
                } else {
                    "/".to_string()
                };

                let proxy_pass_content_source = location_settings.get_http_content_source(
                    app,
                    location_id,
                    &read_access.variables,
                );

                if let Err(err) = proxy_pass_content_source {
                    return Err(ProxyPassError::CanNotReadSettingsConfiguration(err));
                }

                let proxy_pass_content_source = proxy_pass_content_source.unwrap();

                if proxy_pass_content_source.is_none() {
                    continue;
                }

                let proxy_pass_content_source = proxy_pass_content_source.unwrap();

                result.push(ProxyPassLocation::new(
                    location_id,
                    location_path,
                    location_settings.modify_http_headers.clone(),
                    proxy_pass_content_source,
                ));

                /*
                let content_source = match proxy_pass_to
                    .to_content_source(location_settings.is_http1(), default_file)
                {
                    super::ContentSourceSettings::File {
                        file_name,
                        default_file,
                    } => ProxyPassContentSource::File(FileContentSrc::new(
                        file_name.get_value().to_string(),
                        default_file,
                    )),
                    super::ContentSourceSettings::FileOverSsh {
                        file_path,
                        ssh_credentials,
                        default_file,
                    } => ProxyPassContentSource::FileOverSsh(SshFileContentSource::new(
                        ssh_credentials,
                        file_path,
                        default_file,
                        app.connection_settings.remote_connect_timeout,
                    )),
                };

                result.push(ProxyPassLocation::new(
                    location_id,
                    location_path,
                    location_settings.modify_http_headers.clone(),
                    content_source,
                ));
                 */
            }

            return Ok(result);
        }

        return Ok(vec![]);
    }

    pub async fn get_listen_ports(&self) -> Result<BTreeMap<u16, EndpointType>, ProxyPassError> {
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
                        )?,
                    );
                }
                Err(_) => {
                    panic!("Can not read port from host: '{}'", host);
                }
            }
        }

        Ok(result)
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
                    modify_http_headers: None,
                    debug: None,
                },
                locations: vec![LocationSettings {
                    path: Some("/".to_owned()),
                    proxy_pass_to: "https://www.google.com".to_owned(),
                    location_type: Some("http".to_owned()),
                    modify_http_headers: None,
                    default_file: None,
                }],
            },
        );

        let model = SettingsModel {
            hosts,
            global_settings: None,
            variables: None,
            ssl_certificates: None,
            client_certificate_ca: None,
        };

        let json = serde_yaml::to_string(&model).unwrap();

        println!("{}", json);
    }
}
