use std::sync::Arc;

use my_ssh::ssh_settings::OverSshConnectionSettings;

use rustls_pki_types::CertificateDer;
use x509_parser::{certificate::X509Certificate, der_parser::asn1_rs::FromDer};

use my_tls::{crl::CrlRecord, tokio_rustls::rustls};

#[derive(Debug, Clone)]
pub struct ClientCertificateData {
    pub cn: String,
}

pub struct ClientCertificateCa {
    ca_content: CertificateDer<'static>,
    names: Vec<rustls::DistinguishedName>,
    pub ca_file_source: OverSshConnectionSettings,
    pub crl_list: Vec<CrlRecord>,
    pub crl_file_source: Option<OverSshConnectionSettings>,
}

impl ClientCertificateCa {
    pub fn from_bytes(
        value: &[u8],
        file_source: OverSshConnectionSettings,
        crl: Option<(Vec<CrlRecord>, OverSshConnectionSettings)>,
    ) -> Result<Self, String> {
        let mut reader = std::io::BufReader::new(value);

        let certs = rustls_pemfile::certs(&mut reader);

        // Load and return certificate.
        let mut result = Vec::new();

        for cert in certs {
            let cert: rustls_pki_types::CertificateDer<'_> = cert.unwrap();
            result.push(cert);
        }

        if let Some(crl) = crl {
            Self::new(result, file_source, crl.0, Some(crl.1))
        } else {
            Self::new(result, file_source, vec![], None)
        }
    }
    pub fn new(
        mut certs: Vec<CertificateDer<'static>>,
        ca_file_source: OverSshConnectionSettings,
        crl_list: Vec<CrlRecord>,
        crl_file_source: Option<OverSshConnectionSettings>,
    ) -> Result<Self, String> {
        let mut names = Vec::new();

        for ca in &certs {
            let (_, cert) = X509Certificate::from_der(ca).unwrap();

            let issuer = cert.issuer();

            //println!("Issuer: {}", issuer.);
            names.push(issuer.as_raw().to_vec().into());
        }

        let resut = Self {
            ca_content: certs.remove(0),
            names,
            ca_file_source,
            crl_list,
            crl_file_source,
        };

        Ok(resut)
    }

    pub fn verify_cert(
        &self,
        certificate_to_check: &rustls_pki_types::CertificateDer,
    ) -> Option<Arc<ClientCertificateData>> {
        let (_, issuer) = X509Certificate::from_der(self.ca_content.as_ref()).unwrap();

        let (_, cert_to_check) = X509Certificate::from_der(certificate_to_check.as_ref()).unwrap();

        let cn = cert_to_check
            .subject()
            .iter_common_name()
            .next()
            .map(|cn| cn.as_str().unwrap().to_string())
            .unwrap_or_default();

        if cert_to_check
            .verify_signature(Some(issuer.public_key()))
            .is_ok()
        {
            for crl_record in self.crl_list.iter() {
                if crl_record.serial == cert_to_check.serial {
                    return None;
                }
            }

            let cert_data = ClientCertificateData { cn };

            let cert_data = Arc::new(cert_data);

            return Some(cert_data);
        }

        None
    }

    pub fn get_names(&self) -> &[rustls::DistinguishedName] {
        &self.names
    }

    pub fn update_crl(&self, crl: Vec<CrlRecord>) -> Self {
        Self {
            ca_content: self.ca_content.clone(),
            names: self.names.clone(),
            ca_file_source: self.ca_file_source.clone(),
            crl_list: crl,
            crl_file_source: self.crl_file_source.clone(),
        }
    }
}

/*
pub fn get_cert_data(
    ca_content: CertificateDer<'static>,
    certificate_to_check: &rustls_pki_types::CertificateDer,
) -> Result<ClientCertificateData, String> {
    let (_, issuer) = X509Certificate::from_der(ca_content.as_ref()).unwrap();

    let (_, cert_to_check) = X509Certificate::from_der(certificate_to_check.as_ref()).unwrap();

    cert_to_check
        .verify_signature(Some(issuer.public_key()))
        .map_err(|err| format!("{:?}", err))?;

    for itm in cert_to_check.tbs_certificate.subject().iter_common_name() {
        return Ok(ClientCertificateData {
            cn: itm.as_str().unwrap().to_string(),
            serial: cert_to_check.serial.clone(),
        });
    }

    return Err("No certificate data found".to_string());
}
 */
