use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModifyHttpHeadersSettings {
    pub add: Option<AddHttpHeadersSettings>,
    pub remove: Option<RemoveHttpHeadersSettings>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddHeaderSettingsModel {
    pub name: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddHttpHeadersSettings {
    pub request: Option<Vec<AddHeaderSettingsModel>>,
    pub response: Option<Vec<AddHeaderSettingsModel>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemoveHttpHeadersSettings {
    pub request: Option<Vec<String>>,
    pub response: Option<Vec<String>>,
}
