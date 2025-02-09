use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GatewayClientSettings {
    pub remote_host: String,
    pub password: Option<String>,
}
