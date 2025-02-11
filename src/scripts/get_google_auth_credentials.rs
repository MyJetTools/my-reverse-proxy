use crate::{configurations::GoogleAuthCredentials, settings::*};

pub async fn get_google_auth_credentials(
    settings_model: &SettingsModel,
    host_settings: &HostSettings,
) -> Result<Option<String>, String> {
    let google_auth_id = super::get_from_host_or_templates(
        settings_model,
        host_settings,
        |host_settings| host_settings.endpoint.google_auth.as_ref(),
        |templates| templates.google_auth.as_ref(),
    )?;

    if google_auth_id.is_none() {
        return Ok(None);
    }

    let google_auth_id = google_auth_id.unwrap();

    if crate::app::APP_CTX
        .current_configuration
        .get(|config| {
            config
                .google_auth_credentials
                .has_credentials(google_auth_id)
        })
        .await
    {
        return Ok(Some(google_auth_id.to_string()));
    }

    let g_auth_list = match settings_model.g_auth.as_ref() {
        Some(g_auth_list) => g_auth_list,
        None => {
            return Err(format!(
                "Google Auth Credentials {} not found",
                google_auth_id
            ));
        }
    };

    match g_auth_list.get(google_auth_id) {
        Some(g_auth_settings) => {
            let google_auth_credentials = GoogleAuthCredentials {
                client_id: g_auth_settings.client_id.to_string(),
                client_secret: g_auth_settings.client_secret.to_string(),
                whitelisted_domains: g_auth_settings.whitelisted_domains.clone(), // make domains_check
            };

            crate::app::APP_CTX
                .current_configuration
                .write(|config| {
                    config
                        .google_auth_credentials
                        .add_or_update(google_auth_id.to_string(), google_auth_credentials)
                })
                .await;

            return Ok(Some(google_auth_id.to_string()));
        }
        None => {
            return Err(format!(
                "Google Auth Credentials {} not found",
                google_auth_id
            ));
        }
    }
}
