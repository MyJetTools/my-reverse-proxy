use std::collections::{BTreeMap, HashMap};

use crate::{app::AppContext, http_proxy_pass::*, types::WhiteListedIpList};

use super::{
    ClientCertificateCaSettings, ConnectionsSettingsModel, EndpointTemplateSettings, EndpointType,
    FileSource, GlobalSettings, GoogleAuthSettings, HostStr, HostString,
    HttpEndpointModifyHeadersSettings, ProxyPassSettings, SshConfigSettings, SslCertificateId,
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

    pub g_auth: Option<HashMap<String, GoogleAuthSettings>>,

    pub ssh: Option<HashMap<String, SshConfigSettings>>,

    pub endpoint_templates: Option<HashMap<String, EndpointTemplateSettings>>,

    pub allowed_users: Option<HashMap<String, Vec<String>>>,
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
        endpoint_info: &HttpServerConnectionInfo,
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
            if !endpoint_info.is_my_endpoint(host) {
                continue;
            }

            if let Some(modify_headers) = proxy_pass
                .endpoint
                .get_modify_http_headers(&read_access.endpoint_templates)
            {
                result.endpoint_modify_headers_settings = Some(modify_headers.clone());
            }
        }

        result
    }

    pub async fn get_client_certificate_ca(&self, id: &str) -> Result<Option<FileSource>, String> {
        let read_access = self.settings.read().await;

        if let Some(certs) = &read_access.client_certificate_ca {
            for ca in certs {
                if ca.id != id {
                    continue;
                }

                return Ok(Some(ca.get_ca(&read_access.variables, &read_access.ssh)?));
            }
        }

        Ok(None)
    }

    pub async fn get_ssl_certificate(
        &self,
        id: &SslCertificateId,
    ) -> Result<Option<(FileSource, FileSource)>, String> {
        let read_access = self.settings.read().await;

        if let Some(certs) = &read_access.ssl_certificates {
            for cert in certs {
                if cert.id != id.as_str() {
                    continue;
                }

                return Ok(Some((
                    cert.get_certificate(&read_access.variables, &read_access.ssh)?,
                    cert.get_private_key(&read_access.variables, &read_access.ssh)?,
                )));
            }
        }

        Ok(None)
    }

    pub async fn get_https_connection_configuration(
        &self,
        connection_server_name: &str,
        endpoint_listen_port: u16,
    ) -> Result<HttpServerConnectionInfo, String> {
        let read_access = self.settings.read().await;

        for (settings_host, proxy_pass_settings) in &read_access.hosts {
            let host_str = HostStr::new(settings_host);

            if !host_str.is_my_https_server_name(connection_server_name, endpoint_listen_port) {
                continue;
            }

            let endpoint_template_settings = proxy_pass_settings
                .endpoint
                .get_endpoint_template(&read_access.endpoint_templates)?;

            let result = HttpServerConnectionInfo::new(
                HostString::new(settings_host.to_string()),
                proxy_pass_settings.endpoint.get_http_type(),
                proxy_pass_settings.endpoint.get_debug(),
                proxy_pass_settings
                    .endpoint
                    .get_google_auth_settings(endpoint_template_settings, &read_access.g_auth)?,
                proxy_pass_settings
                    .endpoint
                    .get_client_certificate_id(endpoint_template_settings),
            );

            return Ok(result);
        }

        Err(format!(
            "Can not find https server configuration for '{}:{}'",
            connection_server_name, endpoint_listen_port
        ))
    }

    pub async fn get_locations(
        &self,
        app: &AppContext,
        req: &HttpRequestBuilder,
        is_https: bool,
    ) -> Result<(Vec<ProxyPassLocation>, Option<AllowedUserList>), ProxyPassError> {
        let read_access = self.settings.read().await;

        for (settings_host, proxy_pass_settings) in &read_access.hosts {
            if !req.is_mine(settings_host, is_https) {
                continue;
            }

            let location_id = app.get_id();

            let mut allowed_users = None;

            if let Some(allowed_user_id) = &proxy_pass_settings.endpoint.allowed_users {
                if let Some(users) = &read_access.allowed_users {
                    if let Some(users) = users.get(allowed_user_id) {
                        allowed_users = Some(AllowedUserList::new(users.clone()));
                    }
                }
            }

            let mut result = Vec::new();
            for location_settings in &proxy_pass_settings.locations {
                let location_path = if let Some(location) = &location_settings.path {
                    location.to_string()
                } else {
                    "/".to_string()
                };

                let proxy_pass_content_source = location_settings.get_http_content_source(
                    app,
                    settings_host,
                    location_id,
                    &read_access.variables,
                    &read_access.ssh,
                    proxy_pass_settings.endpoint.get_debug(),
                );

                if let Err(err) = proxy_pass_content_source {
                    return Err(ProxyPassError::CanNotReadSettingsConfiguration(err));
                }

                let proxy_pass_content_source = proxy_pass_content_source.unwrap();

                if proxy_pass_content_source.is_none() {
                    continue;
                }

                let proxy_pass_content_source = proxy_pass_content_source.unwrap();

                let mut whitelisted_ip = WhiteListedIpList::new();
                whitelisted_ip.apply(
                    proxy_pass_settings
                        .endpoint
                        .get_white_listed_ip(&read_access.endpoint_templates)
                        .as_deref(),
                );
                whitelisted_ip.apply(location_settings.whitelisted_ip.as_deref());

                result.push(ProxyPassLocation::new(
                    location_id,
                    location_path,
                    location_settings.modify_http_headers.clone(),
                    proxy_pass_content_source,
                    whitelisted_ip,
                ));
            }

            return Ok((result, allowed_users));
        }

        return Ok((vec![], None));
    }

    pub async fn get_listen_ports(&self) -> Result<BTreeMap<u16, EndpointType>, String> {
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
                            &read_access.endpoint_templates,
                            &read_access.variables,
                            &read_access.ssh,
                            &read_access.g_auth,
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
                    google_auth: None,
                    whitelisted_ip: None,
                    template_id: None,
                    allowed_users: None,
                },
                locations: vec![LocationSettings {
                    path: Some("/".to_owned()),
                    proxy_pass_to: "https://www.google.com".to_owned(),
                    location_type: Some("http".to_owned()),
                    modify_http_headers: None,
                    default_file: None,
                    status_code: None,
                    body: None,
                    content_type: None,
                    whitelisted_ip: None,
                }],
            },
        );

        let mut ssh_configs = HashMap::new();

        ssh_configs.insert(
            "root@10.0.0.1".to_string(),
            crate::settings::SshConfigSettings {
                password: "my_password".to_string().into(),
                private_key_file: None,
                passphrase: None,
            },
        );

        ssh_configs.insert(
            "root@10.0.0.2".to_string(),
            crate::settings::SshConfigSettings {
                password: None,
                private_key_file: Some("~/certs/private_key.ssh".to_string()),
                passphrase: Some("my_pass_phrase".to_string()),
            },
        );

        let model = SettingsModel {
            hosts,
            global_settings: None,
            variables: None,
            ssl_certificates: None,
            client_certificate_ca: None,
            ssh: Some(ssh_configs),
            g_auth: None,
            endpoint_templates: None,
            allowed_users: None,
        };

        let json = serde_yaml::to_string(&model).unwrap();

        println!("{}", json);
    }
}
