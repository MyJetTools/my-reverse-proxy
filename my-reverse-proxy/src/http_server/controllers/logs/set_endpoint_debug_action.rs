use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

#[http_route(
    method: "POST",
    route: "/api/logs/endpoint/debug",
    summary: "Enable or disable debug logging for an endpoint",
    description: "When enabled, the endpoint's request/response diagnostics are captured into the in-memory logs",
    input_data: SetEndpointDebugInput,
    controller: "Logs",
    result:[
        {status_code: 204, description: "Ok"},
    ]
)]
pub struct SetEndpointDebugAction;

async fn handle_request(
    _action: &SetEndpointDebugAction,
    input_data: SetEndpointDebugInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    crate::app::APP_CTX
        .debug_flags
        .set_endpoint(&input_data.id, input_data.enabled);
    HttpOutput::Empty.into_ok_result(true).into()
}

#[derive(MyHttpInput)]
pub struct SetEndpointDebugInput {
    #[http_query(name = "id", description = "Endpoint host string, e.g. myapp.com:443")]
    pub id: String,
    #[http_query(
        name = "enabled",
        description = "Enable (true) or disable (false) debug"
    )]
    pub enabled: bool,
}
