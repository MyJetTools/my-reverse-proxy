use std::sync::Arc;

use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

use crate::app::AppContext;

#[http_route(
    method: "POST",
    route: "/api/configuration/RefreshUsersList",
    summary: "Refresh Users List from settings",
    description: "Refresh Users List from settings",
    input_data: ReloadEndpointHttpInput,
    controller: "Configuration",
    result:[
        {status_code: 204, description: "Ok response"},
    ]
)]
pub struct RefreshUsersListAction {
    app: Arc<AppContext>,
}

impl RefreshUsersListAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &RefreshUsersListAction,
    input_data: ReloadEndpointHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    match crate::flows::refresh_users_list(&action.app, &input_data.users_list_id).await {
        Ok(_) => HttpOutput::Empty.into_ok_result(true),
        Err(err) => Err(HttpFailResult::as_validation_error(err)),
    }
}

#[derive(MyHttpInput)]
pub struct ReloadEndpointHttpInput {
    #[http_form_data(name = "users_list_id", description = "Id of certificate")]
    pub users_list_id: String,
}
