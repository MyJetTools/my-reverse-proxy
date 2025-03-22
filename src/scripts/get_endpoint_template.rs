use crate::{settings::*, settings_compiled::SettingsCompiled};

pub fn get_endpoint_template<'s>(
    settings_model: &'s SettingsCompiled,
    host_settings: &'s HostSettings,
) -> Result<Option<&'s EndpointTemplateSettings>, String> {
    let template_id = match host_settings.endpoint.template_id.as_ref() {
        Some(template_id) => template_id,
        None => {
            return Ok(None);
        }
    };

    match settings_model.endpoint_templates.get(template_id) {
        Some(endpoint_template_settings) => Ok(Some(endpoint_template_settings)),
        None => {
            return Err(format!("Template with id '{}' not found", template_id,));
        }
    }
}
