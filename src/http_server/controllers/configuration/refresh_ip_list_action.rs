use std::sync::Arc;

use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

use crate::app::AppContext;

#[http_route(
    method: "POST",
    route: "/api/configuration/RefreshIpList",
    summary: "Refresh Whitelisted Ip List from settings",
    description: "Refresh Whitelisted Ip List from settings",
    input_data: ReloadWhiteListedIpListHttpInput,
    controller: "Configuration",
    result:[
        {status_code: 204, description: "Ok response"},
    ]
)]
pub struct RefreshIpListAction {
    app: Arc<AppContext>,
}

impl RefreshIpListAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &RefreshIpListAction,
    input_data: ReloadWhiteListedIpListHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    match crate::flows::refresh_ip_list_from_settings(&action.app, &input_data.ip_list_id).await {
        Ok(_) => HttpOutput::Empty.into_ok_result(true),
        Err(err) => Err(HttpFailResult::as_validation_error(err)),
    }
}

#[derive(MyHttpInput)]
pub struct ReloadWhiteListedIpListHttpInput {
    #[http_form_data(name = "ip_list_id", description = "Id of ip")]
    pub ip_list_id: String,
}