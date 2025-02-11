use std::sync::Arc;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{
    configurations::SslCertificateIdRef,
    settings::{ClientCertificateCaSettings, SettingsModel},
    tcp_listener::https::ClientCertificateCa,
};

pub async fn refresh_ca_from_sources<'s>(
    settings_model: &SettingsModel,
    ca_id: SslCertificateIdRef<'s>,
) -> Result<(), String> {
    let client_certificates = match settings_model.client_certificate_ca.as_ref() {
        Some(ssl_certificates) => ssl_certificates,
        None => {
            return Err(format!(
                "Client certificate with id '{}' is not found",
                ca_id.as_str(),
            ));
        }
    };

    let mut found_certificate = None;

    for client_certificate in client_certificates {
        if client_certificate.id == ca_id.as_str() {
            found_certificate = Some(client_certificate);
            break;
        }
    }

    if found_certificate.is_none() {
        return Err(format!(
            "Client certificate with id '{}' is not found",
            ca_id.as_str(),
        ));
    }

    let client_certificate: &ClientCertificateCaSettings = found_certificate.unwrap();

    let file_src = super::apply_variables(settings_model, client_certificate.ca.as_str())?;
    let ca_file_src = OverSshConnectionSettings::try_parse(file_src.as_str()).ok_or(format!(
        "Invalid Client Certificate file source {}",
        file_src.as_str()
    ))?;

    let ca = super::load_file(&ca_file_src, crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT).await?;

    let crl = if let Some(crl_file_path) = client_certificate.revocation_list.as_ref() {
        let crl_file_path = super::apply_variables(settings_model, crl_file_path)?;
        let crl_file_src =
            OverSshConnectionSettings::try_parse(crl_file_path.as_str()).ok_or(format!(
                "Invalid Client Certificate CRL file source {}",
                crl_file_path.as_str()
            ))?;

        let crl =
            super::load_file(&crl_file_src, crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT).await?;

        let crl = my_tls::crl::read(crl.as_slice())?;

        Some((crl, crl_file_src))
    } else {
        None
    };

    let client_cert = ClientCertificateCa::from_bytes(ca.as_slice(), ca_file_src, crl)?;

    let client_cert = Arc::new(client_cert);

    crate::app::APP_CTX
        .ssl_certificates_cache
        .write(move |config| {
            config.client_ca.insert(ca_id, client_cert);
        })
        .await;

    Ok(())
}
