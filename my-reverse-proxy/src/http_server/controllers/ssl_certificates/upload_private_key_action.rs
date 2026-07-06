use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult,
};

#[http_route(
    method: "POST",
    route: "/api/SslCertificates/UploadPrivateKey",
    summary: "Upload the private key PEM (step 2 of 2)",
    description: "Uploads the private key for the certificate id given in the `certId` query string. The ENTIRE request body is the raw private key PEM (paste it directly — no JSON, no form fields). Pair this with UploadCertificate; once both parts have been received the certificate is validated (key↔cert match, SNI must coincide with the endpoint / the certificate it replaces) and installed live with no reload. Order does not matter.",
    controller: "SslCertificates",
    input_data: UploadPrivateKeyHttpInput,
    result:[
        {status_code: 200, description: "Private key part received; waiting for the certificate"},
        {status_code: 204, description: "Both parts received; certificate validated and installed"},
        {status_code: 400, description: "Validation error (bad PEM, key mismatch, unknown cert id, or SNI mismatch)"},
    ]
)]
pub struct UploadPrivateKeyAction;

async fn handle_request(
    _action: &UploadPrivateKeyAction,
    input_data: UploadPrivateKeyHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let cert_id = input_data.cert_id.trim();

    let state = crate::app::APP_CTX
        .pending_ssl_uploads
        .set_private_key(cert_id, input_data.body);

    super::finish_pending_upload(cert_id, state).await
}

#[derive(MyHttpInput)]
pub struct UploadPrivateKeyHttpInput {
    #[http_query(name = "certId", description = "Id of the certificate as referenced by the endpoint")]
    pub cert_id: String,

    #[http_body_raw(description = "The private key in PEM format — the whole request body")]
    pub body: Vec<u8>,
}
