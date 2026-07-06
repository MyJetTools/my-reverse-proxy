use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

#[http_route(
    method: "POST",
    route: "/api/SslCertificates/Init",
    summary: "Init (upload) an ssl certificate at runtime",
    description: "Initializes/replaces an SSL certificate in the running proxy. Intended for a certificate declared in configuration without a source (nothing to resolve on TCP accept): paste the certificate chain and private key in PEM format and they are stored in the cache and served on the next TLS handshake — no reload required. The certificate is validated before it is stored: the private key must parse, at least one endpoint must reference the given cert id, and the certificate must cover that endpoint's domain(s). A certificate uploaded here is manually managed and is not auto-renewed until the configured source is refreshed again.",
    controller: "SslCertificates",
    input_data: InitSslCertificateHttpInput,
    result:[
        {status_code: 204, description: "Certificate stored"},
        {status_code: 400, description: "Validation error (bad PEM, unknown cert id, or domain mismatch)"},
    ]
)]
pub struct InitSslCertificateAction;

async fn handle_request(
    _action: &InitSslCertificateAction,
    input_data: InitSslCertificateHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    crate::scripts::init_ssl_cert_manually(
        &input_data.cert_id,
        input_data.certificate.into_bytes(),
        input_data.private_key.into_bytes(),
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

    #[http_form_data(
        name = "certificate",
        description = "Certificate chain in PEM format (-----BEGIN CERTIFICATE----- ...)"
    )]
    pub certificate: String,

    #[http_form_data(
        name = "private_key",
        description = "Private key in PEM format (-----BEGIN PRIVATE KEY----- ...)"
    )]
    pub private_key: String,
}
