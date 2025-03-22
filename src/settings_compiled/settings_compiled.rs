use std::collections::HashMap;

use rust_extensions::duration_utils::DurationExtensions;

use crate::{configurations::EndpointHttpHostString, settings::*};

#[derive(Debug, Default)]

pub struct SettingsCompiled {
    pub ssh: HashMap<String, SshConfigSettings>,
    pub hosts: HashMap<String, HostSettings>,
    pub ssl_certificates: Vec<SslCertificatesSettingsModel>,
    pub client_certificate_ca: Vec<ClientCertificateCaSettings>,
    pub global_settings: Option<GlobalSettings>,
    pub g_auth: HashMap<String, GoogleAuthSettings>,
    pub endpoint_templates: HashMap<String, EndpointTemplateSettings>,
    pub allowed_users: HashMap<String, Vec<String>>,
    pub ip_white_lists: HashMap<String, Vec<String>>,
    pub gateway_server: Option<GatewayServerSettings>,
    pub gateway_clients: HashMap<String, GatewayClientSettings>,
}

impl SettingsCompiled {
    pub fn get_endpoint_host_string(
        &self,
        host_id: &str,
    ) -> Result<EndpointHttpHostString, String> {
        EndpointHttpHostString::new(host_id.to_string())
    }

    pub fn get_http_control_port(&self) -> Option<u16> {
        if let Some(global_settings) = self.global_settings.as_ref() {
            return global_settings.http_control_port;
        }

        None
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
            crate::format_mem(result.buffer_size)
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
}
