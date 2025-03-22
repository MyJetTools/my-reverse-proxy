use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{
    configurations::SslCertificateIdRef, settings_compiled::SettingsCompiled, ssl::SslCertificate,
};

pub async fn refresh_ssl_certs_from_sources<'s>(
    settings_model: &SettingsCompiled,
    ssl_cert_id: SslCertificateIdRef<'s>,
) -> Result<(), String> {
    let mut found_certificate = None;

    for ssl_certificate in settings_model.ssl_certificates.iter() {
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

    let private_key_src = OverSshConnectionSettings::try_parse(
        ssl_certificate.private_key.as_str(),
    )
    .ok_or(format!(
        "Invalid TLS Private Key file source {}",
        ssl_certificate.private_key.as_str()
    ))?;

    let private_key = super::load_file(
        &private_key_src,
        crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT,
    )
    .await?;

    let cert_src = OverSshConnectionSettings::try_parse(ssl_certificate.certificate.as_str())
        .ok_or(format!(
            "Invalid TLS Certificate Key file source {}",
            ssl_certificate.certificate.as_str()
        ))?;

    let certificate =
        super::load_file(&cert_src, crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT).await?;

    let ssl_certificate = SslCertificate::new(private_key, certificate)?;

    crate::app::APP_CTX
        .ssl_certificates_cache
        .write(|config| {
            config
                .ssl_certs
                .add_or_update(ssl_cert_id, ssl_certificate, private_key_src, cert_src);
        })
        .await;

    Ok(())
}
