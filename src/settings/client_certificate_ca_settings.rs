use std::collections::HashMap;

use serde::*;

use super::FileSource;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientCertificateCaSettings {
    pub id: String,
    pub ca: String,
}

impl ClientCertificateCaSettings {
    pub fn get_ca<'s>(&self, variables: &Option<HashMap<String, String>>) -> FileSource {
        let src = crate::populate_variable::populate_variable(&self.ca, variables);
        FileSource::from_src(src)
    }
}
