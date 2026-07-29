use base64::Engine;

use crate::models::{InitSslCertificateRequestModel, InitSslCertificateResultModel};

use super::build_url;

const INIT_FROM_PEM_PATH: &str = "/api/SslCertificates/InitFromPem";

/// Uploads PEM material for `cert_id`. The pasted PEM text is sent base64-encoded so its line
/// breaks survive the json body untouched. The proxy validates the key/certificate pair and the
/// SNI before installing it, so a rejection here is a real validation failure and its text is
/// meant to be shown to the operator as-is.
pub async fn init_ssl_certificate(
    cert_id: &str,
    certificate: &str,
    private_key: &str,
) -> Result<InitSslCertificateResultModel, String> {
    let url = build_url(INIT_FROM_PEM_PATH)?;

    let request = InitSslCertificateRequestModel {
        cert_id: cert_id.to_string(),
        cert: to_base64(certificate),
        private_key: to_base64(private_key),
    };

    let resp = reqwest::Client::new()
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("POST {url} failed: {e}"))?;

    let status = resp.status();

    if !status.is_success() {
        // Validation failures come back as `text/plain` with a `Validation error: ` prefix —
        // strip it, the dialog already tells the operator that the upload was rejected.
        let body = resp.text().await.unwrap_or_default();

        let message = body
            .trim()
            .trim_start_matches("Validation error:")
            .trim()
            .to_string();

        return Err(if message.is_empty() {
            format!("POST {url} returned {status}")
        } else {
            message
        });
    }

    resp.json::<InitSslCertificateResultModel>()
        .await
        .map_err(|e| format!("decoding {url} response failed: {e}"))
}

fn to_base64(pem: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(pem.as_bytes())
}
