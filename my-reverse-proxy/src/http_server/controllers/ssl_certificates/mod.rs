mod get_current_certificates_action;
pub use get_current_certificates_action::*;
mod get_ssl_certificate_action;
pub use get_ssl_certificate_action::*;
mod upload_certificate_action;
pub use upload_certificate_action::*;
mod upload_private_key_action;
pub use upload_private_key_action::*;

use my_http_server::{HttpFailResult, HttpOkResult, HttpOutput};

use crate::app::PendingUploadState;

/// Shared tail of the two upload actions: once both parts are staged, validate + install the
/// certificate; otherwise tell the caller which part is still missing.
pub(crate) async fn finish_pending_upload(
    cert_id: &str,
    state: PendingUploadState,
) -> Result<HttpOkResult, HttpFailResult> {
    match state {
        PendingUploadState::Complete {
            cert_pem,
            private_key_pem,
        } => {
            crate::scripts::init_ssl_cert_manually(cert_id, cert_pem, private_key_pem)
                .await
                .map_err(HttpFailResult::as_validation_error)?;

            HttpOutput::Empty.into_ok_result(true).into()
        }
        PendingUploadState::WaitingForPrivateKey => HttpOutput::as_text(
            "Certificate received. Now upload the matching private key via POST /api/SslCertificates/UploadPrivateKey?certId=... with the raw PEM as the request body.".to_string(),
        )
        .into_ok_result(true)
        .into(),
        PendingUploadState::WaitingForCertificate => HttpOutput::as_text(
            "Private key received. Now upload the certificate via POST /api/SslCertificates/UploadCertificate?certId=... with the raw PEM as the request body.".to_string(),
        )
        .into_ok_result(true)
        .into(),
    }
}
