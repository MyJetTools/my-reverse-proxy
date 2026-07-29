use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{configurations::*, settings::*, settings_compiled::SettingsCompiled};

pub async fn compile_location_proxy_pass_to(
    settings_model: &SettingsCompiled,
    location_settings: &LocationSettings,
    listen_host: &str,
    resolved: &ResolvedTimeouts,
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

    // Every network location type compiles the same model out of
    // `proxy_pass_to` and differs only in which variant wraps it — the variant
    // is the only decision, so the parsing lives in one helper.
    let proxy_pass_to = match location_type {
        LocationType::UnixSocketHttp => ProxyPassToConfig::UnixHttp1(
            compile_model(location_settings, settings_model, resolved, "unix+http").await?,
        ),

        LocationType::UnixSocketHttp2 => ProxyPassToConfig::UnixHttp2(
            compile_model(location_settings, settings_model, resolved, "unix+http2").await?,
        ),

        LocationType::Http | LocationType::Https1 => ProxyPassToConfig::Http1(
            compile_model(location_settings, settings_model, resolved, "http").await?,
        ),

        LocationType::Mcp => ProxyPassToConfig::McpHttp1(
            compile_model(
                location_settings,
                settings_model,
                resolved,
                crate::consts::location_type::MCP,
            )
            .await?,
        ),

        LocationType::McpH2 => ProxyPassToConfig::McpHttp2(
            compile_model(
                location_settings,
                settings_model,
                resolved,
                crate::consts::location_type::MCP_H2,
            )
            .await?,
        ),

        LocationType::Http2 | LocationType::Https2 => ProxyPassToConfig::Http2(
            compile_model(location_settings, settings_model, resolved, "http2").await?,
        ),

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
        LocationType::Drop => ProxyPassToConfig::Drop,
        LocationType::DynamicProxy => ProxyPassToConfig::DynamicProxy(
            DynamicProxyConfig {
                request_timeout: resolved.request_timeout,
                connect_timeout: resolved.connect_timeout,
                allowed_hosts: location_settings.allowed_hosts.clone(),
            }
            .into(),
        ),
    };

    let result = ProxyPassLocationConfig::new(
        path,
        location_settings.modify_http_headers.clone(),
        location_settings.whitelisted_ip.clone(),
        proxy_pass_to,
        location_settings.domain_name.clone(),
        location_settings.get_compress(),
        location_settings.auth_header.clone(),
        listen_host,
    );

    Ok(result)
}

/// Parses `proxy_pass_to` into the upstream model shared by every network
/// location type. `location_type_name` only names the type in the error.
async fn compile_model(
    location_settings: &LocationSettings,
    settings_model: &SettingsCompiled,
    resolved: &ResolvedTimeouts,
    location_type_name: &str,
) -> Result<ProxyPassToModel, String> {
    let Some(proxy_pass_to) = location_settings.proxy_pass_to.as_ref() else {
        return Err(format!(
            "proxy_pass_to is required for {} location type",
            location_type_name
        ));
    };

    Ok(ProxyPassToModel {
        remote_host: MyReverseProxyRemoteEndpoint::try_parse(
            proxy_pass_to.as_str(),
            settings_model,
        )
        .await?,
        request_timeout: resolved.request_timeout,
        connect_timeout: resolved.connect_timeout,
        pool_tuning: PoolTuning::from_resolved(resolved),
    })
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
