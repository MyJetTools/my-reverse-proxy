use std::sync::Arc;

use rust_extensions::{date_time::DateTimeAsMicroseconds, MyTimerTick};

use crate::{
    configurations::SslCertificateId,
    ssl::{SslCertificate, SslCertificateHolder, SslCertificateOrigin},
};

pub struct SslCertsRefreshTimer;

#[async_trait::async_trait]
impl MyTimerTick for SslCertsRefreshTimer {
    async fn tick(&self) {
        let ssl_certs = crate::app::APP_CTX
            .ssl_certificates_cache
            .read(|itm| itm.ssl_certs.get_list())
            .await;

        if ssl_certs.len() == 0 {
            return;
        }

        let now = DateTimeAsMicroseconds::now();
        for (cert_id, ssl_cert) in ssl_certs {
            try_renew_cert(cert_id.into(), ssl_cert, now).await;
        }
    }
}

async fn try_renew_cert(
    cert_id: SslCertificateId,
    ssl_holder: Arc<SslCertificateHolder>,
    now: DateTimeAsMicroseconds,
) {
    let (private_key_src, cert_src) = match &ssl_holder.origin {
        SslCertificateOrigin::LocalSource {
            private_key_src,
            cert_src,
        } => (private_key_src.clone(), cert_src.clone()),
        SslCertificateOrigin::GatewayPushed { .. } => {
            return;
        }
    };

    let ssl_cert_info = ssl_holder.ssl_cert.get_cert_info();

    let expires_in = ssl_cert_info.expires.duration_since(now);

    let days = expires_in.get_full_days();

    if days > 7 {
        println!(
            "Certificate {} is ok. Days to expire: {days}. No need to renew.",
            cert_id.as_str()
        );
        return;
    }

    let certificates_content =
        crate::scripts::load_file(&cert_src, crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT).await;

    if let Err(err) = &certificates_content {
        println!(
            "Error loading certificate {}. Err:{}",
            cert_id.as_str(),
            err
        );
        return;
    }

    let certificates_content = certificates_content.unwrap();

    let private_key_content = crate::scripts::load_file(
        &private_key_src,
        crate::consts::DEFAULT_HTTP_CONNECT_TIMEOUT,
    )
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

    let ssl_cert = SslCertificate::new(private_key_content.clone(), certificates_content.clone());

    if let Err(err) = &ssl_cert {
        println!(
            "Error creating certificate {}. Err:{}",
            cert_id.as_str(),
            err
        );
        return;
    }

    let ssl_cert = ssl_cert.unwrap();

    let origin = SslCertificateOrigin::LocalSource {
        private_key_src,
        cert_src,
    };

    crate::app::APP_CTX
        .ssl_certificates_cache
        .write(|config| {
            config.ssl_certs.add_or_update(
                cert_id.as_ref(),
                ssl_cert,
                origin,
                certificates_content,
                private_key_content,
            );
        })
        .await;

    println!("Certificate {} has been renewed.", cert_id.as_str());
}
