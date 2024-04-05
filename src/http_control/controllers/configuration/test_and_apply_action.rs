use std::sync::Arc;

use my_http_server::{macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

use crate::app::AppContext;

#[http_route(
    method: "GET",
    route: "/api/configuration/TestAndApply",
    summary: "Test and Apply Configuration",
    description: "Test and Apply Configuration",
    controller: "Configuration",
    result:[
        {status_code: 200, description: "Ok response", model:"String"},
        {status_code: 400, description: "Invalid configuration", model:"String"},
    ]
)]
pub struct TestAndApplyAction {
    app: Arc<AppContext>,
}

impl TestAndApplyAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &TestAndApplyAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    match crate::flows::get_and_check_app_config(&action.app).await {
        Ok(app_configuration) => {
            action
                .app
                .set_current_app_configuration(app_configuration)
                .await;
            return HttpOutput::as_text("Configuration is ok".to_string())
                .into_ok_result(true)
                .into();
        }
        Err(err) => return HttpOutput::as_text(err).into_fail_result(400, false),
    };
}
