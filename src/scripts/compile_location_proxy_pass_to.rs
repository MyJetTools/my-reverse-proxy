use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{configurations::*, settings::*, settings_compiled::SettingsCompiled};

pub async fn compile_location_proxy_pass_to(
    settings_model: &SettingsCompiled,
    location_settings: &LocationSettings,
) -> Result<ProxyPassLocationConfig, String> {
    let path = location_settings
        .path
        .as_ref()
        .map(|itm| itm.as_str())
        .unwrap_or("/")
        .to_string();

    let location_type = match location_settings.get_location_type()? {
        Some(location_type) => location_type,
        None => LocationType::detect_from_location_settings(location_settings)?,
    };

    let proxy_pass_to = match location_type {
        LocationType::UnixSocketHttp => {
            if location_settings.proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for http location type".to_string());
            }

            let proxy_pass_to = location_settings.proxy_pass_to.clone().unwrap();

            ProxyPassToConfig::UnixHttp1(ProxyPassToModel {
                remote_host: MyReverseProxyRemoteEndpoint::try_parse(
                    proxy_pass_to.as_str(),
                    settings_model,
                )
                .await?,
                request_timeout: location_settings.get_request_timeout(),
                connect_timeout: location_settings.get_connect_timeout(),
                is_mcp: false,
            })
        }

        LocationType::UnixSocketHttp2 => {
            if location_settings.proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for http location type".to_string());
            }

            let proxy_pass_to = location_settings.proxy_pass_to.clone().unwrap();

            ProxyPassToConfig::UnixHttp2(ProxyPassToModel {
                remote_host: MyReverseProxyRemoteEndpoint::try_parse(
                    proxy_pass_to.as_str(),
                    settings_model,
                )
                .await?,
                request_timeout: location_settings.get_request_timeout(),
                connect_timeout: location_settings.get_connect_timeout(),
                is_mcp: false,
            })
        }
        LocationType::Http => {
            if location_settings.proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for http location type".to_string());
            }

            let proxy_pass_to = location_settings.proxy_pass_to.clone().unwrap();

            ProxyPassToConfig::Http1(ProxyPassToModel {
                remote_host: MyReverseProxyRemoteEndpoint::try_parse(
                    proxy_pass_to.as_str(),
                    settings_model,
                )
                .await?,
                request_timeout: location_settings.get_request_timeout(),
                connect_timeout: location_settings.get_connect_timeout(),
                is_mcp: false,
            })
        }
        LocationType::Mcp => {
            if location_settings.proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for mcp location type".to_string());
            }

            let proxy_pass_to = location_settings.proxy_pass_to.clone().unwrap();

            ProxyPassToConfig::Http1(ProxyPassToModel {
                remote_host: MyReverseProxyRemoteEndpoint::try_parse(
                    proxy_pass_to.as_str(),
                    settings_model,
                )
                .await?,
                request_timeout: location_settings.get_request_timeout(),
                connect_timeout: location_settings.get_connect_timeout(),
                is_mcp: true,
            })
        }
        LocationType::Http2 => {
            if location_settings.proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for http2 location type".to_string());
            }

            let proxy_pass_to = location_settings.proxy_pass_to.clone().unwrap();

            ProxyPassToConfig::Http2(ProxyPassToModel {
                remote_host: MyReverseProxyRemoteEndpoint::try_parse(
                    proxy_pass_to.as_str(),
                    settings_model,
                )
                .await?,
                request_timeout: location_settings.get_request_timeout(),
                connect_timeout: location_settings.get_connect_timeout(),
                is_mcp: false,
            })
        }
        LocationType::Https1 => {
            if location_settings.proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for http2 location type".to_string());
            }

            let proxy_pass_to = location_settings.proxy_pass_to.clone().unwrap();

            ProxyPassToConfig::Http1(ProxyPassToModel {
                remote_host: MyReverseProxyRemoteEndpoint::try_parse(
                    proxy_pass_to.as_str(),
                    settings_model,
                )
                .await?,
                request_timeout: location_settings.get_request_timeout(),
                connect_timeout: location_settings.get_connect_timeout(),
                is_mcp: false,
            })
        }
        LocationType::Https2 => {
            if location_settings.proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for http2 location type".to_string());
            }

            let proxy_pass_to = location_settings.proxy_pass_to.clone().unwrap();

            ProxyPassToConfig::Http2(ProxyPassToModel {
                remote_host: MyReverseProxyRemoteEndpoint::try_parse(
                    proxy_pass_to.as_str(),
                    settings_model,
                )
                .await?,
                request_timeout: location_settings.get_request_timeout(),
                connect_timeout: location_settings.get_connect_timeout(),
                is_mcp: false,
            })
        }
        LocationType::Files => {
            if location_settings.proxy_pass_to.is_none() {
                return Err("proxy_pass_to is required for files location type".to_string());
            }

            let proxy_pass_to = location_settings.proxy_pass_to.clone().unwrap();
            let files_path =
                MyReverseProxyRemoteEndpoint::try_parse(proxy_pass_to.as_str(), settings_model)
                    .await?;

            let model = ProxyPassFilesPathModel {
                files_path,
                default_file: location_settings.default_file.clone(),
            };

            ProxyPassToConfig::FilesPath(model)
        }
        LocationType::StaticContent => {
            let body = location_settings.body.clone().unwrap_or_default();

            let body = get_static_content_body(body).await?;
            let model: StaticContentConfig = StaticContentConfig {
                status_code: location_settings.status_code.unwrap_or(200),
                content_type: location_settings.content_type.clone(),
                body,
            };

            ProxyPassToConfig::Static(model.into())
        }
    };

    let result = ProxyPassLocationConfig::new(
        path,
        location_settings.modify_http_headers.clone(),
        location_settings.whitelisted_ip.clone(),
        proxy_pass_to,
        location_settings.domain_name.clone(),
        location_settings.get_compress(),
        location_settings.get_trace_payload(),
    );

    Ok(result)
}

async fn get_static_content_body(body: String) -> Result<Vec<u8>, String> {
    if body.is_empty() {
        return Ok(Vec::new());
    }
    match get_fist_char(body.as_str()) {
        Some(c) => {
            if c == '<' {
                return Ok(body.into_bytes());
            }

            if c == '{' {
                return Ok(body.into_bytes());
            }

            match OverSshConnectionSettings::try_parse(body.as_str()) {
                Some(data_source) => {
                    super::load_file(&data_source, crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT)
                        .await
                }
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
