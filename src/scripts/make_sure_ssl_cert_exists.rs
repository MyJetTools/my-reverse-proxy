use crate::{
    configurations::{SslCertificateId, SslCertificateIdRef},
    settings::*,
};

pub async fn make_sure_ssl_cert_exists(
    settings_model: &SettingsModel,
    host_settings: &HostSettings,
) -> Result<SslCertificateId, String> {
    let ssl_id = match super::get_from_host_or_templates(
        settings_model,
        host_settings,
        |host_settings| host_settings.endpoint.ssl_certificate.as_ref(),
        |templates| templates.ssl_certificate.as_ref(),
    )? {
        Some(ssl_id) => ssl_id,
        None => return Ok(SslCertificateId::new_as_self_signed()),
    };

    let ssl_cert_id = SslCertificateIdRef::new(ssl_id);

    if ssl_cert_id.is_self_signed() {
        return Ok(ssl_cert_id.into());
    }

    let ssl_cert_is_loaded = crate::app::APP_CTX
        .ssl_certificates_cache
        .read(|config| config.ssl_certs.has_certificate(ssl_cert_id))
        .await;

    if ssl_cert_is_loaded {
        return Ok(ssl_cert_id.into());
    }

    super::refresh_ssl_certs_from_sources(settings_model, ssl_cert_id).await?;

    Ok(ssl_cert_id.into())
}
