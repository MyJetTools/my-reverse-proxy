use std::collections::HashMap;

use crate::configurations::EndpointHttpHostString;

use super::*;
use rust_extensions::duration_utils::DurationExtensions;
use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SettingsModel {
    pub hosts: HashMap<String, HostSettings>,
    pub variables: Option<HashMap<String, String>>,
    pub ssl_certificates: Option<Vec<SslCertificatesSettingsModel>>,
    pub client_certificate_ca: Option<Vec<ClientCertificateCaSettings>>,
    pub global_settings: Option<GlobalSettings>,
    pub g_auth: Option<HashMap<String, GoogleAuthSettings>>,
    pub ssh: Option<HashMap<String, SshConfigSettings>>,
    pub endpoint_templates: Option<HashMap<String, EndpointTemplateSettings>>,
    pub allowed_users: Option<HashMap<String, Vec<String>>>,
    pub ip_white_lists: Option<HashMap<String, Vec<String>>>,
    pub gateway_server: Option<GatewayServerSettings>,
    pub gateway_clients: Option<HashMap<String, GatewayClientSettings>>,
}

impl SettingsModel {
    pub async fn load_async() -> Result<Self, String> {
        let file_name = format!("{}/{}", std::env::var("HOME").unwrap(), ".my-reverse-proxy");
        let file_result = tokio::fs::read(file_name.as_str()).await;
        if file_result.is_err() {
            return Err(format!("Can not read settings from file: {}", file_name));
        }

        let file_content = file_result.unwrap();

        match my_settings_reader::serde_yaml::from_slice(&file_content) {
            Ok(result) => Ok(result),
            Err(err) => Err(format!(
                "Invalid yaml format of file: {}. Err: {}",
                file_name, err
            )),
        }
    }

    pub fn load() -> Result<Self, String> {
        let file_name = format!("{}/{}", std::env::var("HOME").unwrap(), ".my-reverse-proxy");
        let file_result = std::fs::read(file_name.as_str());
        if file_result.is_err() {
            return Err(format!("Can not read settings from file: {}", file_name));
        }

        let file_content = file_result.unwrap();

        match my_settings_reader::serde_yaml::from_slice(&file_content) {
            Ok(result) => Ok(result),
            Err(err) => Err(format!(
                "Invalid yaml format of file: {}. Err: {}",
                file_name, err
            )),
        }
    }

    pub fn get_http_control_port(&self) -> Option<u16> {
        if let Some(global_settings) = self.global_settings.as_ref() {
            return global_settings.http_control_port;
        }

        None
    }

    pub fn get_gateway_server(&self) -> Option<&GatewayServerSettings> {
        self.gateway_server.as_ref()
    }

    pub fn get_show_error_description_on_error_page(&self) -> bool {
        if let Some(global_settings) = self.global_settings.as_ref() {
            if let Some(show_error_description_on_error_page) =
                global_settings.show_error_description_on_error_page
            {
                return show_error_description_on_error_page;
            }
        }

        false
    }
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

    pub fn get_endpoint_host_string(
        &self,
        host_id: &str,
    ) -> Result<EndpointHttpHostString, String> {
        let host_id = crate::scripts::apply_variables(self, host_id)?;
        EndpointHttpHostString::new(host_id.to_string())
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

    use super::*;

    use super::SettingsModel;

    #[test]
    fn test() {
        let mut hosts = HashMap::new();

        hosts.insert(
            "localhost:9000".to_string(),
            HostSettings {
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
                    proxy_pass_to: "https://www.google.com".to_owned().into(),
                    location_type: Some("http".to_owned()),
                    modify_http_headers: None,
                    default_file: None,
                    status_code: None,
                    body: None,
                    content_type: None,
                    whitelisted_ip: None,
                    domain_name: None,
                    compress: None,
                    request_timeout: None,
                    connect_timeout: None,
                    trace_payload: None,
                }],
            },
        );

        let mut ssh_configs = HashMap::new();

        ssh_configs.insert(
            "root@10.0.0.1".to_string(),
            SshConfigSettings {
                password: "my_password".to_string().into(),
                private_key_file: None,
                passphrase: None,
            },
        );

        ssh_configs.insert(
            "root@10.0.0.2".to_string(),
            SshConfigSettings {
                password: None,
                private_key_file: Some("~/certs/private_key.ssh".to_string()),
                passphrase: Some("my_pass_phrase".to_string()),
            },
        );

        let mut gateway_clients = HashMap::new();

        gateway_clients.insert(
            "client_id".to_string(),
            GatewayClientSettings {
                remote_host: "127.0.0.1:3000".to_string(),
                encryption_key: String::new(),
                debug: None,
                compress: None,
                allow_incoming_forward_connections: None,
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
            ip_white_lists: None,
            gateway_server: None,
            gateway_clients: Some(gateway_clients),
        };

        let json = serde_yaml::to_string(&model).unwrap();

        println!("{}", json);
    }
}
