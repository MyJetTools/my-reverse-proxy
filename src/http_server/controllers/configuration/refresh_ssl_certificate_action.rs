use std::sync::Arc;

use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

use crate::app::AppContext;

#[http_route(
    method: "POST",
    route: "/api/configuration/RefreshSslCertificate",
    summary: "Refresh SSL certificate",
    description: "Refresh SSL certificate from the source",
    input_data: ReloadEndpointHttpInput,
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
    input_data: ReloadEndpointHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    match crate::flows::refresh_ssl_certificate_from_settings(&action.app, &input_data.cert_id)
        .await
    {
        Ok(_) => HttpOutput::Empty.into_ok_result(true),
        Err(err) => Err(HttpFailResult::as_validation_error(err)),
    }
}

#[derive(MyHttpInput)]
pub struct ReloadEndpointHttpInput {
    #[http_form_data(name = "cert_id", description = "Id of certificate")]
    pub cert_id: String,
}
