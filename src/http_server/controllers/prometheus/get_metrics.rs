use std::sync::Arc;

use my_http_server::{macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

use crate::app::AppContext;

#[http_route(
    method: "GET",
    route: "/metrics",
)]
pub struct GetMetricsAction {
    app: Arc<AppContext>,
}

impl GetMetricsAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &GetMetricsAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let content = action.app.prometheus.build();

    HttpOutput::Content {
        headers: None,
        content_type: None,
        content,
        set_cookies: None,
    }
    .into_ok_result(false)
    .into()
}
