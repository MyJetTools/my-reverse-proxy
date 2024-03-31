use crate::types::Email;

pub struct HttpProxyPassIdentity {
    pub client_cert_cn: Option<String>,
    pub ga_user: Option<Email>,
}

impl HttpProxyPassIdentity {
    pub fn new(client_cert_cn: Option<String>) -> Self {
        Self {
            client_cert_cn,
            ga_user: None,
        }
    }

    pub fn get_identity(&self) -> Option<&str> {
        if let Some(result) = self.client_cert_cn.as_ref() {
            return Some(result);
        }

        if let Some(result) = self.ga_user.as_ref() {
            return Some(result.as_str());
        }

        None
    }
}
