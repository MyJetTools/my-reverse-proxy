use rustls_pki_types::CertificateDer;
use x509_parser::{
    certificate::X509Certificate, der_parser::asn1_rs::FromDer, num_bigint::BigUint,
};

use crate::configurations::SslCertificateId;

#[derive(Debug, Clone)]
pub struct ClientCertificateData {
    pub ca_id: SslCertificateId,
    pub cn: String,
    pub serial: BigUint,
}

pub struct ClientCertificateCa {
    ca_content: CertificateDer<'static>,
    names: Vec<tokio_rustls::rustls::DistinguishedName>,
}

impl ClientCertificateCa {
    pub fn new(mut certs: Vec<CertificateDer<'static>>) -> Self {
        let mut names = Vec::new();

        for ca in &certs {
            let (_, cert) = X509Certificate::from_der(ca).unwrap();

            let issuer = cert.issuer();

            //println!("Issuer: {}", issuer.);
            names.push(issuer.as_raw().to_vec().into());
        }

        Self {
            ca_content: certs.remove(0),
            names,
        }
    }

    pub fn check_certificate(
        &self,
        ca_id: &SslCertificateId,
        certificate_to_check: &rustls_pki_types::CertificateDer,
    ) -> Option<ClientCertificateData> {
        let (_, issuer) = X509Certificate::from_der(self.ca_content.as_ref()).unwrap();

        let (_, cert_to_check) = X509Certificate::from_der(certificate_to_check.as_ref()).unwrap();

        let result = cert_to_check
            .verify_signature(Some(issuer.public_key()))
            .is_ok();

        if !result {
            return None;
        }

        for itm in cert_to_check.tbs_certificate.subject().iter_common_name() {
            return Some(ClientCertificateData {
                ca_id: ca_id.clone(),
                cn: itm.as_str().unwrap().to_string(),
                serial: cert_to_check.serial.clone(),
            });
        }

        return None;
        //println!("{:?}", result);

        /*
        for ca in &self.ca_content {
            if ca.0 == certificate_to_check.0 {
                return true;
            }
        }



        println!("Serial: {:?}", cert.raw_serial_as_string()); // Serial number of SSL cert
        println!("Public Key: {:?}", HexSlice(&cert.public_key().raw[..8])); // Serial number of SSL cert

        let issuer = cert.issuer();

        for common_name in issuer.iter_common_name() {
            println!("CommonName: {}", common_name.as_str().unwrap());
        }

        for common_name in issuer.iter_email() {
            println!("Email: {}", common_name.as_str().unwrap());
        }

        for attr in issuer.iter_attributes() {
            println!("Attr: {}={}", attr.attr_type(), attr.as_str().unwrap());
        }

        if let Some(uid) = &cert.issuer_uid {
            println!("UID: {:?}", uid);
        }

        false
         */
    }

    pub fn get_names(&self) -> &[tokio_rustls::rustls::DistinguishedName] {
        &self.names
    }
}

impl From<Vec<u8>> for ClientCertificateCa {
    fn from(value: Vec<u8>) -> Self {
        let mut reader = std::io::BufReader::new(value.as_slice());

        let certs = rustls_pemfile::certs(&mut reader);

        // Load and return certificate.
        let mut result = Vec::new();

        for cert in certs {
            let cert: rustls_pki_types::CertificateDer<'_> = cert.unwrap();
            result.push(cert);
        }

        Self::new(result)
    }
}
