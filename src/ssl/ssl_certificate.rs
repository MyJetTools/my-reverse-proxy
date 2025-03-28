use std::sync::Arc;

use rust_extensions::date_time::DateTimeAsMicroseconds;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

use my_tls::tokio_rustls;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct SslCertInfo {
    pub cn: String,
    pub expires: DateTimeAsMicroseconds,
}

#[derive(Clone, Debug)]
pub struct SslCertificate {
    pub cert_key: Arc<tokio_rustls::rustls::sign::CertifiedKey>,
    cert_info: Arc<Mutex<Option<SslCertInfo>>>,
}

impl SslCertificate {
    pub fn new(private_key: Vec<u8>, certificates_content: Vec<u8>) -> Result<Self, String> {
        let private_key = super::certificates::load_private_key(private_key)?;

        let certs = super::certificates::load_certs(certificates_content);
        let cert_key = calc_cert_key(&private_key, certs);

        let result = SslCertificate {
            cert_key: Arc::new(cert_key),
            cert_info: Arc::new(Mutex::new(None)),
        };

        Ok(result)
    }

    pub async fn get_cert_info(&self) -> SslCertInfo {
        let mut cert_info = self.cert_info.lock().await;

        if let Some(cert_info) = &*cert_info {
            return cert_info.clone();
        }

        use x509_parser::prelude::FromDer;
        use x509_parser::prelude::X509Certificate;

        let mut found_cn = None;
        let mut expires = None;

        for cert_der in self.cert_key.cert.iter() {
            let (_, cert) = X509Certificate::from_der(cert_der).unwrap();

            let expires_from_cer = cert.validity().not_after.to_datetime().unix_timestamp();
            match expires {
                Some(expires_value) => {
                    if expires_from_cer < expires_value {
                        expires = Some(expires_from_cer);
                    }
                }
                None => {
                    expires = Some(expires_from_cer);
                }
            }

            for attr in cert.subject().iter_attributes() {
                // OID for Common Name
                if let Ok(cn) = attr.as_str() {
                    let value = match &found_cn {
                        Some(found_cn) => {
                            format!("{};{}", found_cn, cn)
                        }
                        None => cn.to_string(),
                    };

                    found_cn = Some(value);
                }
            }
        }

        let result = SslCertInfo {
            cn: found_cn.unwrap_or_else(|| "Unknown".to_string()),
            expires: DateTimeAsMicroseconds::from(expires.unwrap()),
        };

        if cert_info.is_some() {
            *cert_info = Some(result.clone());
        }

        result
    }

    pub fn get_certified_key(&self) -> Arc<tokio_rustls::rustls::sign::CertifiedKey> {
        self.cert_key.clone()
    }
}

pub fn calc_cert_key(
    private_key: &PrivateKeyDer<'static>,
    certificates: Vec<CertificateDer<'static>>,
) -> tokio_rustls::rustls::sign::CertifiedKey {
    let private_key =
        tokio_rustls::rustls::crypto::aws_lc_rs::sign::any_supported_type(private_key).unwrap();
    tokio_rustls::rustls::sign::CertifiedKey::new(certificates.clone(), private_key)
}
