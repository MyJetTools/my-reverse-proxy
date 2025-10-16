use my_http_server::{macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput};

#[http_route(
    method: "GET",
    route: "/metrics",
)]
pub struct GetMetricsAction;
async fn handle_request(
    _action: &GetMetricsAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let content = crate::app::APP_CTX.prometheus.build();

    HttpOutput::Content {
        status_code: 200,
        headers: None,
        content_type: None,
        content,
        set_cookies: None,
    }
    .into_ok_result(false)
    .into()
}
