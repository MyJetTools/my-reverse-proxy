use crate::settings::*;

use super::{SettingsCompiled, VariablesCompiled};

impl SettingsCompiled {
    pub async fn load_settings() -> Result<Self, String> {
        let mut result = Self::default();

        let settings_model = crate::settings::SettingsModel::load_async(None).await?;
        let include_settings = load_includes(&settings_model).await?;
        result.populate(settings_model, include_settings)?;

        Ok(result)
    }

    pub fn load_settings_block() -> Result<Self, String> {
        let mut result = Self::default();

        let settings_model = crate::settings::SettingsModel::load(None)?;
        let include_settings = load_includes_block(&settings_model)?;
        result.populate(settings_model, include_settings)?;

        Ok(result)
    }

    fn populate(
        &mut self,
        mut settings_model: SettingsModel,
        mut include_settings: Vec<SettingsModel>,
    ) -> Result<(), String> {
        let mut variables = VariablesCompiled::default();
        variables.merge(&mut settings_model);

        for sub_settings in include_settings.iter_mut() {
            variables.merge(sub_settings);
        }

        self.populate_hosts(&mut settings_model, &variables)?;
        self.populate_ssh_configs(&mut settings_model, &variables)?;
        self.populate_ssl_certificates(&mut settings_model, &variables)?;
        self.populate_ca_certificates(&mut settings_model, &variables)?;
        self.populate_global_settings(&mut settings_model, &variables)?;
        self.populate_g_auth(&mut settings_model, &variables)?;
        self.populate_endpoint_templates(&mut settings_model, &variables)?;
        self.populate_allowed_users(&mut settings_model, &variables)?;
        self.populate_ip_white_lists(&mut settings_model, &variables)?;
        self.populate_gateway_server(&mut settings_model, &variables)?;
        self.populate_gateway_client(&mut settings_model, &variables)?;

        for sub_settings in include_settings.iter_mut() {
            self.populate_hosts(sub_settings, &variables)?;
            self.populate_ssh_configs(sub_settings, &variables)?;
            self.populate_ssl_certificates(sub_settings, &variables)?;
            self.populate_ca_certificates(sub_settings, &variables)?;
            self.populate_global_settings(sub_settings, &variables)?;
            self.populate_g_auth(sub_settings, &variables)?;
            self.populate_endpoint_templates(sub_settings, &variables)?;
            self.populate_allowed_users(sub_settings, &variables)?;
            self.populate_ip_white_lists(sub_settings, &variables)?;
            self.populate_gateway_server(sub_settings, &variables)?;
            self.populate_gateway_client(sub_settings, &variables)?;
        }

        Ok(())
    }

    fn populate_ssh_configs(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        if let Some(ssh) = settings_model.ssh.take() {
            for (ssh_id, ssh_settings) in ssh {
                let ssh_settings = SshConfigSettings {
                    password: variables.apply_variables_opt(ssh_settings.password)?,
                    private_key_file: variables
                        .apply_variables_opt(ssh_settings.private_key_file)?,
                    passphrase: variables.apply_variables_opt(ssh_settings.passphrase)?,
                };

                self.ssh.insert(ssh_id, ssh_settings);
            }
        }

        Ok(())
    }

    fn populate_hosts(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        let hosts = std::mem::take(&mut settings_model.hosts);
        for (host_id, host_settings) in hosts {
            let host_id = variables.apply_variables(host_id)?;
            let locations = compile_locations(host_settings.locations, variables)?;

            let host_settings = HostSettings {
                endpoint: EndpointSettings {
                    endpoint_type: variables
                        .apply_variables(host_settings.endpoint.endpoint_type)?,
                    ssl_certificate: variables
                        .apply_variables_opt(host_settings.endpoint.ssl_certificate)?,
                    client_certificate_ca: variables
                        .apply_variables_opt(host_settings.endpoint.client_certificate_ca)?,
                    google_auth: variables
                        .apply_variables_opt(host_settings.endpoint.google_auth)?,
                    modify_http_headers: super::populate_modify_http_headers_settings(
                        host_settings.endpoint.modify_http_headers,
                        variables,
                    )?,
                    debug: host_settings.endpoint.debug,
                    whitelisted_ip: variables
                        .apply_variables_opt(host_settings.endpoint.whitelisted_ip)?,
                    template_id: variables
                        .apply_variables_opt(host_settings.endpoint.template_id)?,
                    allowed_users: variables
                        .apply_variables_opt(host_settings.endpoint.allowed_users)?,
                },
                locations,
            };

            self.hosts.insert(host_id, host_settings);
        }

        Ok(())
    }

    fn populate_ssl_certificates(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        if let Some(ssl) = settings_model.ssl_certificates.take() {
            for itm in ssl {
                self.ssl_certificates.push(SslCertificatesSettingsModel {
                    id: variables.apply_variables(itm.id)?,
                    certificate: variables.apply_variables(itm.certificate)?,
                    private_key: variables.apply_variables(itm.private_key)?,
                });
            }
        }

        Ok(())
    }

    fn populate_ca_certificates(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        if let Some(ca_certs) = settings_model.client_certificate_ca.take() {
            for itm in ca_certs {
                self.client_certificate_ca
                    .push(ClientCertificateCaSettings {
                        id: variables.apply_variables(itm.id)?,
                        ca: variables.apply_variables(itm.ca)?,
                        revocation_list: variables.apply_variables_opt(itm.revocation_list)?,
                    });
            }
        }

        Ok(())
    }

