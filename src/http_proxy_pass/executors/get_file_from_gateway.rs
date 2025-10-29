use bytes::Bytes;
use http_body_util::Full;
use my_http_server::WebContentType;

use crate::{http_content_source::RequestExecutorResult, http_proxy_pass::*};

pub async fn get_file_from_gateway(
    gateway_id: &str,
    path: &str,
    default_file: &Option<String>,
    req: &hyper::Request<Full<Bytes>>,
) -> Result<RequestExecutorResult, ProxyPassError> {
    let gateway = crate::app::APP_CTX.get_gateway_by_id(gateway_id).await;

    if gateway.is_none() {
        return Err(ProxyPassError::GatewayError);
    }

    let gateway = gateway.unwrap();

    let full_path = super::merge_path_and_file(path, req.uri().path(), default_file);

    match gateway.request_file(full_path.as_str()).await {
        Ok(content) => Ok(RequestExecutorResult {
            status_code: 200,
            content_type: WebContentType::detect_by_extension(full_path.as_str()),
            body: content,
        }),
        Err(err) => match err {
            crate::tcp_gateway::FileRequestError::FileNotFound => {
                return Err(ProxyPassError::NoLocationFound)
            }
            crate::tcp_gateway::FileRequestError::GatewayDisconnected => {
                return Err(ProxyPassError::GatewayError)
            }
        },
    }
}
