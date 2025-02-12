use my_http_server::{
    macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType,
};

use crate::http_server::controllers::configuration::contracts::CurrentConfigurationHttpModel;

#[http_route(
    method: "GET",
    route: "/content",
)]
pub struct GetContentAction;

async fn handle_request(
    _action: &GetContentAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let config_model = CurrentConfigurationHttpModel::new().await;
    let content = super::render_content(&config_model).await;
    HttpOutput::Content {
        headers: None,
        content_type: WebContentType::Html.into(),
        content: content.into_bytes(),
        set_cookies: None,
    }
    .into_ok_result(false)
}
