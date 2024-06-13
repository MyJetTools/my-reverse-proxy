use std::collections::{BTreeMap, HashMap};

use crate::{
    app::AppContext,
    app_configuration::{EndpointType, HttpListenPortConfiguration, ListenPortConfiguration},
};

use super::{
    AllowedUsersSettingsModel, ClientCertificateCaSettings, ConnectionsSettingsModel,
    EndpointHttpHostString, EndpointTemplateSettings, FileSource, GlobalSettings,
    GoogleAuthSettings, ProxyPassSettings, SshConfigSettings, SslCertificateId,
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

    allowed_users: Option<HashMap<String, Vec<String>>>,
}

impl SettingsModel {
    pub fn get_connections_settings(&self) -> ConnectionsSettingsModel {
        let result = if let Some(global_settings) = self.global_settings.as_ref() {
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

    pub fn get_session_key(&self) -> Option<String> {
        if let Some(global_settings) = self.global_settings.as_ref() {
            if let Some(connection_settings) = global_settings.connection_settings.as_ref() {
                return connection_settings.session_key.clone();
            }
        }

        None
    }

    async fn get_allowed_users_settings(&self) -> Result<AllowedUsersSettingsModel, String> {
        let mut allowed_users = self.allowed_users.clone();

        let mut files_to_load = None;

        if let Some(allowed_users) = allowed_users.as_mut() {
            files_to_load = allowed_users.remove("from_file");
        }

        let mut result = AllowedUsersSettingsModel::new(self.allowed_users.clone());

        if let Some(files_to_load) = files_to_load {
            for file_to_load in files_to_load {
                let file_to_load =
                    crate::populate_variable::populate_variable(&file_to_load, &self.variables);

                let file_src = FileSource::from_src(file_to_load.into(), &self.ssh)?;
                result.populate_from_file(file_src).await?;
            }
        }

        Ok(result)
    }

    pub async fn get_listen_ports(
        &self,
        app: &AppContext,
    ) -> Result<BTreeMap<u16, ListenPortConfiguration>, String> {
        let mut result: BTreeMap<u16, ListenPortConfiguration> = BTreeMap::new();

        for (host, proxy_pass) in &self.hosts {
            let host = crate::populate_variable::populate_variable(host, &self.variables);

            let end_point = EndpointHttpHostString::new(host.as_str().to_string())?;

            let port = end_point.get_port();

            let endpoint_template_settings = proxy_pass
                .endpoint
                .get_endpoint_template(&self.endpoint_templates)?;

            let allowed_users_settings = self.get_allowed_users_settings().await?;

            let allowed_users = proxy_pass
                .get_allowed_users(&allowed_users_settings, endpoint_template_settings)?;

            let endpoint_type = proxy_pass.endpoint.get_type(
                end_point,
                &proxy_pass.endpoint,
                proxy_pass.locations.as_slice(),
                endpoint_template_settings,
                &self.variables,
                &self.ssh,
                &self.g_auth,
                allowed_users,
                &self.global_settings,
                app,
            )?;

            match endpoint_type {
                EndpointType::Http(http_endpoint_info) => match result.get_mut(&port) {
                    Some(other_port_configuration) => {
                        other_port_configuration
                            .add_http_endpoint_info(host.as_str(), http_endpoint_info)?;
                    }
                    None => {
                        result.insert(
                            port,
                            ListenPortConfiguration::Http(HttpListenPortConfiguration::new(
                                http_endpoint_info.into(),
                            )),
                        );
                    }
                },
                EndpointType::Tcp(endpoint_info) => match result.get(&port) {
                    Some(other_end_point_type) => {
                        return Err(format!(
                            "Port {} is used twice by host configurations {} and {}",
                            port,
                            host.as_str(),
                            other_end_point_type.get_endpoint_host_as_str()
                        ));
                    }
                    None => {
                        result.insert(port, ListenPortConfiguration::Tcp(endpoint_info));
                    }
                },
                EndpointType::TcpOverSsh(endpoint_info) => match result.get(&port) {
                    Some(other_end_point_type) => {
                        return Err(format!(
                            "Port {} is used twice by host configurations {} and {}",
                            port,
                            host.as_str(),
                            other_end_point_type.get_endpoint_host_as_str()
                        ));
                    }
                    None => {
                        result.insert(port, ListenPortConfiguration::TcpOverSsh(endpoint_info));
                    }
                },
            }
        }

        Ok(result)
    }

    pub fn get_client_certificate_ca(&self, id: &str) -> Result<Option<FileSource>, String> {
        if let Some(certs) = &self.client_certificate_ca {
            for ca in certs {
                if ca.id != id {
                    continue;
                }

                return Ok(Some(ca.get_ca(&self.variables, &self.ssh)?));
            }
        }

        Ok(None)
    }

    pub fn get_ssl_certificate(
        &self,
        id: &SslCertificateId,
    ) -> Result<Option<(FileSource, FileSource)>, String> {
        if let Some(certs) = &self.ssl_certificates {
            for cert in certs {
                if cert.id != id.as_str() {
                    continue;
                }

                return Ok(Some((
                    cert.get_certificate(&self.variables, &self.ssh)?,
                    cert.get_private_key(&self.variables, &self.ssh)?,
                )));
            }
        }

        Ok(None)
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
