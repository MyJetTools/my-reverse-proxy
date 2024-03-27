use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GoogleAuthSettings {
    pub client_id: String,
    pub client_secret: String,
}
