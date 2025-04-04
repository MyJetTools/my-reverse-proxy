use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientCertificateCaSettings {
    pub id: String,
    pub ca: String,
    pub revocation_list: Option<String>,
}
