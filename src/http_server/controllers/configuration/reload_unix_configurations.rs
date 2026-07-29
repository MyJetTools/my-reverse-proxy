use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

#[http_route(
    method: "POST",
    route: "/api/configuration/ReloadUnixConfig",
    summary: "Reload unix configurations",
    description: "Reload all configurations for specified unix host",
    input_data: ReloadUnixHostHttpInput,
    controller: "Configuration",
    result:[
        {status_code: 200, description: "Ok response"},
    ]
)]
pub struct ReloadUnixHostAction;

async fn handle_request(
    _action: &ReloadUnixHostAction,
    input_data: ReloadUnixHostHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    match crate::flows::reload_unix_configurations(&input_data.unix_host).await {
        Ok(result) => HttpOutput::as_text(result).into_ok_result(true),
        Err(err) => Err(HttpFailResult::as_validation_error(err)),
    }
}

#[derive(MyHttpInput)]
pub struct ReloadUnixHostHttpInput {
    #[http_form_data(name = "unixHost", description = "Unix socket host to reload")]
    pub unix_host: String,
}
