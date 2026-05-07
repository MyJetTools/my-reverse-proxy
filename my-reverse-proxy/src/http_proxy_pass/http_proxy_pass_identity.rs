use std::sync::Arc;

use crate::{tcp_listener::https::ClientCertificateData, types::Email};

pub enum HttpProxyPassIdentity {
    ClientCert(Arc<ClientCertificateData>),
    GoogleUser(Email),
}

impl HttpProxyPassIdentity {
    pub fn as_str(&self) -> &str {
        match self {
            HttpProxyPassIdentity::ClientCert(data) => data.cn.as_str(),
            HttpProxyPassIdentity::GoogleUser(email) => email.as_str(),
        }
    }
}
