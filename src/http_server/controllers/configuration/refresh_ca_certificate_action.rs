use std::sync::Arc;

use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

use crate::app::AppContext;

#[http_route(
    method: "POST",
    route: "/api/configuration/RefreshCaCertificate",
    summary: "Refresh Ca Certificates configuration",
    description: "Refresh Ca Certificates configuration",
    input_data: ReloadCaCertificatesHttpInput,
    controller: "Configuration",
    result:[
        {status_code: 204, description: "Ok response"},
    ]
)]
pub struct RefreshCaAction {
    app: Arc<AppContext>,
}

impl RefreshCaAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &RefreshCaAction,
    input_data: ReloadCaCertificatesHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    match crate::flows::refresh_ca_from_settings(&action.app, &input_data.ca_id).await {
        Ok(_) => HttpOutput::Empty.into_ok_result(true),
        Err(err) => Err(HttpFailResult::as_validation_error(err)),
    }
}

#[derive(MyHttpInput)]
pub struct ReloadCaCertificatesHttpInput {
    #[http_form_data(name = "ca_id", description = "Id of ca")]
    pub ca_id: String,
}
