use std::collections::HashMap;

use serde::*;

use super::{ModifyHttpHeadersSettings, ProxyPassTo};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LocationSettings {
    pub path: Option<String>,
    pub proxy_pass_to: String,
    #[serde(rename = "type")]
    pub location_type: Option<String>,
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
    pub default_file: Option<String>,
}

impl LocationSettings {
    pub fn get_proxy_pass_to<'s>(
        &'s self,
        variables: &Option<HashMap<String, String>>,
    ) -> (ProxyPassTo, Option<String>) {
        let result =
            crate::populate_variable::populate_variable(self.proxy_pass_to.trim(), variables);

        (
            ProxyPassTo::new(result.to_string()),
            self.default_file.clone(),
        )
    }

    pub fn is_http1(&self) -> bool {
        if self.location_type.is_none() {
            panic!("Unknown remote location type. Missing location.type in yaml")
        }
        let location_type = self.location_type.as_ref().unwrap();
        match location_type.as_str() {
            "http" => true,
            "http2" => false,
            _ => panic!("Unknown remote location type: {}", location_type),
        }
    }
}
