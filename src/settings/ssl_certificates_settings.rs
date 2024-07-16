use std::collections::HashMap;

use serde::*;

use crate::variables_reader::VariablesReader;

use crate::configurations::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SslCertificatesSettingsModel {
    pub id: String,
    pub certificate: String,
    pub private_key: String,
}

impl SslCertificatesSettingsModel {
    pub fn get_certificate(
        &self,
        variables: VariablesReader,
        ssh_config: &Option<HashMap<String, SshConfigSettings>>,
    ) -> Result<FileSource, String> {
        let src = crate::populate_variable::populate_variable(&self.certificate, variables);
        FileSource::from_src(src, ssh_config, variables)
    }

    pub fn get_private_key(
        &self,
        variables: VariablesReader,
        ssh_config: &Option<HashMap<String, SshConfigSettings>>,
    ) -> Result<FileSource, String> {
        let src = crate::populate_variable::populate_variable(&self.private_key, variables);
        FileSource::from_src(src, ssh_config, variables)
    }
}
