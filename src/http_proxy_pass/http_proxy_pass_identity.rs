use crate::{http_server::ClientCertificateData, types::Email};

pub struct HttpProxyPassIdentity {
    pub client_cert_cn: Option<ClientCertificateData>,
    pub ga_user: Option<Email>,
}

impl HttpProxyPassIdentity {
    pub fn new(client_cert_cn: Option<ClientCertificateData>) -> Self {
        Self {
            client_cert_cn,
            ga_user: None,
        }
    }

    pub fn get_identity(&self) -> Option<&str> {
        if let Some(result) = self.client_cert_cn.as_ref() {
            return Some(result.cn.as_str());
        }

        if let Some(result) = self.ga_user.as_ref() {
            return Some(result.as_str());
        }

        None
    }
}
