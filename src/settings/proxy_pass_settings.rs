use serde::*;

use super::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProxyPassSettings {
    pub endpoint: EndpointSettings,
    pub locations: Vec<LocationSettings>,
}
