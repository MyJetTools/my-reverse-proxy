use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};
use serde::Serialize;

#[http_route(
    method: "POST",
    route: "/api/SSH/InitPassKey",
    summary: "Init ssh pass key to use in ssh connections",
    description: "Init ssh pass key to use in ssh connections",
    controller: "Ssh",
    input_data: InitPassKeyHttpModel,
    result:[
        {status_code: 204, description: "Ok response"},
    ]
)]
pub struct InitPassKeyAction;

async fn handle_request(
    _action: &InitPassKeyAction,
    input_data: InitPassKeyHttpModel,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    crate::app::CERT_PASS_KEYS
        .add(input_data.id, input_data.pass_key)
        .await;

    HttpOutput::Empty.into_ok_result(true).into()
}

#[derive(Debug, Serialize, MyHttpInput)]
pub struct InitPassKeyHttpModel {
    #[http_body(description = "Id of path like name@host:port. Or * for default passkey")]
    pub id: String,
    #[http_body(description = "Passkey")]
    pub pass_key: String,
}
