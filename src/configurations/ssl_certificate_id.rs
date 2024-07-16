#[derive(Debug, Clone)]
pub struct SslCertificateId(String);

impl SslCertificateId {
    pub fn new(location: String) -> Self {
        Self(location)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}