    fn populate_global_settings(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        if let Some(itm) = settings_model.global_settings.take() {
            self.global_settings = Some(GlobalSettings {
                connection_settings: itm.connection_settings,
                all_http_endpoints: if let Some(itm) = itm.all_http_endpoints {
                    Some(AllHttpEndpointsGlobalSettings {
                        modify_http_headers: super::populate_modify_http_headers_settings(
                            itm.modify_http_headers,
                            variables,
                        )?,
                    })
                } else {
                    None
                },
                show_error_description_on_error_page: itm.show_error_description_on_error_page,
                http_control_port: itm.http_control_port,
            })
        }

        Ok(())
    }

    fn populate_g_auth(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        if let Some(g_auth) = settings_model.g_auth.take() {
            for (key, itm) in g_auth {
                self.g_auth.insert(
                    variables.apply_variables(key)?,
                    GoogleAuthSettings {
                        client_id: variables.apply_variables(itm.client_id)?,
                        client_secret: variables.apply_variables(itm.client_secret)?,
                        whitelisted_domains: variables.apply_variables(itm.whitelisted_domains)?,
                    },
                );
            }
        }

        Ok(())
    }

    fn populate_endpoint_templates(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        if let Some(endpoint_templates) = settings_model.endpoint_templates.take() {
            for (key, itm) in endpoint_templates {
                self.endpoint_templates.insert(
                    variables.apply_variables(key)?,
                    EndpointTemplateSettings {
                        ssl_certificate: variables.apply_variables_opt(itm.ssl_certificate)?,
                        client_certificate_ca: variables
                            .apply_variables_opt(itm.client_certificate_ca)?,
                        google_auth: variables.apply_variables_opt(itm.google_auth)?,
                        modify_http_headers: super::populate_modify_http_headers_settings(
                            itm.modify_http_headers,
                            variables,
                        )?,
                        whitelisted_ip: variables.apply_variables_opt(itm.whitelisted_ip)?,
                    },
                );
            }
        }

        Ok(())
    }

    fn populate_allowed_users(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        if let Some(allowed_users) = settings_model.allowed_users.take() {
            for (key, users) in allowed_users {
                self.allowed_users.insert(
                    variables.apply_variables(key)?,
                    super::populate_vec_of_string(users, variables)?,
                );
            }
        }

        Ok(())
    }

    fn populate_ip_white_lists(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        if let Some(ip_white_lists) = settings_model.ip_white_lists.take() {
            for (key, ip_list) in ip_white_lists {
                self.ip_white_lists.insert(
                    variables.apply_variables(key)?,
                    super::populate_vec_of_string(ip_list, variables)?,
                );
            }
        }

        Ok(())
    }

    fn populate_gateway_server(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        if let Some(itm) = settings_model.gateway_server.take() {
            self.gateway_server = Some(GatewayServerSettings {
                port: itm.port,
                allowed_ip: itm.allowed_ip.clone(),
                encryption_key: variables.apply_variables(itm.encryption_key)?,
                debug: itm.debug,
            });
        }

        Ok(())
    }

    fn populate_gateway_client(
        &mut self,
        settings_model: &mut SettingsModel,
        variables: &VariablesCompiled,
    ) -> Result<(), String> {
        if let Some(settings) = settings_model.gateway_clients.take() {
            for (key, itm) in settings {
                self.gateway_clients.insert(
                    variables.apply_variables(key)?,
                    GatewayClientSettings {
                        remote_host: variables.apply_variables(itm.remote_host)?,
                        encryption_key: variables.apply_variables(itm.encryption_key)?,
                        compress: itm.compress,
                        debug: itm.debug,
                        allow_incoming_forward_connections: itm.allow_incoming_forward_connections,
                        connect_timeout_seconds: itm.connect_timeout_seconds,
                    },
                );
            }
        }

        Ok(())
    }
}

async fn load_includes(
    settings_model: &SettingsModel,
) -> Result<Vec<crate::settings::SettingsModel>, String> {
    let mut result = vec![];

    if let Some(include) = settings_model.include.as_ref() {
        for include_file in include {
            let settings =
                crate::settings::SettingsModel::load_async(Some(include_file.as_str())).await?;
            result.push(settings);
        }
    }

    Ok(result)
}

fn load_includes_block(
    settings_model: &SettingsModel,
) -> Result<Vec<crate::settings::SettingsModel>, String> {
    let mut result = vec![];

    if let Some(include) = settings_model.include.as_ref() {
        for include_file in include {
            let settings = crate::settings::SettingsModel::load(Some(include_file.as_str()))?;
            result.push(settings);
        }
    }

    Ok(result)
}

fn compile_locations(
    locations: Vec<LocationSettings>,
    variables: &VariablesCompiled,
) -> Result<Vec<LocationSettings>, String> {
    let mut result = Vec::with_capacity(locations.len());

    for location in locations {
        result.push(LocationSettings {
            path: variables.apply_variables_opt(location.path)?,
            proxy_pass_to: variables.apply_variables_opt(location.proxy_pass_to)?,
            location_type: variables.apply_variables_opt(location.location_type)?,
            domain_name: variables.apply_variables_opt(location.domain_name)?,
            modify_http_headers: super::populate_modify_http_headers_settings(
                location.modify_http_headers,
                variables,
            )?,
            default_file: variables.apply_variables_opt(location.default_file)?,
            status_code: location.status_code,
            content_type: variables.apply_variables_opt(location.content_type)?,
            body: variables.apply_variables_opt(location.body)?,
            whitelisted_ip: variables.apply_variables_opt(location.whitelisted_ip)?,
            compress: location.compress,
            connect_timeout: location.connect_timeout,
            request_timeout: location.request_timeout,
            trace_payload: location.trace_payload,
        });
    }

    Ok(result)
}
