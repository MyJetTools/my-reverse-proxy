use std::sync::Arc;

use my_http_server::{macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

use super::contracts::*;
use crate::app::AppContext;

#[http_route(
    method: "GET",
    route: "/api/configuration/Current",
    summary: "Get current configuration",
    description: "Get current configuration",
    controller: "Configuration",
    result:[
        {status_code: 200, description: "Ok response", model:"CurrentConfigurationHttpModel"},
    ]
)]
pub struct GetCurrentConfigAction {
    app: Arc<AppContext>,
}

impl GetCurrentConfigAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &GetCurrentConfigAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let result = action
        .app
        .current_configuration
        .get(|itm| CurrentConfigurationHttpModel::new(itm))
        .await;

    HttpOutput::as_json(result).into_ok_result(true).into()
}
