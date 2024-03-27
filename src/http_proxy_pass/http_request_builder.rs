use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    header::{HeaderName, HeaderValue},
    HeaderMap, Request, Uri,
};
use hyper_tungstenite::{tungstenite::http::request::Parts, HyperWebsocket};
use tokio::sync::Mutex;

use crate::settings::ModifyHttpHeadersSettings;

use super::{
    HostPort, HttpProxyPassInner, HttpType, LocationIndex, ProxyPassError, SourceHttpData,
};

pub const AUTHORIZED_COOKIE_NAME: &str = "x-authorized";

#[derive(Clone)]
pub enum BuildResult {
    HttpRequest(LocationIndex),
    WebSocketUpgrade {
        location_index: LocationIndex,
        upgrade_response: hyper::Response<Full<Bytes>>,
        web_socket: Arc<Mutex<Option<HyperWebsocket>>>,
    },
}

impl BuildResult {
    pub fn get_location_index(&self) -> &LocationIndex {
        match self {
            BuildResult::HttpRequest(location_index) => location_index,
            BuildResult::WebSocketUpgrade { location_index, .. } => location_index,
        }
    }
}

pub struct HttpRequestBuilder {
    src: Option<hyper::Request<hyper::body::Incoming>>,
    prepared_request: Option<hyper::Request<Full<Bytes>>>,
    src_http_type: HttpType,
    last_result: Option<BuildResult>,
    pub g_auth_user: Option<String>,
}

impl HttpRequestBuilder {
    pub fn new(src_http_type: HttpType, src: hyper::Request<hyper::body::Incoming>) -> Self {
        Self {
            src: Some(src),
            prepared_request: None,
            src_http_type,
            last_result: None,
            g_auth_user: None,
        }
    }

    pub async fn populate_and_build(
        &mut self,
        inner: &HttpProxyPassInner,
    ) -> Result<BuildResult, ProxyPassError> {
        let location_index = inner.locations.find_location_index(self.uri())?;
        if let Some(last_result) = &self.last_result {
            return Ok(last_result.clone());
        }

        let dest_http1 = inner.locations.find(&location_index).is_http1();

        if dest_http1.is_none() {
            return Ok(BuildResult::HttpRequest(location_index));
        }

        let dest_http1 = dest_http1.unwrap();

        if self.src_http_type.is_http1() {
            if dest_http1 {
                // src_http1 && dest_http1
                let (mut parts, incoming) = self.src.take().unwrap().into_parts();

                let websocket_update = parts.headers.get("sec-websocket-key").is_some();

                handle_headers(
                    inner,
                    &mut parts,
                    &location_index,
                    self.g_auth_user.as_deref(),
                );
                let body = into_full_bytes(incoming).await?;

                if websocket_update {
                    println!("Detected Upgrade http1->http1");
                    let upgrade_req = hyper::Request::from_parts(parts.clone(), body.clone());
                    let (response, web_socket) = hyper_tungstenite::upgrade(upgrade_req, None)?;
                    //tokio::spawn(super::web_socket_loop(web_socket));

                    let request = hyper::Request::from_parts(parts, body);

                    self.prepared_request = Some(request);

                    self.last_result = Some(BuildResult::HttpRequest(location_index.clone()));

                    return Ok(BuildResult::WebSocketUpgrade {
                        location_index,
                        upgrade_response: response,
                        web_socket: Arc::new(Mutex::new(Some(web_socket))),
                    });
                }

                let result = hyper::Request::from_parts(parts, body);
                self.prepared_request = Some(result);
                self.last_result = Some(BuildResult::HttpRequest(location_index.clone()));
                return Ok(BuildResult::HttpRequest(location_index));
            } else {
                let (mut parts, incoming) = self.src.take().unwrap().into_parts();

                handle_headers(
                    inner,
                    &mut parts,
                    &location_index,
                    self.g_auth_user.as_deref(),
                );
                let body = into_full_bytes(incoming).await?;

                let request = hyper::Request::from_parts(parts, body);

                self.prepared_request = Some(request);

                self.last_result = Some(BuildResult::HttpRequest(location_index.clone()));
                Ok(BuildResult::HttpRequest(location_index))
            }
        } else {
            if dest_http1 {
                return self.http2_to_http1(location_index).await;
            } else {
                // src_http2 && dest_http2
                let (mut parts, incoming) = self.src.take().unwrap().into_parts();
                handle_headers(
                    inner,
                    &mut parts,
                    &location_index,
                    self.g_auth_user.as_deref(),
                );
                let body = into_full_bytes(incoming).await?;

                self.prepared_request = Some(hyper::Request::from_parts(parts, body));

                self.last_result = Some(BuildResult::HttpRequest(location_index.clone()));
                Ok(BuildResult::HttpRequest(location_index))
            }
        }
    }

