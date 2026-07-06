use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult,
};

#[http_route(
    method: "POST",
    route: "/api/SslCertificates/UploadCertificate",
    summary: "Upload the certificate PEM (step 1 of 2)",
    description: "Uploads the certificate chain for the certificate id given in the `certId` query string. The ENTIRE request body is the raw certificate PEM (paste it directly — no JSON, no form fields). Pair this with UploadPrivateKey; once both parts have been received the certificate is validated (key↔cert match, SNI must coincide with the endpoint / the certificate it replaces) and installed live with no reload. Order does not matter.",
    controller: "SslCertificates",
    input_data: UploadCertificateHttpInput,
    result:[
        {status_code: 200, description: "Certificate part received; waiting for the private key"},
        {status_code: 204, description: "Both parts received; certificate validated and installed"},
        {status_code: 400, description: "Validation error (bad PEM, key mismatch, unknown cert id, or SNI mismatch)"},
    ]
)]
pub struct UploadCertificateAction;

async fn handle_request(
    _action: &UploadCertificateAction,
    input_data: UploadCertificateHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let cert_id = input_data.cert_id.trim();

    let state = crate::app::APP_CTX
        .pending_ssl_uploads
        .set_certificate(cert_id, input_data.body);

    super::finish_pending_upload(cert_id, state).await
}

#[derive(MyHttpInput)]
pub struct UploadCertificateHttpInput {
    #[http_query(name = "certId", description = "Id of the certificate as referenced by the endpoint")]
    pub cert_id: String,

    #[http_body_raw(description = "The certificate chain in PEM format — the whole request body")]
    pub body: Vec<u8>,
}
