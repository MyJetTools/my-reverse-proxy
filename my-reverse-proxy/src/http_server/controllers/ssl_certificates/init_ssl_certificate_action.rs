use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

#[http_route(
    method: "POST",
    route: "/api/SslCertificates/Init",
    summary: "Init (upload) an ssl certificate at runtime",
    description: "Initializes/replaces an SSL certificate in the running proxy. Upload the certificate chain and the matching private key as PEM files (multipart/form-data). Once received the certificate is validated (the private key must match the certificate; the SNI must coincide with the endpoint and the certificate it replaces) and installed on the next TLS handshake — no reload required. A certificate uploaded here is manually managed and is not auto-renewed until the configured source is refreshed again.",
    controller: "SslCertificates",
    input_data: InitSslCertificateHttpInput,
    result:[
        {status_code: 204, description: "Certificate validated and installed"},
        {status_code: 400, description: "Validation error (bad PEM, key/cert mismatch, unknown cert id, or SNI mismatch)"},
    ]
)]
pub struct InitSslCertificateAction;

async fn handle_request(
    _action: &InitSslCertificateAction,
    input_data: InitSslCertificateHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    crate::scripts::init_ssl_cert_manually(
        input_data.cert_id.trim(),
        input_data.certificate,
        input_data.private_key,
    )
    .await
    .map_err(HttpFailResult::as_validation_error)?;

    HttpOutput::Empty.into_ok_result(true).into()
}

#[derive(MyHttpInput)]
pub struct InitSslCertificateHttpInput {
    #[http_form_data(
        name = "cert_id",
        description = "Id of the certificate as referenced by the endpoint in configuration"
    )]
    pub cert_id: String,

    #[http_form_data(name = "certificate", description = "Certificate chain file in PEM format")]
    pub certificate: Vec<u8>,

    #[http_form_data(name = "private_key", description = "Private key file in PEM format")]
    pub private_key: Vec<u8>,
}
