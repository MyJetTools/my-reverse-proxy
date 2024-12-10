use crate::settings::*;

pub fn get_endpoint_modify_headers(
    settings_model: &SettingsModel,
    host_settings: &HostSettings,
) -> HttpEndpointModifyHeadersSettings {
    let result = HttpEndpointModifyHeadersSettings {
        global_modify_headers_settings: get_global_modify_headers(settings_model),
        endpoint_modify_headers_settings: host_settings.endpoint.modify_http_headers.clone(),
    };

    result
}

fn get_global_modify_headers(settings_model: &SettingsModel) -> Option<ModifyHttpHeadersSettings> {
    let global_settings = settings_model.global_settings.as_ref()?;

    let all_endpoints_global_settings = global_settings.all_http_endpoints.as_ref()?;

    let modify_headers = all_endpoints_global_settings.modify_http_headers.clone()?;

    Some(modify_headers)
}
