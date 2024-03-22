use std::collections::HashMap;

use serde::*;

use super::FileSource;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SslCertificatesSettingsModel {
    pub id: String,
    pub certificate: String,
    pub private_key: String,
}

impl SslCertificatesSettingsModel {
    pub fn get_certificate(&self, variables: &Option<HashMap<String, String>>) -> FileSource {
        let src = crate::populate_variable::populate_variable(&self.certificate, variables);
        FileSource::from_src(src)
    }

    pub fn get_private_key(&self, variables: &Option<HashMap<String, String>>) -> FileSource {
        let src = crate::populate_variable::populate_variable(&self.private_key, variables);
        FileSource::from_src(src)
    }
}
