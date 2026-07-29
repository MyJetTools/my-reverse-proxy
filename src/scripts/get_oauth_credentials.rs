use crate::{
    oauth::{default_signing_key_file, OAuthContext, SigningKeyFile},
    settings::*,
    settings_compiled::SettingsCompiled,
};

/// Resolves the `oauth:` block an endpoint refers to and makes sure it is loaded
/// into the running configuration, returning the id to store on the endpoint.
pub async fn get_oauth_credentials(
    settings_model: &SettingsCompiled,
    host_settings: &HostSettings,
) -> Result<Option<String>, String> {
    let oauth_id = super::get_from_host_or_templates(
        settings_model,
        host_settings,
        |host_settings| host_settings.endpoint.oauth.as_ref(),
        |templates| templates.oauth.as_ref(),
    )?;

    let Some(oauth_id) = oauth_id else {
        return Ok(None);
    };

    let Some(oauth_settings) = settings_model.oauth.get(oauth_id) else {
        return Err(format!(
            "OAuth settings '{}' are not found. Add an 'oauth:' block with that name",
            oauth_id
        ));
    };

    validate(oauth_id, oauth_settings)?;

    // An unchanged block keeps the context it already has, along with the
    // authorization codes waiting in it.
    if crate::app::APP_CTX
        .current_configuration
        .get(|config| {
            config
                .oauth_credentials
                .is_up_to_date(oauth_id, oauth_settings)
        })
        .await
    {
        return Ok(Some(oauth_id.to_string()));
    }

    let signing_key = resolve_signing_key(oauth_id, oauth_settings).await?;

    let context = OAuthContext::new(oauth_settings, signing_key);

    crate::app::APP_CTX
        .current_configuration
        .write(|config| {
            config
                .oauth_credentials
                .add_or_update(oauth_id.to_string(), context)
        })
        .await;

    Ok(Some(oauth_id.to_string()))
}

fn validate(oauth_id: &str, settings: &OAuthSettings) -> Result<(), String> {
    for (field, value) in [
        ("client_id", &settings.client_id),
        ("client_secret", &settings.client_secret),
        ("consent_password", &settings.consent_password),
    ] {
        if value.trim().is_empty() {
            return Err(format!(
                "OAuth settings '{}' have an empty '{}'. Claude sends the client id and secret on \
                 every token request and an empty one would match any client",
                oauth_id, field
            ));
        }
    }

    Ok(())
}

/// The key the stateless tokens are signed with: whatever the settings pin, or
/// one generated once and kept on disk.
///
/// It has to be stable across restarts — tokens carry their own claims and are
/// checked by signature alone, so a new key logs every connector out.
async fn resolve_signing_key(oauth_id: &str, settings: &OAuthSettings) -> Result<Vec<u8>, String> {
    if let Some(signing_key) = settings.signing_key.as_deref() {
        let signing_key = signing_key.trim();

        if !signing_key.is_empty() {
            // Taken as raw bytes rather than decoded: the value is a secret from
            // the settings file (often through a ${variable}), and HMAC accepts a
            // key of any length, so there is nothing to be gained by demanding a
            // particular encoding.
            return Ok(signing_key.as_bytes().to_vec());
        }
    }

    let path = settings
        .signing_key_file
        .clone()
        .unwrap_or_else(|| default_signing_key_file(oauth_id));

    let path = rust_extensions::file_utils::format_path(path.as_str()).to_string();

    SigningKeyFile::load_or_create(std::path::Path::new(path.as_str())).await
}
