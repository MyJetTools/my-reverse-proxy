use std::sync::Arc;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::{
    app::AppContext,
    configurations::{EndpointHttpHostString, SslCertificateId, SslCertificateIdRef},
    settings::*,
    tcp_listener::https::ClientCertificateCa,
};

pub async fn make_sure_client_ca_exists<'s>(
    app: &Arc<AppContext>,
    settings_model: &'s SettingsModel,
    host_endpoint: &EndpointHttpHostString,
    host_settings: &'s HostSettings,
) -> Result<Option<SslCertificateId>, String> {
    let client_ca_id = super::get_from_host_or_templates(
        settings_model,
        host_endpoint,
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

    let client_certificates = match settings_model.client_certificate_ca.as_ref() {
        Some(ssl_certificates) => ssl_certificates,
        None => {
            return Err(format!(
                "Client certificate with id {} not found for endpoint {}",
                client_ca_id.as_str(),
                host_endpoint.as_str()
            ));
        }
    };

    let mut found_certificate = None;

    for client_certificate in client_certificates {
        if client_certificate.id == client_ca_id.as_str() {
            found_certificate = Some(client_certificate);
            break;
        }
    }

    if found_certificate.is_none() {
        return Err(format!(
            "Client certificate with id {} not found for endpoint {}",
            client_ca_id.as_str(),
            host_endpoint.as_str()
        ));
    }

    let client_certificate: &ClientCertificateCaSettings = found_certificate.unwrap();

    let file_src = super::apply_variables(settings_model, client_certificate.ca.as_str())?;
    let ca_file_src = OverSshConnectionSettings::try_parse(file_src.as_str()).ok_or(format!(
        "Invalid Client Certificate file source {}",
        file_src.as_str()
    ))?;

    let ca = super::load_file(app, &ca_file_src).await?;

    let crl = if let Some(crl_file_path) = client_certificate.revocation_list.as_ref() {
        let crl_file_path = super::apply_variables(settings_model, crl_file_path)?;
        let crl_file_src =
            OverSshConnectionSettings::try_parse(crl_file_path.as_str()).ok_or(format!(
                "Invalid Client Certificate CRL file source {}",
                crl_file_path.as_str()
            ))?;

        let crl = super::load_file(app, &crl_file_src).await?;

        let crl = my_tls::crl::read(crl.as_slice())?;

        Some((crl, crl_file_src))
    } else {
        None
    };

    let client_cert = ClientCertificateCa::from_bytes(ca.as_slice(), ca_file_src, crl)?;

    let client_cert = Arc::new(client_cert);

    app.ssl_certificates_cache
        .write(move |config| {
            config.client_ca.insert(client_ca_id, client_cert);
        })
        .await;

    Ok(Some(client_ca_id.into()))
}
