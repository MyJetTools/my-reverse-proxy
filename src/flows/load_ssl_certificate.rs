use crate::configurations::*;
use crate::{files_cache::FilesCache, settings::SettingsModel, ssl::SslCertificate};

pub struct SslCertificateResult {
    pub cert_src: FileSource,
    pub private_key_src: FileSource,
    pub cert: SslCertificate,
}

pub async fn load_ssl_certificate(
    settings_model: &SettingsModel,
    ssl_id: &SslCertificateId,
    listen_port: u16,
    files_cache: &FilesCache,
    init_on_start: bool,
) -> Result<SslCertificateResult, String> {
    let cert_result = settings_model.get_ssl_certificate(ssl_id)?;

    if cert_result.is_none() {
        return Err(format!(
            "SSL certificate {} not found for https port {}",
            ssl_id.as_str(),
            listen_port
        ));
    }

    let (cert_src, private_key_src) = cert_result.unwrap();

    let certificates = cert_src
        .load_file_content(Some(files_cache), init_on_start)
        .await?;
    let private_key = private_key_src
        .load_file_content(Some(files_cache), init_on_start)
        .await?;

    let result = SslCertificateResult {
        cert_src,
        private_key_src,
        cert: SslCertificate::new(private_key, certificates)?,
    };

    Ok(result)
}
