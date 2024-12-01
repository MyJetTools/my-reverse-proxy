use std::sync::Arc;

use my_http_server::{
    macros::{http_route, MyHttpObjectStructure},
    HttpContext, HttpFailResult, HttpOkResult, HttpOutput,
};
use serde::Serialize;

use crate::app::AppContext;

#[http_route(
    method: "GET",
    route: "/api/SslCertificates/Current",
    summary: "Get current ssl certificates",
    description: "Get current ssl certificates",
    controller: "SslCertificates",
    result:[
        {status_code: 200, description: "Ok response", model:"Vec<CurrentSslCertificateHttpModel>"},
    ]
)]
pub struct GetCurrentSslCertificatesAction {
    app: Arc<AppContext>,
}

impl GetCurrentSslCertificatesAction {
    pub fn new(app: Arc<AppContext>) -> Self {
        Self { app }
    }
}
async fn handle_request(
    action: &GetCurrentSslCertificatesAction,
    _ctx: &HttpContext,
) -> Result<HttpOkResult, HttpFailResult> {
    let config = action.app.get_current_app_configuration().await;

    let mut result: Vec<CurrentSslCertificateHttpModel> = Vec::new();
    for (key, holder) in config.ssl_certificates_cache.get_list().await {
        let cert_info = holder.ssl_cert.get_cert_info().await;
        let cert = CurrentSslCertificateHttpModel {
            id: key,
            cn: cert_info.cn.to_string(),
            expires: cert_info.expires.to_rfc3339(),
        };

        result.push(cert);
    }

    HttpOutput::as_json(result).into_ok_result(true).into()
}

#[derive(Debug, Serialize, MyHttpObjectStructure)]
pub struct CurrentSslCertificateHttpModel {
    pub id: String,
    pub cn: String,
    pub expires: String,
}
