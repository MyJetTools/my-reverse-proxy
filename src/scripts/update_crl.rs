use my_ssh::ssh_settings::OverSshConnectionSettings;

pub async fn update_crl(id: String, file_source: &OverSshConnectionSettings) {
    let crl = super::load_file(&file_source, crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT).await;

    if let Err(err) = &crl {
        println!("Error loading CRL file: {}", err);
        return;
    }

    let crl = crl.unwrap();

    let crl = match my_tls::crl::read(&crl) {
        Ok(crl) => crl,
        Err(err) => {
            println!("Error reading CRL file: {}", err);
            return;
        }
    };

    crate::app::APP_CTX
        .ssl_certificates_cache
        .write(|config| config.client_ca.update_crl(id, crl))
        .await;
}
