use std::collections::HashMap;

use super::*;
use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SettingsModel {
    pub hosts: HashMap<String, HostSettings>,
    pub include: Option<Vec<String>>,
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
    pub async fn load_async(file_name: Option<&str>) -> Result<Self, String> {
        let file_name = if let Some(file_name) = file_name {
            rust_extensions::file_utils::format_path(file_name).to_string()
        } else {
            format!("{}/{}", std::env::var("HOME").unwrap(), ".my-reverse-proxy")
        };

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

    pub fn load(file_name: Option<&str>) -> Result<Self, String> {
        let file_name = if let Some(file_name) = file_name {
            rust_extensions::file_utils::format_path(file_name).to_string()
        } else {
            format!("{}/{}", std::env::var("HOME").unwrap(), ".my-reverse-proxy")
        };

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
                connect_timeout_seconds: None,
            },
        );

        let model = SettingsModel {
            hosts,
            global_settings: None,
            include: None,
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
