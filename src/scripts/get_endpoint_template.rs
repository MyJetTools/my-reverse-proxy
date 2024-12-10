use crate::{configurations::EndpointHttpHostString, settings::*};

pub fn get_endpoint_template<'s>(
    settings_model: &'s SettingsModel,
    host_endpoint: &EndpointHttpHostString,
    host_settings: &'s HostSettings,
) -> Result<Option<&'s EndpointTemplateSettings>, String> {
    let template_id = match host_settings.endpoint.template_id.as_ref() {
        Some(template_id) => template_id,
        None => {
            return Ok(None);
        }
    };

    let endpoint_templates = match settings_model.endpoint_templates.as_ref() {
        Some(endpoint_templates) => endpoint_templates,
        None => {
            return Err(format!(
                "Template with id {} not found for endpoint {}",
                template_id,
                host_endpoint.as_str()
            ));
        }
    };

    match endpoint_templates.get(template_id) {
        Some(endpoint_template_settings) => Ok(Some(endpoint_template_settings)),
        None => {
            return Err(format!(
                "Template with id {} not found for endpoint {}",
                template_id,
                host_endpoint.as_str()
            ));
        }
    }
}
