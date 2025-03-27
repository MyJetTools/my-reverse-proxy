use std::{fmt::Debug, sync::Arc};

use my_tls::tokio_rustls::rustls;

use rustls::{server::danger::ClientCertVerifier, SignatureScheme};

use super::{client_cert_cell::ClientCertCell, ClientCertificateCa};

pub struct MyClientCertVerifier {
    client_cert_cell: Arc<ClientCertCell>,
    pub ca: Arc<ClientCertificateCa>,
    endpoint_port: u16,
}

impl MyClientCertVerifier {
    pub fn new(
        client_cert_cell: Arc<ClientCertCell>,
        ca: Arc<ClientCertificateCa>,
        endpoint_port: u16,
    ) -> Self {
        Self {
            ca,
            client_cert_cell,
            endpoint_port,
        }
    }
}

impl Debug for MyClientCertVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MyClientCertVerifier")
            .field("server_id", &self.endpoint_port)
            .finish()
    }
}

impl ClientCertVerifier for MyClientCertVerifier {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        self.ca.get_names()
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<
        my_tls::tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        my_tls::tokio_rustls::rustls::Error,
    > {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![SignatureScheme::RSA_PSS_SHA256]
    }

    fn verify_client_cert(
        &self,
        end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _now: rustls_pki_types::UnixTime,
    ) -> Result<
        my_tls::tokio_rustls::rustls::server::danger::ClientCertVerified,
        my_tls::tokio_rustls::rustls::Error,
    > {
        if let Some(client_certificate) = self.ca.verify_cert(end_entity) {
            println!("Client Cert CN: {:?}", client_certificate.cn.as_str());

            self.client_cert_cell.set(client_certificate);

            return Ok(
                my_tls::tokio_rustls::rustls::server::danger::ClientCertVerified::assertion(),
            );
        }

        Err(my_tls::tokio_rustls::rustls::Error::General(
            "Client certificate is not valid".to_string(),
        ))
    }
}
