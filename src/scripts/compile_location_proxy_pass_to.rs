use std::sync::Arc;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{app::AppContext, configurations::ProxyPassLocationConfig, settings::*};

pub async fn compile_location_proxy_pass_to(
    app: &Arc<AppContext>,
    settings_model: &SettingsModel,
    location_settings: &LocationSettings,
) -> Result<ProxyPassLocationConfig, String> {
    let path = match location_settings.path.as_ref() {
        Some(path) => super::apply_variables(settings_model, path)?,
        None => "/".into(),
    };

    let proxy_pass_to = match location_settings.proxy_pass_to.as_ref() {
        Some(proxy_pass_to) => {
            let proxy_pass_to = super::apply_variables(settings_model, proxy_pass_to)?;
            Some(proxy_pass_to)
        }
        None => None,
    };

    let location_type = match location_settings.get_location_type()? {
        Some(location_type) => location_type,
        None => {
            LocationType::detect_from_proxy_pass_to(proxy_pass_to.as_ref().map(|itm| itm.as_str()))?
        }
    };

    let proxy_pass_to = match location_type {
        LocationType::Http => {
            if proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for http location type".to_string());
            }

            let proxy_pass_to = proxy_pass_to.unwrap();
            let proxy_pass_to = OverSshConnectionSettings::try_parse(proxy_pass_to.as_str())
                .ok_or(format!(
                    "error parsing proxy_pass_to {}",
                    proxy_pass_to.as_str()
                ))?;
            ProxyPassTo::Http1(proxy_pass_to)
        }
        LocationType::Http2 => {
            if proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for http2 location type".to_string());
            }

            let proxy_pass_to = proxy_pass_to.unwrap();
            let proxy_pass_to = OverSshConnectionSettings::try_parse(proxy_pass_to.as_str())
                .ok_or(format!(
                    "error parsing proxy_pass_to {}",
                    proxy_pass_to.as_str()
                ))?;

            ProxyPassTo::Http2(proxy_pass_to)
        }
        LocationType::Https1 => {
            if proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for http2 location type".to_string());
            }

            let proxy_pass_to = proxy_pass_to.unwrap();
            let proxy_pass_to = OverSshConnectionSettings::try_parse(proxy_pass_to.as_str())
                .ok_or(format!(
                    "error parsing proxy_pass_to {}",
                    proxy_pass_to.as_str()
                ))?;

            ProxyPassTo::Http1(proxy_pass_to)
        }
        LocationType::Https2 => {
            if proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for http2 location type".to_string());
            }

            let proxy_pass_to = proxy_pass_to.unwrap();
            let proxy_pass_to = OverSshConnectionSettings::try_parse(proxy_pass_to.as_str())
                .ok_or(format!(
                    "error parsing proxy_pass_to {}",
                    proxy_pass_to.as_str()
                ))?;
            ProxyPassTo::Http2(proxy_pass_to)
        }
        LocationType::Files => {
            if proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for files location type".to_string());
            }

            let proxy_pass_to = proxy_pass_to.unwrap();
            let files_path = OverSshConnectionSettings::try_parse(proxy_pass_to.as_str())
                .ok_or(format!("Invalid proxy_pass_to {}", proxy_pass_to.as_str()))?;

            let model = ProxyPassFilesPathModel {
                files_path,
                default_file: location_settings.default_file.clone(),
            };

            ProxyPassTo::FilesPath(model)
        }
        LocationType::StaticContent => {
            if location_settings.body.is_none() {
                return Err("body is required for static content location type".to_string());
            }

            let body = location_settings.body.clone().unwrap();

            let body = get_static_content_body(app, settings_model, body).await?;
            let model: StaticContentModel = StaticContentModel {
                status_code: location_settings.status_code.unwrap_or(200),
                content_type: location_settings.content_type.clone(),
                body,
            };

            ProxyPassTo::Static(model)
        }
    };

    let result = ProxyPassLocationConfig::new(
        app.get_next_id(),
        path.to_string(),
        location_settings.modify_http_headers.clone(),
        location_settings.whitelisted_ip.clone(),
        proxy_pass_to,
        location_settings.domain_name.clone(),
        location_settings.compress.unwrap_or(false),
    );

    Ok(result)
}

async fn get_static_content_body(
    app: &Arc<AppContext>,
    settings_model: &SettingsModel,
    body: String,
) -> Result<Vec<u8>, String> {
    match get_fist_char(body.as_str()) {
        Some(c) => {
            if c == '<' {
                return Ok(body.into_bytes());
            }

            if c == '{' {
                return Ok(body.into_bytes());
            }

            let remote_resource = super::apply_variables(settings_model, body.as_str())?;
            match OverSshConnectionSettings::try_parse(remote_resource.as_str()) {
                Some(data_source) => super::load_file(app, &data_source).await,
                None => {
                    return Ok(body.into_bytes());
                }
            }
        }
        None => {
            return Ok(body.into_bytes());
        }
    }
}

fn get_fist_char(body: &str) -> Option<char> {
    for c in body.chars() {
        if c.is_whitespace() {
            continue;
        }

        return Some(c);
    }

    None
}
