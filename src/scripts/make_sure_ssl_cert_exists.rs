use std::sync::Arc;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{
    app::AppContext,
    configurations::{SslCertificateId, SslCertificateIdRef},
    self_signed_cert::SELF_SIGNED_CERT_NAME,
    settings::*,
    ssl::SslCertificate,
};

pub async fn make_sure_ssl_cert_exists(
    app: &Arc<AppContext>,
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
        None => return Ok(SslCertificateId::new(SELF_SIGNED_CERT_NAME.to_string())),
    };

    let ssl_cert_id = SslCertificateIdRef::new(ssl_id);

    let ssl_cert_is_loaded = app
        .ssl_certificates_cache
        .read(|config| config.ssl_certs.has_certificate(ssl_cert_id))
        .await;

    if ssl_cert_is_loaded {
        return Ok(ssl_cert_id.into());
    }

    let ssl_certificates = match settings_model.ssl_certificates.as_ref() {
        Some(ssl_certificates) => ssl_certificates,
        None => {
            return Err(format!("SSL certificate with id '{}' not found", ssl_id));
        }
    };

    let mut found_certificate = None;

    for ssl_certificate in ssl_certificates {
        if ssl_certificate.id.as_str() == ssl_id {
            found_certificate = Some(ssl_certificate);
            break;
        }
    }

    if found_certificate.is_none() {
        return Err(format!("SSL certificate with id '{}' not found", ssl_id));
    }

    let ssl_certificate = found_certificate.unwrap();

    let private_key_file_src =
        super::apply_variables(settings_model, ssl_certificate.private_key.as_str())?;

    let private_key_src = OverSshConnectionSettings::try_parse(private_key_file_src.as_str())
        .ok_or(format!(
            "Invalid TLS Private Key file source {}",
            private_key_file_src.as_str()
        ))?;

    let private_key = super::load_file(app, &private_key_src).await?;

    let cert_src = super::apply_variables(settings_model, ssl_certificate.certificate.as_str())?;

    let cert_src = OverSshConnectionSettings::try_parse(cert_src.as_str()).ok_or(format!(
        "Invalid TLS Certificate Key file source {}",
        cert_src.as_str()
    ))?;

    let certificate = super::load_file(app, &cert_src).await?;

    let ssl_certificate = SslCertificate::new(private_key, certificate)?;

    app.ssl_certificates_cache
        .write(|config| {
            config
                .ssl_certs
                .add_or_update(ssl_cert_id, ssl_certificate, private_key_src, cert_src);
        })
        .await;

    Ok(ssl_cert_id.into())
}

/*
fn get_ssl_id<'s>(
    settings_model: &'s SettingsModel,
    host_endpoint: &EndpointHttpHostString,
    host_settings: &'s HostSettings,
) -> Result<Option<&'s str>, String> {
    if let Some(ssl_id) = host_settings.endpoint.ssl_certificate.as_ref() {
        return Ok(Some(ssl_id));
    }

    match super::get_endpoint_template(settings_model, host_endpoint, host_settings)? {
        Some(endpoint_template_settings) => {
            let ssl_cert = endpoint_template_settings.ssl_certificate.as_deref();
            return Ok(ssl_cert);
        }
        None => {
            return Ok(None);
        }
    }
}
 */
