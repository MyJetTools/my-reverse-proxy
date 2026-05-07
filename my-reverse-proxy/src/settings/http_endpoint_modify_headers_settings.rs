use super::ModifyHttpHeadersSettings;

#[derive(Default)]
pub struct HttpEndpointModifyHeadersSettings {
    pub global_modify_headers_settings: Option<ModifyHttpHeadersSettings>,
    pub endpoint_modify_headers_settings: Option<ModifyHttpHeadersSettings>,
}
