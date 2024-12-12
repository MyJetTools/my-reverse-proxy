use std::sync::Arc;

use crate::{
    app::AppContext,
    configurations::{SslCertificateId, SslCertificateIdRef},
    settings::*,
};

pub async fn make_sure_client_ca_exists<'s>(
    app: &Arc<AppContext>,
    settings_model: &'s SettingsModel,
    host_settings: &'s HostSettings,
) -> Result<Option<SslCertificateId>, String> {
    let client_ca_id = super::get_from_host_or_templates(
        settings_model,
        host_settings,
        |host_settings| host_settings.endpoint.client_certificate_ca.as_ref(),
        |templates| templates.client_certificate_ca.as_ref(),
    )?;

    if client_ca_id.is_none() {
        return Ok(None);
    }

    let client_ca_id = client_ca_id.unwrap();

    let client_ca_id = SslCertificateIdRef::new(client_ca_id);

    let client_ca_is_loaded = app
        .ssl_certificates_cache
        .read(|config| config.client_ca.has_certificate(client_ca_id))
        .await;

    if client_ca_is_loaded {
        return Ok(Some(client_ca_id.into()));
    }

    super::refresh_ca_from_sources(app, settings_model, client_ca_id).await?;

    Ok(Some(client_ca_id.into()))
}
