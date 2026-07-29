use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

#[http_route(
    method: "POST",
    route: "/api/logs/location/debug",
    summary: "Enable or disable debug logging for a location",
    description: "When enabled, the location's request-building / payload diagnostics are captured into the in-memory logs",
    input_data: SetLocationDebugInput,
    controller: "Logs",
    result:[
        {status_code: 204, description: "Ok"},
    ]
)]
pub struct SetLocationDebugAction;

async fn handle_request(
    _action: &SetLocationDebugAction,
    input_data: SetLocationDebugInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    crate::app::APP_CTX
        .debug_flags
        .set_location(input_data.id, input_data.enabled);
    HttpOutput::Empty.into_ok_result(true).into()
}

#[derive(MyHttpInput)]
pub struct SetLocationDebugInput {
    #[http_query(name = "id", description = "Location id")]
    pub id: i64,
    #[http_query(
        name = "enabled",
        description = "Enable (true) or disable (false) debug"
    )]
    pub enabled: bool,
}
