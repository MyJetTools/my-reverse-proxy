use std::{fmt::Debug, sync::Arc};

use my_tls::tokio_rustls::rustls;

use rustls::{server::danger::ClientCertVerifier, SignatureScheme};

use crate::configurations::SslCertificateId;

use super::{client_cert_cell::ClientCertCell, ClientCertificateCa};

pub struct MyClientCertVerifier {
    client_cert_cell: Arc<ClientCertCell>,
    pub ca: Arc<ClientCertificateCa>,
    endpoint_port: u16,
    ca_id: SslCertificateId,
    debug: bool,
}

impl MyClientCertVerifier {
    pub fn new(
        ca_id: SslCertificateId,
        client_cert_cell: Arc<ClientCertCell>,
        ca: Arc<ClientCertificateCa>,
        endpoint_port: u16,
        debug: bool,
    ) -> Self {
        Self {
            ca_id,
            ca,
            client_cert_cell,
            endpoint_port,
            debug,
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
        if self.debug {
            println!("Verifying signature tls12");
        }

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
        if self.debug {
            println!("Verifying signature tls12");
        }

        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        if self.debug {
            println!("supported_verify_schemes");
        }

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
        if self.debug {
            println!("Verifying Client Cert");
        }

        if let Some(client_certificate) = self.ca.check_certificate(&self.ca_id, end_entity) {
            if self.debug {
                println!("Accepted certificate: {:?}", client_certificate);
            }

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
