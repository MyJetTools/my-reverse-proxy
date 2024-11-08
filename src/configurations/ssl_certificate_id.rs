#[derive(Debug, Clone)]
pub struct SslCertificateId(String);

impl SslCertificateId {
    pub fn new(cert_id: String) -> Self {
        Self(cert_id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Into<SslCertificateId> for String {
    fn into(self) -> SslCertificateId {
        SslCertificateId::new(self)
    }
}
