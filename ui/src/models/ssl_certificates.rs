use serde::{Deserialize, Serialize};

/// Body of `POST /api/SslCertificates/InitFromPem`. The names must match the
/// `#[http_body(name = ...)]` names declared by `InitSslCertificateFromPemHttpInput`
/// in `my-reverse-proxy`. `cert` and `private_key` carry base64 of the PEM text —
/// json would otherwise have to escape every line break of the pasted material.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InitSslCertificateRequestModel {
    pub cert_id: String,
    pub cert: String,
    pub private_key: String,
}

/// Mirror of `InitSslCertificateResultHttpModel` — what the proxy reports about the
/// certificate it has just installed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InitSslCertificateResultModel {
    pub cert_id: String,
    pub cn: String,
    pub expires: String,
    pub covered_domains: Vec<String>,
    pub validated_sni: Vec<String>,
}
