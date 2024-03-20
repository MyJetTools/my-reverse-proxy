#[derive(Debug)]
pub struct SslCertificateId(String);

impl SslCertificateId {
    pub fn new(location: String) -> Self {
        Self(location)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
