use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

use super::contracts::*;

#[http_route(
    method: "GET",
    route: "/api/logs/endpoint",
    summary: "Get in-memory logs for an endpoint",
    description: "Returns the last 100 log messages attributed to the given endpoint (host_endpoint string, e.g. myapp.com:443)",
    input_data: GetEndpointLogsInput,
    controller: "Logs",
    result:[
        {status_code: 200, description: "Ok response", model: "ProxyLogsHttpModel"},
    ]
)]
pub struct GetEndpointLogsAction;

async fn handle_request(
    _action: &GetEndpointLogsAction,
    input_data: GetEndpointLogsInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let entries = crate::app::APP_CTX
        .proxy_logs
        .get_by_endpoint(&input_data.id);
    HttpOutput::as_json(ProxyLogsHttpModel::from_entries(entries))
        .into_ok_result(true)
        .into()
}

#[derive(MyHttpInput)]
pub struct GetEndpointLogsInput {
    #[http_query(name = "id", description = "Endpoint host string, e.g. myapp.com:443")]
    pub id: String,
}
