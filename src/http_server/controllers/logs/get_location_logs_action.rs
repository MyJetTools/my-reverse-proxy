use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

use super::contracts::*;

#[http_route(
    method: "GET",
    route: "/api/logs/location",
    summary: "Get in-memory logs for a location",
    description: "Returns the last 100 log messages attributed to the given location id (ProxyPassLocationConfig.id)",
    input_data: GetLocationLogsInput,
    controller: "Logs",
    result:[
        {status_code: 200, description: "Ok response", model: "ProxyLogsHttpModel"},
    ]
)]
pub struct GetLocationLogsAction;

async fn handle_request(
    _action: &GetLocationLogsAction,
    input_data: GetLocationLogsInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let entries = crate::app::APP_CTX
        .proxy_logs
        .get_by_location(input_data.id);
    HttpOutput::as_json(ProxyLogsHttpModel::from_entries(entries))
        .into_ok_result(true)
        .into()
}

#[derive(MyHttpInput)]
pub struct GetLocationLogsInput {
    #[http_query(name = "id", description = "Location id")]
    pub id: i64,
}
