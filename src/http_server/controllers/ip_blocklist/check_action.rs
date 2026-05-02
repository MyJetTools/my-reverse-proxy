use my_http_server::{
    macros::{http_route, MyHttpInput, MyHttpObjectStructure},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};
use serde::Serialize;

#[http_route(
    method: "GET",
    route: "/api/IpBlocklist/Check",
    summary: "Check if IP is in the blocklist",
    description: "Returns whether the given source IP is currently blocked",
    input_data: CheckIpBlocklistInput,
    controller: "IpBlocklist",
    result:[
        {status_code: 200, description: "Ok response", model: "CheckIpBlocklistResponse"},
    ]
)]
pub struct CheckIpBlocklistAction;

async fn handle_request(
    _action: &CheckIpBlocklistAction,
    input_data: CheckIpBlocklistInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let ip = input_data
        .ip
        .parse::<std::net::IpAddr>()
        .map_err(|err| HttpFailResult::as_validation_error(format!("Invalid IP: {}", err)))?;

    let blocked = crate::app::APP_CTX.ip_blocklist.is_blocked(&ip);

    HttpOutput::as_json(CheckIpBlocklistResponse { ip: ip.to_string(), blocked })
        .into_ok_result(true)
        .into()
}

#[derive(MyHttpInput)]
pub struct CheckIpBlocklistInput {
    #[http_query(name = "ip", description = "Source IP address to check")]
    pub ip: String,
}

#[derive(Serialize, MyHttpObjectStructure)]
pub struct CheckIpBlocklistResponse {
    pub ip: String,
    pub blocked: bool,
}
