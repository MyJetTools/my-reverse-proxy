use my_http_server::{
    macros::{http_route, MyHttpInput},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};

#[http_route(
    method: "POST",
    route: "/api/configuration/RefreshUsersList",
    summary: "Refresh Users List from settings",
    description: "Refresh Users List from settings",
    input_data: ReloadUsersListHttpInput,
    controller: "Configuration",
    result:[
        {status_code: 204, description: "Ok response"},
    ]
)]
pub struct RefreshUsersListAction;

async fn handle_request(
    _action: &RefreshUsersListAction,
    input_data: ReloadUsersListHttpInput,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    match crate::flows::refresh_users_list(&input_data.users_list_id).await {
        Ok(_) => HttpOutput::Empty.into_ok_result(true),
        Err(err) => Err(HttpFailResult::as_validation_error(err)),
    }
}

#[derive(MyHttpInput)]
pub struct ReloadUsersListHttpInput {
    #[http_form_data(name = "users_list_id", description = "Id of certificate")]
    pub users_list_id: String,
}
