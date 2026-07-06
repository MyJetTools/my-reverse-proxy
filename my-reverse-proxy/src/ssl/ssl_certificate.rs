use std::sync::Arc;

use arc_swap::ArcSwapOption;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

use my_tls::tokio_rustls;

#[derive(Debug, Clone)]
pub struct SslCertInfo {
    pub cn: String,
    pub expires: DateTimeAsMicroseconds,
}

#[derive(Clone, Debug)]
pub struct SslCertificate {
    pub cert_key: Arc<tokio_rustls::rustls::sign::CertifiedKey>,
    cert_info: Arc<ArcSwapOption<SslCertInfo>>,
}

impl SslCertificate {
    pub fn new(private_key: Vec<u8>, certificates_content: Vec<u8>) -> Result<Self, String> {
        let private_key = super::certificates::load_private_key(private_key)?;

        let certs = super::certificates::load_certs(certificates_content)?;
        let cert_key = calc_cert_key(&private_key, certs)?;

        // Ensure the private key actually matches the leaf certificate. rustls'
        // `CertifiedKey::new` does NOT verify this, so a valid-but-unrelated key would
        // otherwise be accepted and then break every TLS handshake once served. Only a
        // definite mismatch is fatal; if the key type can't expose its SPKI (`Unknown`) we
        // don't block (best effort — RSA/ECDSA/Ed25519 all expose it).
        if let Err(tokio_rustls::rustls::Error::InconsistentKeys(
            tokio_rustls::rustls::InconsistentKeys::KeyMismatch,
        )) = cert_key.keys_match()
        {
            return Err("The provided private key does not match the certificate".to_string());
        }

        let result = SslCertificate {
            cert_key: Arc::new(cert_key),
            cert_info: Arc::new(ArcSwapOption::empty()),
        };

        Ok(result)
    }

    pub fn get_cert_info(&self) -> SslCertInfo {
        if let Some(cert_info) = self.cert_info.load_full() {
            return (*cert_info).clone();
        }

        use x509_parser::prelude::FromDer;
        use x509_parser::prelude::X509Certificate;

        let mut found_cn = None;
        let mut expires = None;

        for cert_der in self.cert_key.cert.iter() {
            let cert = match X509Certificate::from_der(cert_der) {
                Ok((_, cert)) => cert,
                Err(_) => continue,
            };

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

            for attr in cert.subject().iter_common_name() {
                // Common Name attributes only (OID 2.5.4.3) — not the whole subject DN.
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
            // `expires` is unix SECONDS from the x509 not_after. Build micros explicitly:
            // DateTimeAsMicroseconds::from(i64) guesses the unit by magnitude and misreads
            // timestamps past ~year 2120 (e.g. the RFC 5280 "no expiry" 9999 value).
            expires: DateTimeAsMicroseconds::new(expires.unwrap_or(0) * 1_000_000),
        };

        self.cert_info.store(Some(Arc::new(result.clone())));

        result
    }

    pub fn get_certified_key(&self) -> Arc<tokio_rustls::rustls::sign::CertifiedKey> {
        self.cert_key.clone()
    }

    /// Domains the leaf certificate is valid for: DNS entries from the Subject
    /// Alternative Name extension plus the Common Name. Used to verify an
    /// uploaded certificate actually protects the endpoint's domain before it
    /// replaces what is served.
    pub fn get_domains(&self) -> Vec<String> {
        use x509_parser::prelude::FromDer;
        use x509_parser::prelude::GeneralName;
        use x509_parser::prelude::X509Certificate;

        let mut result: Vec<String> = Vec::new();

        let Some(leaf) = self.cert_key.cert.first() else {
            return result;
        };

        let Ok((_, cert)) = X509Certificate::from_der(leaf) else {
            return result;
        };

        if let Ok(Some(san)) = cert.subject_alternative_name() {
            for general_name in san.value.general_names.iter() {
                if let GeneralName::DNSName(dns) = general_name {
                    push_unique_domain(&mut result, dns);
                }
            }
        }

        for cn in cert.subject().iter_common_name() {
            if let Ok(cn) = cn.as_str() {
                push_unique_domain(&mut result, cn);
            }
        }

        result
    }
}

fn push_unique_domain(result: &mut Vec<String>, name: &str) {
    if !result.iter().any(|itm| itm.eq_ignore_ascii_case(name)) {
        result.push(name.to_string());
    }
}

pub fn calc_cert_key(
    private_key: &PrivateKeyDer<'static>,
    certificates: Vec<CertificateDer<'static>>,
) -> Result<tokio_rustls::rustls::sign::CertifiedKey, String> {
    let private_key = tokio_rustls::rustls::crypto::aws_lc_rs::sign::any_supported_type(private_key)
        .map_err(|err| format!("Unsupported private key: {:?}", err))?;
    Ok(tokio_rustls::rustls::sign::CertifiedKey::new(
        certificates,
        private_key,
    ))
}
