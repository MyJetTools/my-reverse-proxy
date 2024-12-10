use std::sync::Arc;

use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

use crate::app::AppContext;

#[http_route(
    method: "POST",
    route: "/api/configuration/ReloadPort",
    summary: "Reload port configurations",
    description: "Reload all configurations for specified port",
    input_data: ReloadPortHttpInput,
    controller: "Configuration",
    result:[
        {status_code: 200, description: "Ok response"},
    ]
)]
pub struct ReloadPortAction {
    app: Arc<AppContext>,
}

impl ReloadPortAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &ReloadPortAction,
    input_data: ReloadPortHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    match crate::flows::reload_port_configurations(&action.app, input_data.port).await {
        Ok(result) => HttpOutput::as_text(result).into_ok_result(true),
        Err(err) => Err(HttpFailResult::as_validation_error(err)),
    }
}

#[derive(MyHttpInput)]
pub struct ReloadPortHttpInput {
    #[http_form_data(name = "port", description = "Port to reload")]
    pub port: u16,
}
