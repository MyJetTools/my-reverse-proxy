use crate::{
    configurations::{SslCertificateId, SslCertificateIdRef},
    settings::*,
    settings_compiled::SettingsCompiled,
};

pub async fn make_sure_ssl_cert_exists(
    settings_model: &SettingsCompiled,
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

    // The certificate is not loaded yet. Try to resolve it from the configured source, but do
    // NOT fail the whole endpoint if that fails (no source in settings, source unreachable, or
    // no settings entry at all). The endpoint must stay usable so the certificate can arrive in
    // the background — an upload via /api/SslCertificates/Init or a gateway push. Until it does,
    // the TLS handshake for this endpoint is cut per-connection (the cert is absent from the
    // cache) without penalising the waiting client.
    if let Err(err) = super::refresh_ssl_certs_from_sources(settings_model, ssl_cert_id).await {
        println!(
            "Endpoint keeps listening without TLS certificate '{}' (connections are cut until it arrives): {}",
            ssl_cert_id.as_str(),
            err
        );
    }

    Ok(ssl_cert_id.into())
}