    async fn http2_to_http1(
        &mut self,
        location_index: LocationIndex,
    ) -> Result<BuildResult, ProxyPassError> {
        let (parts, incoming) = self.src.take().unwrap().into_parts();

        let path_and_query = if let Some(path_and_query) = parts.uri.path_and_query() {
            path_and_query.as_str()
        } else {
            "/"
        };

        let uri: Uri = path_and_query.parse().unwrap();

        let host_header = if let Some(port) = parts.uri.port() {
            format!("{}:{}", parts.uri.host().unwrap(), port)
        } else {
            parts.uri.host().unwrap().to_string()
        };

        let mut builder = Request::builder()
            .uri(uri)
            .method(parts.method.clone())
            .header("host", host_header);

        for header in parts.headers.iter() {
            builder = builder.header(header.0, header.1);
        }

        let body = into_full_bytes(incoming).await?;

        if parts.headers.get("sec-websocket-key").is_some() {
            println!("Detected Upgrade");
            let req = hyper::Request::from_parts(parts, body.clone());
            let (response, web_socket) = hyper_tungstenite::upgrade(req, None)?;
            //tokio::spawn(super::web_socket_loop(web_socket));
            let request = builder.body(body).unwrap();

            self.prepared_request = Some(request);

            self.last_result = Some(BuildResult::HttpRequest(location_index.clone()));

            return Ok(BuildResult::WebSocketUpgrade {
                location_index,
                upgrade_response: response,
                web_socket: Arc::new(Mutex::new(Some(web_socket))),
            });
        }
        let result = builder.body(body).unwrap();
        self.prepared_request = Some(result);
        self.last_result = Some(BuildResult::HttpRequest(location_index.clone()));
        return Ok(BuildResult::HttpRequest(location_index));
    }

    pub fn uri(&self) -> &Uri {
        if let Some(src) = self.src.as_ref() {
            return src.uri();
        }

        self.prepared_request.as_ref().unwrap().uri()
    }

    pub fn get_from_query(&self, param: &str) -> Option<String> {
        let query = self.get_uri().query()?;

        for itm in query.split("&") {
            let mut parts = itm.split("=");

            let left = parts.next().unwrap().trim();

            if let Some(right) = parts.next() {
                if left == param {
                    return Some(
                        my_settings_reader::flurl::url_utils::decode_from_url_string(right.trim()),
                    );
                }
            }
        }

        None
    }

    pub fn get_cookie(&self, cookie_name: &str) -> Option<&str> {
        let auth_token = self.get_headers().get("Cookie")?;

        match auth_token.to_str() {
            Ok(result) => {
                println!("Cookie: {}", result);
                for itm in result.split(";") {
                    let mut parts = itm.split("=");

                    let left = parts.next().unwrap().trim();

                    if let Some(right) = parts.next() {
                        if left == cookie_name {
                            println!("'{}' = '{}'", left, right.trim());
                            return Some(right.trim());
                        }
                    }
                }

                Some(result)
            }
            Err(_) => None,
        }
    }

    pub fn get_authorization_token(&self) -> Option<&str> {
        let result = self.get_cookie(AUTHORIZED_COOKIE_NAME);
        result
    }

    pub fn get(&self) -> hyper::Request<Full<Bytes>> {
        self.prepared_request.as_ref().unwrap().clone()
    }
}

pub async fn into_full_bytes(
    incoming: impl hyper::body::Body<Data = hyper::body::Bytes, Error = hyper::Error>,
) -> Result<Full<Bytes>, ProxyPassError> {
    use http_body_util::BodyExt;

    let collected = incoming.collect().await?;
    let bytes = collected.to_bytes();

    let body = http_body_util::Full::new(bytes);
    Ok(body)
}

fn handle_headers(
    inner: &HttpProxyPassInner,
    parts: &mut Parts,
    location_index: &LocationIndex,
    x_auth_user: Option<&str>,
) {
    if let Some(modify_headers_settings) = inner
        .modify_headers_settings
        .global_modify_headers_settings
        .as_ref()
    {
        modify_headers(parts, modify_headers_settings, &inner.src, x_auth_user);
    }

    if let Some(modify_headers_settings) = inner
        .modify_headers_settings
        .endpoint_modify_headers_settings
        .as_ref()
    {
        modify_headers(parts, modify_headers_settings, &inner.src, x_auth_user);
    }

    let proxy_pass_location = inner.locations.find(location_index);

    if let Some(modify_headers_settings) = proxy_pass_location.modify_headers.as_ref() {
        modify_headers(parts, modify_headers_settings, &inner.src, x_auth_user);
    }
}

fn modify_headers<'s>(
    parts: &mut Parts,
    headers_settings: &ModifyHttpHeadersSettings,
    src: &SourceHttpData,
    x_auth_user: Option<&str>,
) {
    if let Some(remove_header) = headers_settings.remove.as_ref() {
        if let Some(remove_headers) = remove_header.request.as_ref() {
            for remove_header in remove_headers {
                parts.headers.remove(remove_header.as_str());
            }
        }
    }

    if let Some(add_headers) = headers_settings.add.as_ref() {
        if let Some(add_headers) = add_headers.request.as_ref() {
            for add_header in add_headers {
                let value = src.populate_value(&add_header.value, parts, x_auth_user);
                if !value.as_str().is_empty() {
                    parts.headers.insert(
                        HeaderName::from_bytes(add_header.name.as_bytes()).unwrap(),
                        src.populate_value(&add_header.value, parts, x_auth_user)
                            .as_str()
                            .parse()
                            .unwrap(),
                    );
                }
            }
        }
    }
}

impl HostPort for HttpRequestBuilder {
    fn get_uri(&self) -> &Uri {
        if let Some(src) = self.src.as_ref() {
            return src.uri();
        }

        self.prepared_request.as_ref().unwrap().uri()
    }

    fn get_headers(&self) -> &HeaderMap<HeaderValue> {
        if let Some(src) = self.src.as_ref() {
            return src.headers();
        }

        self.prepared_request.as_ref().unwrap().headers()
    }
}
