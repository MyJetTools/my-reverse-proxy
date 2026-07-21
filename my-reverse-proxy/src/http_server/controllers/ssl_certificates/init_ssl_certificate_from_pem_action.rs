use base64::Engine;
use my_http_server::{
    macros::{http_route, MyHttpInput, MyHttpObjectStructure},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};
use serde::Serialize;

#[http_route(
    method: "POST",
    route: "/api/SslCertificates/InitFromPem",
    summary: "Init (upload) an ssl certificate passing PEM in a json body",
    description: "Same as /api/SslCertificates/Init but the certificate and the private key travel in a json body as base64 of their PEM text - this is what the admin ui uses when the operator pastes the PEM material into the 'SSL cert not loaded' dialog. Base64 keeps the multi-line PEM intact through json. The certificate is validated exactly the same way (the private key must match the certificate; the certificate must cover the SNI of the endpoint and of the certificate it replaces) and is installed on the next TLS handshake - no reload required. A certificate uploaded here is manually managed and is not auto-renewed until the configured source is refreshed again.",
    controller: "SslCertificates",
    input_data: InitSslCertificateFromPemHttpInput,
    result:[
        {status_code: 200, description: "Certificate validated and installed", model:"InitSslCertificateResultHttpModel"},
        {status_code: 400, description: "Validation error (bad base64, bad PEM, key/cert mismatch, unknown cert id, or SNI mismatch)"},
    ]
)]
pub struct InitSslCertificateFromPemAction;

async fn handle_request(
    _action: &InitSslCertificateFromPemAction,
    input_data: InitSslCertificateFromPemHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let cert_pem = decode_base64_pem(&input_data.cert, "cert")?;
    let private_key_pem = decode_base64_pem(&input_data.private_key, "private_key")?;

    let result =
        crate::scripts::init_ssl_cert_manually(input_data.cert_id.trim(), cert_pem, private_key_pem)
            .await
            .map_err(HttpFailResult::as_validation_error)?;

    let model = InitSslCertificateResultHttpModel {
        cert_id: result.cert_id,
        cn: result.cn,
        expires: result.expires.to_rfc3339(),
        covered_domains: result.covered_domains,
        validated_sni: result.validated_sni,
    };

    HttpOutput::as_json(model).into_ok_result(true).into()
}

/// Decodes a base64 field into the PEM bytes the certificate loader expects. Both the empty
/// field and broken base64 are reported by name, so the caller knows which one to fix instead
/// of getting a generic "bad certificate" from the PEM parser further down.
fn decode_base64_pem(value: &str, field: &str) -> Result<Vec<u8>, HttpFailResult> {
    let value = value.trim();

    if value.is_empty() {
        return Err(HttpFailResult::as_validation_error(format!(
            "'{}' must not be empty",
            field
        )));
    }

    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|err| {
            HttpFailResult::as_validation_error(format!(
                "'{}' is not a valid base64 of the PEM text: {}",
                field, err
            ))
        })
}

#[derive(MyHttpInput)]
pub struct InitSslCertificateFromPemHttpInput {
    #[http_body(
        name = "cert_id",
        description = "Id of the certificate as referenced by the endpoint in configuration"
    )]
    pub cert_id: String,

    #[http_body(
        name = "cert",
        description = "Certificate chain: base64 of the PEM text"
    )]
    pub cert: String,

    #[http_body(
        name = "private_key",
        description = "Private key: base64 of the PEM text"
    )]
    pub private_key: String,
}

// `MyHttpObjectStructure` does not tolerate doc comments on the fields - the field meaning is
// described in the action `description` above instead.
#[derive(Debug, Serialize, MyHttpObjectStructure)]
pub struct InitSslCertificateResultHttpModel {
    pub cert_id: String,
    pub cn: String,
    pub expires: String,
    pub covered_domains: Vec<String>,
    pub validated_sni: Vec<String>,
}
