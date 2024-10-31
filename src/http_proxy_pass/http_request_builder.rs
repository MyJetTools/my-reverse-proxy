use std::{io::Write, sync::Arc};

use bytes::Bytes;
use flate2::{write::GzEncoder, Compression};
use http_body_util::Full;
use hyper::{
    header::{HeaderName, HeaderValue, CONTENT_ENCODING, CONTENT_TYPE},
    HeaderMap, Request, Uri,
};
use hyper_tungstenite::{tungstenite::http::request::Parts, HyperWebsocket};
use tokio::sync::Mutex;

use crate::{configurations::*, settings::ModifyHttpHeadersSettings};

use super::{HostPort, HttpProxyPass, HttpProxyPassInner, LocationIndex, ProxyPassError};

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
}

impl HttpRequestBuilder {
    pub fn new(src_http_type: HttpType, src: hyper::Request<hyper::body::Incoming>) -> Self {
        Self {
            src: Some(src),
            prepared_request: None,
            src_http_type,
            last_result: None,
        }
    }

    pub async fn populate_and_build(
        &mut self,
        proxy_pass: &HttpProxyPass,
        inner: &HttpProxyPassInner,
    ) -> Result<BuildResult, ProxyPassError> {
        let location_index = inner.locations.find_location_index(self.uri())?;
        if let Some(last_result) = &self.last_result {
            return Ok(last_result.clone());
        }

        let (compress, dest_http1) = {
            let item = inner.locations.find(&location_index);

            (item.compress, item.is_http1())
        };

        if dest_http1.is_none() {
            return Ok(BuildResult::HttpRequest(location_index));
        }

        let dest_http1 = dest_http1.unwrap();

        if self.src_http_type.is_protocol_http1() {
            if dest_http1 {
                // src_http1 && dest_http1
                let (mut parts, incoming) = self.src.take().unwrap().into_parts();

                let websocket_update = parts.headers.get("sec-websocket-key").is_some();

                handle_headers(proxy_pass, inner, &mut parts, &location_index);

                let body = into_full_bytes(
                    &mut parts,
                    incoming,
                    compress,
                    proxy_pass.endpoint_info.debug,
                )
                .await?;

                if websocket_update {
                    if proxy_pass.endpoint_info.debug {
                        println!("Detected Upgrade http1->http1");
                    }

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

                handle_headers(proxy_pass, inner, &mut parts, &location_index);
                let body = into_full_bytes(
                    &mut parts,
                    incoming,
                    compress,
                    proxy_pass.endpoint_info.debug,
                )
                .await?;

                let request = hyper::Request::from_parts(parts, body);

                self.prepared_request = Some(request);

                self.last_result = Some(BuildResult::HttpRequest(location_index.clone()));
                Ok(BuildResult::HttpRequest(location_index))
            }
        } else {
            if dest_http1 {
                return self
                    .http2_to_http1(location_index, compress, proxy_pass.endpoint_info.debug)
                    .await;
            } else {
                // src_http2 && dest_http2
                let (mut parts, incoming) = self.src.take().unwrap().into_parts();
                handle_headers(proxy_pass, inner, &mut parts, &location_index);
                let body = into_full_bytes(
                    &mut parts,
                    incoming,
                    compress,
                    proxy_pass.endpoint_info.debug,
                )
                .await?;

                self.prepared_request = Some(hyper::Request::from_parts(parts, body));

                self.last_result = Some(BuildResult::HttpRequest(location_index.clone()));
                Ok(BuildResult::HttpRequest(location_index))
            }
        }
    }

    async fn http2_to_http1(
        &mut self,
        location_index: LocationIndex,
        compress: bool,
        debug: bool,
    ) -> Result<BuildResult, ProxyPassError> {
        let (mut parts, incoming) = self.src.take().unwrap().into_parts();

        let path_and_query = if let Some(path_and_query) = parts.uri.path_and_query() {
            path_and_query.as_str()
        } else {
            "/"
        };

        let uri: Uri = path_and_query.parse().unwrap();

        let host_header = if let Some(port) = parts.uri.port() {
            format!("{}:{}", parts.uri.host().unwrap(), port)
        } else {
            if let Some(host) = parts.uri.host() {
                host.to_string()
            } else {
                if let Some(host) = parts.get_headers().get("host") {
                    host.to_str().unwrap().to_string()
                } else {
                    println!("Parts: {:?}", parts);
                    panic!("No host found in uri: {:?}", parts.uri);
                }
            }
        };

        let mut builder = Request::builder()
            .uri(uri)
            .method(parts.method.clone())
            .header("host", host_header);

        for header in parts.headers.iter() {
            builder = builder.header(header.0, header.1);
        }

        let body = into_full_bytes(&mut parts, incoming, compress, debug).await?;

        if parts.headers.get("sec-websocket-key").is_some() {
            if debug {
                println!("Detected Upgrade");
            }
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
                for itm in result.split(";") {
                    if let Some(eq_index) = itm.find("=") {
                        let name = itm[..eq_index].trim();

                        if name == cookie_name {
                            let value = &itm[eq_index + 1..];
                            return Some(value);
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

    /*
    pub fn is_mine(&self, host_str: &str, is_https: bool) -> bool {
        let mut parts = host_str.split(":");

        let left = parts.next().unwrap();

        let req_port = if let Some(port) = self.get_port() {
            port
        } else {
            if is_https {
                443
            } else {
                80
            }
        };

        if let Some(right_part) = parts.next() {
            let port = right_part.parse::<u16>().unwrap();
            if let Some(req_host) = self.get_host() {
                return port == req_port && req_host == left;
            }

            return false;
        }

        let port = left.parse::<u16>().unwrap();
        return req_port == port;
    }
     */
}

pub async fn into_full_bytes(
    headers: &mut Parts,
    incoming: impl hyper::body::Body<Data = hyper::body::Bytes, Error = hyper::Error>,
    compress: bool,
    debug: bool,
) -> Result<Full<Bytes>, ProxyPassError> {
    use http_body_util::BodyExt;

    let collected = incoming.collect().await?;
    let bytes = collected.to_bytes();

    let before_compress = bytes.len();

    let body = if compress && before_compress >= 2048 {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&bytes)?;
        let compressed_data = encoder.finish()?;
        println!(
            "Compressed: {} -> {}",
            before_compress,
            compressed_data.len()
        );

        headers
            .headers
            .append(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        headers.headers.append(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        http_body_util::Full::new(compressed_data.into())
    } else {
        into_full_body(bytes, debug)
    };

    Ok(body)
}

fn into_full_body(src: Bytes, debug: bool) -> Full<Bytes> {
    if debug {
        println!("Body Len: {}", src.len());
    }
    Full::new(Bytes::from(src))
}
fn handle_headers(
    proxy_pass: &HttpProxyPass,
    inner: &HttpProxyPassInner,
    parts: &mut Parts,
    location_index: &LocationIndex,
) {
    if let Some(modify_headers_settings) = proxy_pass
        .endpoint_info
        .modify_headers_settings
        .global_modify_headers_settings
        .as_ref()
    {
        modify_headers(inner, parts, modify_headers_settings);
    }

    if let Some(modify_headers_settings) = proxy_pass
        .endpoint_info
        .modify_headers_settings
        .endpoint_modify_headers_settings
        .as_ref()
    {
        modify_headers(inner, parts, modify_headers_settings);
    }

    let proxy_pass_location = inner.locations.find(location_index);

    if let Some(modify_headers_settings) = proxy_pass_location.config.modify_headers.as_ref() {
        modify_headers(inner, parts, modify_headers_settings);
    }
}

fn modify_headers<'s>(
    inner: &HttpProxyPassInner,
    parts: &mut Parts,
    headers_settings: &ModifyHttpHeadersSettings,
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
                let value = inner.populate_value(&add_header.value, parts);
                if !value.as_str().is_empty() {
                    let value = inner.populate_value(&add_header.value, parts);
                    //println!("Adding Header: '{}'='{}'", add_header.name, value.as_str());
                    parts.headers.insert(
                        HeaderName::from_bytes(add_header.name.as_bytes()).unwrap(),
                        value.as_str().parse().unwrap(),
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
