use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

use super::contracts::*;

#[http_route(
    method: "GET",
    route: "/api/logs/port",
    summary: "Get in-memory logs for a listening port",
    description: "Returns the last 100 pre-endpoint log messages (e.g. rejected connections that could not be resolved to an endpoint) for the given listening port or unix socket",
    input_data: GetPortLogsInput,
    controller: "Logs",
    result:[
        {status_code: 200, description: "Ok response", model: "ProxyLogsHttpModel"},
    ]
)]
pub struct GetPortLogsAction;

async fn handle_request(
    _action: &GetPortLogsAction,
    input_data: GetPortLogsInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let entries = crate::app::APP_CTX.proxy_logs.get_by_port(&input_data.id);
    HttpOutput::as_json(ProxyLogsHttpModel::from_entries(entries))
        .into_ok_result(true)
        .into()
}

#[derive(MyHttpInput)]
pub struct GetPortLogsInput {
    #[http_query(name = "id", description = "Listening port number or unix socket path")]
    pub id: String,
}
