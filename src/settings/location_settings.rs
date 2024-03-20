use std::collections::HashMap;

use serde::*;

use super::ProxyPassTo;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LocationSettings {
    pub path: Option<String>,
    pub proxy_pass_to: String,
    #[serde(rename = "type")]
    pub location_type: Option<String>,
    pub add_request_headers: Option<HashMap<String, String>>,
}

impl LocationSettings {
    pub fn get_proxy_pass<'s>(
        &'s self,
        variables: &Option<HashMap<String, String>>,
    ) -> ProxyPassTo {
        let result = super::populate_variable(self.proxy_pass_to.trim(), variables);

        ProxyPassTo::new(result.to_string())
    }

    pub fn is_http1(&self) -> bool {
        if self.location_type.is_none() {
            panic!("Unknown remote location type")
        }
        let location_type = self.location_type.as_ref().unwrap();
        match location_type.as_str() {
            "http" => true,
            "http2" => false,
            _ => panic!("Unknown remote location type: {}", location_type),
        }
    }
}
