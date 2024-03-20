use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SslCertificatesSettingsModel {
    pub id: String,
    pub certificate: String,
    pub private_key: String,
}
