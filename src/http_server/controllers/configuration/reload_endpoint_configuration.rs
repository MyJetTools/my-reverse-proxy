use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

#[http_route(
    method: "POST",
    route: "/api/configuration/ReloadEndpoint",
    summary: "Reload endpoint",
    description: "Reload endpoint. Example: mydomain.com:443",
    input_data: ReloadEndpointHttpInput,
    controller: "Configuration",
    result:[
        {status_code: 204, description: "Ok response"},
    ]
)]
pub struct ReloadEndpointAction;
async fn handle_request(
    action: &ReloadEndpointAction,
    input_data: ReloadEndpointHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    match crate::flows::reload_endpoint_configuration(&input_data.endpoint_id).await {
        Ok(result) => HttpOutput::as_text(result).into_ok_result(true),
        Err(err) => Err(HttpFailResult::as_validation_error(err)),
    }
}

#[derive(MyHttpInput)]
pub struct ReloadEndpointHttpInput {
    #[http_form_data(name = "endpoint_id", description = "Endpoint id to reload")]
    pub endpoint_id: String,
}
