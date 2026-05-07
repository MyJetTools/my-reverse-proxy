use serde::*;

use super::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HostSettings {
    pub endpoint: EndpointSettings,
    pub locations: Vec<LocationSettings>,
}
