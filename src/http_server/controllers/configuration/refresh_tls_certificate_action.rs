use std::sync::Arc;

use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

use crate::app::AppContext;

#[http_route(
    method: "POST",
    route: "/api/configuration/RefreshSslCertificate",
    summary: "Refresh TLS certificate",
    description: "Refresh TLS certificate from the source",
    input_data: ReloadTlsCertificatesHttpInput,
    controller: "Configuration",
    result:[
        {status_code: 204, description: "Ok response"},
    ]
)]
pub struct RefreshSslCertificateAction {
    app: Arc<AppContext>,
}

impl RefreshSslCertificateAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &RefreshSslCertificateAction,
    input_data: ReloadTlsCertificatesHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    match crate::flows::refresh_tls_certificate_from_settings(&action.app, &input_data.cert_id)
        .await
    {
        Ok(_) => HttpOutput::Empty.into_ok_result(true),
        Err(err) => Err(HttpFailResult::as_validation_error(err)),
    }
}

#[derive(MyHttpInput)]
pub struct ReloadTlsCertificatesHttpInput {
    #[http_form_data(name = "cert_id", description = "Id of certificate")]
    pub cert_id: String,
}