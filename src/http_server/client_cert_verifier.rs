use std::{fmt::Debug, sync::Arc};

use tokio_rustls::rustls::{server::danger::ClientCertVerifier, SignatureScheme};

use crate::app::AppContext;

use super::ClientCertificateCa;

pub struct MyClientCertVerifier {
    app: Arc<AppContext>,
    pub ca: Arc<ClientCertificateCa>,
    endpoint_port: u16,
    connection_id: u64,
}

impl MyClientCertVerifier {
    pub fn new(
        app: Arc<AppContext>,
        ca: Arc<ClientCertificateCa>,
        endpoint_port: u16,
        connection_id: u64,
    ) -> Self {
        Self {
            ca,
            app,
            endpoint_port,
            connection_id,
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
    fn root_hint_subjects(&self) -> &[tokio_rustls::rustls::DistinguishedName] {
        self.ca.get_names()
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        println!("Verifying signature tls12");
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer<'_>,
        _dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        println!("Verifying signature tls12");
        Ok(tokio_rustls::rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        println!("supported_verify_schemes");
        vec![SignatureScheme::RSA_PSS_SHA256]
    }

    fn verify_client_cert(
        &self,
        end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _now: rustls_pki_types::UnixTime,
    ) -> Result<tokio_rustls::rustls::server::danger::ClientCertVerified, tokio_rustls::rustls::Error>
    {
        println!("Verifying Client Cert");

        if let Some(common_name) = self.ca.check_certificate(end_entity) {
            println!("Accepted certificate with common name: {}", common_name);

            self.app
                .saved_client_certs
                .save(self.endpoint_port, self.connection_id, common_name);

            return Ok(tokio_rustls::rustls::server::danger::ClientCertVerified::assertion());
        }

        Err(tokio_rustls::rustls::Error::General(
            "Client certificate is not valid".to_string(),
        ))
    }
}
