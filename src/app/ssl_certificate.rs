use std::sync::Arc;

use rustls_pki_types::{CertificateDer, PrivateKeyDer};

#[derive(Clone)]
pub struct SslCertificate {
    pub certificates: Vec<CertificateDer<'static>>,
    pub private_key: Arc<PrivateKeyDer<'static>>,
}
