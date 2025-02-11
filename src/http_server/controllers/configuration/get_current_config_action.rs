use my_http_server::{macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

use super::contracts::*;

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
pub struct GetCurrentConfigAction;

async fn handle_request(
    _action: &GetCurrentConfigAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let result = CurrentConfigurationHttpModel::new().await;
    HttpOutput::as_json(result).into_ok_result(true).into()
}
