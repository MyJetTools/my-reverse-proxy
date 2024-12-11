use std::sync::Arc;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{
    app::AppContext, configurations::SslCertificateIdRef, settings::SettingsModel,
    ssl::SslCertificate,
};

pub async fn refresh_ssl_certs_from_sources<'s>(
    app: &Arc<AppContext>,
    settings_model: &SettingsModel,
    ssl_cert_id: SslCertificateIdRef<'s>,
) -> Result<(), String> {
    let ssl_certificates = match settings_model.ssl_certificates.as_ref() {
        Some(ssl_certificates) => ssl_certificates,
        None => {
            return Err(format!(
                "SSL certificate with id '{}' not found",
                ssl_cert_id.as_str()
            ));
        }
    };

    let mut found_certificate = None;

    for ssl_certificate in ssl_certificates {
        if ssl_certificate.id.as_str() == ssl_cert_id.as_str() {
            found_certificate = Some(ssl_certificate);
            break;
        }
    }

    if found_certificate.is_none() {
        return Err(format!(
            "SSL certificate with id '{}' not found",
            ssl_cert_id.as_str()
        ));
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

    Ok(())
}
