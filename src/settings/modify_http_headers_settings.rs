use std::collections::HashMap;

use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModifyHttpHeadersSettings {
    pub add: Option<AddHttpHeadersSettings>,
    pub remove: Option<RemoveHttpHeadersSettings>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddHttpHeadersSettings {
    pub request: Option<HashMap<String, String>>,
    pub response: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoveHttpHeadersSettings {
    pub request: Option<Vec<String>>,
    pub response: Option<Vec<String>>,
}
