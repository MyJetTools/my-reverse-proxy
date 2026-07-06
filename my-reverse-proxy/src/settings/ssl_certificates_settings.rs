use serde::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SslCertificatesSettingsModel {
    pub id: String,
    /// Source of the certificate (local file or over-ssh). When omitted together
    /// with `private_key`, the certificate is "manual" — it is not resolved from a
    /// source on startup and must be pushed at runtime via POST /api/SslCertificates/Init.
    pub certificate: Option<String>,
    pub private_key: Option<String>,
}

impl SslCertificatesSettingsModel {
    /// A "manual" certificate has no source configured — neither the certificate
    /// nor the private key location is specified. It is pushed at runtime via Swagger.
    pub fn get_sources(&self) -> Result<Option<(&str, &str)>, String> {
        let cert = self.certificate.as_deref().filter(|s| !s.trim().is_empty());
        let private_key = self.private_key.as_deref().filter(|s| !s.trim().is_empty());

        match (cert, private_key) {
            (None, None) => Ok(None),
            (Some(cert), Some(private_key)) => Ok(Some((cert, private_key))),
            _ => Err(format!(
                "SSL certificate '{}' must specify both 'certificate' and 'private_key', or neither (manual init via Swagger).",
                self.id
            )),
        }
    }
}

/*
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
 */
