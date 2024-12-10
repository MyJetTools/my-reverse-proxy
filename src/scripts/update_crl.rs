use std::sync::Arc;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use crate::app::AppContext;

pub async fn update_crl(
    app: &Arc<AppContext>,
    id: String,
    file_source: &OverSshConnectionSettings,
) {
    let crl = super::load_file(app, &file_source).await;

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

    app.ssl_certificates_cache
        .write(|config| config.client_ca.update_crl(id, crl))
        .await;
}
