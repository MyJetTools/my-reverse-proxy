use crate::{configurations::EndpointHttpHostString, settings::*};

pub fn get_from_host_or_templates<'s, TResult>(
    settings_model: &'s SettingsModel,
    host_endpoint: &EndpointHttpHostString,
    host_settings: &'s HostSettings,
    get_from_host_settings: fn(&'s HostSettings) -> Option<&'s TResult>,
    get_from_templates: fn(&'s EndpointTemplateSettings) -> Option<&'s TResult>,
) -> Result<Option<&'s TResult>, String> {
    if let Some(ssl_id) = get_from_host_settings(host_settings) {
        return Ok(Some(ssl_id));
    }

    match super::get_endpoint_template(settings_model, host_endpoint, host_settings)? {
        Some(endpoint_template_settings) => {
            let ssl_cert = get_from_templates(endpoint_template_settings);
            return Ok(ssl_cert);
        }
        None => {
            return Ok(None);
        }
    }
}
