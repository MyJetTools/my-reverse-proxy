use my_http_server::{
    macros::{http_route, MyHttpInput, MyHttpObjectStructure},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};
use serde::Serialize;

#[http_route(
    method: "GET",
    route: "/api/SslCertificates/Certificate",
    summary: "Get metadata of a loaded ssl certificate",
    description: "Returns metadata (CN, expiry, origin, and the domains it is valid for) of the currently loaded certificate for the given id. The raw certificate PEM material and the private key are never returned.",
    controller: "SslCertificates",
    input_data: GetSslCertificateHttpInput,
    result:[
        {status_code: 200, description: "Ok response", model:"SslCertificateHttpModel"},
        {status_code: 404, description: "No certificate loaded under this id"},
    ]
)]
pub struct GetSslCertificateAction;

async fn handle_request(
    _action: &GetSslCertificateAction,
    input_data: GetSslCertificateHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let details = crate::scripts::get_ssl_certificate_details(&input_data.cert_id)
        .await
        .ok_or_else(|| {
            HttpFailResult::as_not_found(
                format!("No ssl certificate loaded under id '{}'", input_data.cert_id.trim()),
                false,
            )
        })?;

    let model = SslCertificateHttpModel {
        id: details.id,
        cn: details.cn,
        expires: details.expires.to_rfc3339(),
        origin: details.origin,
        domains: details.domains,
    };

    HttpOutput::as_json(model).into_ok_result(true).into()
}

#[derive(MyHttpInput)]
pub struct GetSslCertificateHttpInput {
    #[http_query(name = "certId", description = "Id of the certificate")]
    pub cert_id: String,
}

#[derive(Debug, Serialize, MyHttpObjectStructure)]
pub struct SslCertificateHttpModel {
    pub id: String,
    pub cn: String,
    pub expires: String,
    pub origin: String,
    pub domains: Vec<String>,
}
