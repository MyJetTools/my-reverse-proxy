use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GatewayServerSettings {
    pub port: u16,
    pub password: Option<String>,
}
