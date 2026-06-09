use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use hyper::header::{HeaderValue, HOST};
use my_http_client::http1::{
    MyHttpClient, MyHttpClientMetrics, MyHttpRequest, MyHttpResponse,
};
use rust_extensions::remote_endpoint::{RemoteEndpointOwned, Scheme};

use crate::{
    app::APP_CTX,
    http_client_connectors::{HttpConnector, HttpTlsConnector},
    http_proxy_pass::ProxyPassError,
};

use super::{attach_conn_guard, HttpResponse, WebSocketUpgradeStream};

pub struct DynamicProxyContentSource {
    pub request_timeout: Duration,
    pub connect_timeout: Duration,
    pub allowed_hosts: Option<Arc<Vec<String>>>,
    pub debug: bool,
}

impl DynamicProxyContentSource {
    pub async fn execute(
        &self,
        mut req: http::Request<Full<Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        let proxy_to_header = req
            .headers_mut()
            .remove("proxy-to")
            .ok_or(ProxyPassError::ProxyToHeaderMissing)?;

        let proxy_to = proxy_to_header
            .to_str()
            .map_err(|_| ProxyPassError::ProxyToHeaderInvalid)?
            .to_string();

        let endpoint = RemoteEndpointOwned::try_parse(proxy_to)
            .map_err(|_| ProxyPassError::ProxyToHeaderInvalid)?;

        if let Some(allowed) = &self.allowed_hosts {
            let host = endpoint.get_host();
            if !allowed.iter().any(|h| h.eq_ignore_ascii_case(host)) {
                return Err(ProxyPassError::ProxyToHostNotAllowed);
            }
        }

        let path_and_query = req
            .uri()
            .path_and_query()
            .map(|p| p.as_str().to_string())
            .unwrap_or_else(|| "/".to_string());

        *req.uri_mut() = path_and_query
            .parse()
            .map_err(|_| ProxyPassError::ProxyToHeaderInvalid)?;

        let host_value = endpoint.get_host_port();
        req.headers_mut().insert(
            HOST,
            HeaderValue::from_str(host_value.as_str())
                .map_err(|_| ProxyPassError::ProxyToHeaderInvalid)?,
        );

        let endpoint_arc = Arc::new(endpoint);
        let metrics: Arc<dyn MyHttpClientMetrics + Send + Sync + 'static> =
            APP_CTX.prometheus.clone();

        let my_req = MyHttpRequest::from_hyper_request(req).await;

        match endpoint_arc.get_scheme() {
            Some(Scheme::Http) | Some(Scheme::Ws) => {
                let connector = HttpConnector {
                    remote_endpoint: endpoint_arc.clone(),
                    debug: self.debug,
                };
                let mut client = MyHttpClient::new_with_metrics(connector, metrics);
                client.set_connect_timeout(self.connect_timeout);
                match client.do_request(&my_req, self.request_timeout).await? {
                    MyHttpResponse::Response(r) => {
                        // The client owns the upstream connection — tie it to
                        // the body so a streaming response is not cut off when
                        // `execute` returns.
                        Ok(HttpResponse::Response(attach_conn_guard(
                            r,
                            Box::new(client),
                        )))
                    }
                    MyHttpResponse::WebSocketUpgrade {
                        stream,
                        response,
                        disconnection,
                    } => Ok(HttpResponse::WebSocketUpgrade {
                        stream: WebSocketUpgradeStream::TcpStream(stream),
                        response,
                        disconnection,
                    }),
                }
            }
            Some(Scheme::Https) | Some(Scheme::Wss) => {
                let connector = HttpTlsConnector {
                    remote_endpoint: endpoint_arc.clone(),
                    domain_name: None,
                    debug: self.debug,
                };
                let mut client = MyHttpClient::new_with_metrics(connector, metrics);
                client.set_connect_timeout(self.connect_timeout);
                match client.do_request(&my_req, self.request_timeout).await? {
                    MyHttpResponse::Response(r) => {
                        // The client owns the upstream connection — tie it to
                        // the body so a streaming response is not cut off when
                        // `execute` returns.
                        Ok(HttpResponse::Response(attach_conn_guard(
                            r,
                            Box::new(client),
                        )))
                    }
                    MyHttpResponse::WebSocketUpgrade {
                        stream,
                        response,
                        disconnection,
                    } => Ok(HttpResponse::WebSocketUpgrade {
                        stream: WebSocketUpgradeStream::TlsStream(stream),
                        response,
                        disconnection,
                    }),
                }
            }
            _ => Err(ProxyPassError::ProxyToHeaderInvalid),
        }
    }
}
