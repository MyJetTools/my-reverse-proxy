use my_http_server::{
    macros::http_route, HttpContext, HttpFailResult, HttpOkResult, HttpOutput, WebContentType,
};

use crate::http_server::controllers::configuration::contracts::CurrentConfigurationHttpModel;

#[http_route(
    method: "GET",
    route: "/",
)]
pub struct IndexAction;

async fn handle_request(
    _action: &IndexAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let config = CurrentConfigurationHttpModel::new().await;
    HttpOutput::Content {
        status_code: 200,
        headers: None,
        content_type: WebContentType::Html.into(),
        content: create_html_content(config).await.into_bytes(),
        set_cookies: None,
    }
    .into_ok_result(false)
}

async fn create_html_content(config_model: CurrentConfigurationHttpModel) -> String {
    let content = super::render_content(&config_model).await;

    let javascript = javascript();
    format!(
        r##"
    <!DOCTYPE html>
    <html>
        <head>
            <title>MyReverseProxy</title>
            <meta http-equiv="content-type" content="text/html; charset=utf-8">
            <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.3/dist/css/bootstrap.min.css" rel="stylesheet" integrity="sha384-QWTKZyjpPEjISv5WaRU9OFeRpok6YctnYmDr5pNlyT2bRjXh0JMhjY6hW+ALEwIH" crossorigin="anonymous">
            <script>
            function background(){{
            {javascript}
            }}
              window.setInterval(() =>background(), 1000);
            </script>
        </head>
        <body>
            {content}
        </body>
        "##
    )
}

fn javascript() -> &'static str {
    r#"
    fetch("/content")
      .then((response) => {

          response.text().then((html)=>{
           document.body.innerHTML = html
           });

    
  })
  .catch((error) => console.error("Fetch error:", error)); // In case of an error,
    "#
}
