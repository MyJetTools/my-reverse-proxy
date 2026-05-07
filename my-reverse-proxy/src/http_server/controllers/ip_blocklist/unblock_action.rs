use my_http_server::{
    macros::{http_route, MyHttpInput, MyHttpObjectStructure},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};
use serde::Serialize;

#[http_route(
    method: "POST",
    route: "/api/IpBlocklist/Unblock",
    summary: "Remove IP from the blocklist",
    description: "Forcibly removes the given source IP from the blocklist (resets fail counter)",
    input_data: UnblockIpInput,
    controller: "IpBlocklist",
    result:[
        {status_code: 200, description: "Ok response", model: "UnblockIpResponse"},
    ]
)]
pub struct UnblockIpAction;

async fn handle_request(
    _action: &UnblockIpAction,
    input_data: UnblockIpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let ip = input_data
        .ip
        .parse::<std::net::IpAddr>()
        .map_err(|err| HttpFailResult::as_validation_error(format!("Invalid IP: {}", err)))?;

    let removed = crate::app::APP_CTX.ip_blocklist.unblock(&ip);

    HttpOutput::as_json(UnblockIpResponse { ip: ip.to_string(), removed })
        .into_ok_result(true)
        .into()
}

#[derive(MyHttpInput)]
pub struct UnblockIpInput {
    #[http_form_data(name = "ip", description = "Source IP address to unblock")]
    pub ip: String,
}

#[derive(Serialize, MyHttpObjectStructure)]
pub struct UnblockIpResponse {
    pub ip: String,
    pub removed: bool,
}
