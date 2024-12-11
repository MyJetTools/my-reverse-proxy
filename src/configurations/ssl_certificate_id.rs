use crate::self_signed_cert::SELF_SIGNED_CERT_NAME;

#[derive(Debug, Clone)]
pub struct SslCertificateId(String);

impl SslCertificateId {
    pub fn new(cert_id: String) -> Self {
        Self(cert_id)
    }

    pub fn new_as_self_signed() -> Self {
        Self(SELF_SIGNED_CERT_NAME.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }

    pub fn as_ref(&self) -> SslCertificateIdRef {
        SslCertificateIdRef::new(&self.0)
    }

    pub fn is_self_signed(&self) -> bool {
        self.0 == SELF_SIGNED_CERT_NAME
    }
}

impl Into<SslCertificateId> for String {
    fn into(self) -> SslCertificateId {
        SslCertificateId::new(self)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SslCertificateIdRef<'s>(&'s str);

impl<'s> SslCertificateIdRef<'s> {
    pub fn new(cert_id: &'s str) -> Self {
        Self(cert_id)
    }

    pub fn new_as_self_signed() -> Self {
        Self(SELF_SIGNED_CERT_NAME)
    }

    pub fn as_str(&'s self) -> &'s str {
        self.0
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }

    pub fn is_self_signed(&self) -> bool {
        self.0 == SELF_SIGNED_CERT_NAME
    }
}

impl Into<SslCertificateId> for SslCertificateIdRef<'_> {
    fn into(self) -> SslCertificateId {
        SslCertificateId::new(self.0.to_string())
    }
}

impl<'s> Into<SslCertificateIdRef<'s>> for &'s SslCertificateId {
    fn into(self) -> SslCertificateIdRef<'s> {
        SslCertificateIdRef::new(self.as_str())
    }
}
