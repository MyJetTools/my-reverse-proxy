use std::sync::Arc;

use rust_extensions::{date_time::DateTimeAsMicroseconds, MyTimerTick};

use crate::{
    app::AppContext,
    configurations::{AppConfiguration, SslCertificateId},
    ssl::{SslCertificate, SslCertificateHolder},
};

pub struct SslCertsRefreshTimer {
    app: Arc<AppContext>,
}

impl SslCertsRefreshTimer {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl MyTimerTick for SslCertsRefreshTimer {
    async fn tick(&self) {
        let configuration = self.app.try_get_current_app_configuration().await;

        if configuration.is_none() {
            return;
        }

        let configuration = configuration.unwrap();

        let ssl_certs = configuration.ssl_certificates_cache.get_list().await;

        let now = DateTimeAsMicroseconds::now();
        for (cert_id, ssl_cert) in ssl_certs {
            try_renew_cert(&configuration, cert_id.into(), ssl_cert, now).await;
        }
    }
}

async fn try_renew_cert(
    configuration: &AppConfiguration,
    cert_id: SslCertificateId,
    ssl_holder: Arc<SslCertificateHolder>,
    now: DateTimeAsMicroseconds,
) {
    let ssl_cert_info = ssl_holder.ssl_cert.get_cert_info().await;

    let expires_in = ssl_cert_info.expires.duration_since(now);

    let days = expires_in.get_full_days();

    if days > 7 {
        println!(
            "Certificate {} is ok. Days to expire: {days}. No need to renew.",
            cert_id.as_str()
        );
        return;
    }

    let certificates_content = ssl_holder.cert_src.load_file_content(None, false).await;

    if let Err(err) = &certificates_content {
        println!(
            "Error loading certificate {}. Err:{}",
            cert_id.as_str(),
            err
        );
        return;
    }

    let certificates_content = certificates_content.unwrap();

    let private_key_content = ssl_holder
        .private_key_src
        .load_file_content(None, false)
        .await;

    if let Err(err) = &private_key_content {
        println!(
            "Error loading private_key {}. Err:{}",
            cert_id.as_str(),
            err
        );
        return;
    }

    let private_key_content = private_key_content.unwrap();

    let ssl_cert = SslCertificate::new(private_key_content, certificates_content);

    if let Err(err) = &ssl_cert {
        println!(
            "Error creating certificate {}. Err:{}",
            cert_id.as_str(),
            err
        );
        return;
    }

    let ssl_cert = ssl_cert.unwrap();

    configuration
        .ssl_certificates_cache
        .add_or_update(
            &cert_id,
            ssl_cert,
            ssl_holder.private_key_src.clone(),
            ssl_holder.cert_src.clone(),
        )
        .await;

    println!("Certificate {} has been renewed.", cert_id.as_str());
}
