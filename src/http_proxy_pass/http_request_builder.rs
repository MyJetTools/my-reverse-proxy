use std::io::Write;

use bytes::Bytes;
use flate2::{write::GzEncoder, Compression};
use http_body_util::{BodyExt, Full};
use hyper::{
    body::Incoming,
    header::{HeaderName, HeaderValue, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE},
    HeaderMap, Request, Uri,
};
use hyper_tungstenite::{tungstenite::http::request::Parts, HyperWebsocket};

use crate::{configurations::*, settings::ModifyHttpHeadersSettings};

use super::{HostPort, HttpProxyPass, HttpProxyPassInner, ProxyPassError, ProxyPassLocation};

pub const AUTHORIZED_COOKIE_NAME: &str = "x-authorized";

pub struct WebSocketUpgrade {
    pub upgrade_response: hyper::Response<Full<Bytes>>,
    pub server_web_socket: HyperWebsocket,
}

pub struct TransformedRequest {
    pub request: hyper::Request<Full<Bytes>>,
    pub req_parts: Parts,
    pub web_socket_upgrade: Option<WebSocketUpgrade>,
}

pub struct HttpRequestBuilder {
    parts: Parts,
    body: Incoming,
    src_http_type: ListenHttpEndpointType,
}

impl HttpRequestBuilder {
    pub fn new(
        src_http_type: ListenHttpEndpointType,
        src: hyper::Request<hyper::body::Incoming>,
    ) -> Self {
        let (parts, body) = src.into_parts();
        Self {
            parts,
            body,
            src_http_type,
        }
    }

    pub async fn into_response(
        self,
        proxy_pass: &HttpProxyPass,
        location: &ProxyPassLocation,
    ) -> Result<TransformedRequest, ProxyPassError> {
        //let location_index = inner.locations.find_location_index(self.uri())?;

        let dest_http1 = location.is_http1();
        //let (compress, dest_http1, debug) = { (item.compress, item.is_http1(), item.debug) };

        if dest_http1.is_none() {
            let (parts, body) = self
                .build_request(location.compress, location.debug)
                .await?;

            return Ok(TransformedRequest {
                req_parts: parts.clone(),
                request: Request::from_parts(parts, body),
                web_socket_upgrade: None,
            });
        }

        let dest_http1 = dest_http1.unwrap();

        if !self.src_http_type.is_http1() && dest_http1 {
            return self.http2_to_http1(location).await;
        }

        let (parts, body) = self
            .build_request(location.compress, location.debug)
            .await?;

        let mut web_socket_upgrade = None;
        if parts.headers.get("sec-websocket-key").is_some() {
            if proxy_pass.endpoint_info.debug {
                println!("Detected Upgrade http1->http1");
            }

            let upgrade_req = hyper::Request::from_parts(parts.clone(), body.clone());
            let upgrade_response = hyper_tungstenite::upgrade(upgrade_req, None)?;

            web_socket_upgrade = Some(WebSocketUpgrade {
                upgrade_response: upgrade_response.0,
                server_web_socket: upgrade_response.1,
            });
        }

        return Ok(TransformedRequest {
            req_parts: parts.clone(),
            request: Request::from_parts(parts, body),

            web_socket_upgrade,
        });
        //return Ok((location_index, ));
    }

    async fn http2_to_http1(
        self,
        location: &ProxyPassLocation,
    ) -> Result<TransformedRequest, ProxyPassError> {
        let path_and_query = if let Some(path_and_query) = self.parts.uri.path_and_query() {
            path_and_query.as_str()
        } else {
            "/"
        };

        let uri: Uri = path_and_query.parse().unwrap();

        let host_header = if let Some(port) = self.parts.uri.port() {
            format!("{}:{}", self.parts.uri.host().unwrap(), port)
        } else {
            if let Some(host) = self.parts.uri.host() {
                host.to_string()
            } else {
                if let Some(host) = self.parts.get_headers().get("host") {
                    host.to_str().unwrap().to_string()
                } else {
                    println!("Parts: {:?}", self.parts);
                    panic!("No host found in uri: {:?}", self.parts.uri);
                }
            }
        };

        let mut builder = Request::builder()
            .uri(uri)
            .method(self.parts.method.clone())
            .header("host", host_header);

        for header in self.parts.headers.iter() {
            builder = builder.header(header.0, header.1);
        }

        let collected = self.body.collect().await?;
        let bytes = collected.to_bytes();

        let req_parts = self.parts.clone();

        let body = into_full_body(bytes, location.debug);

        let request = builder.body(body).unwrap();

        if location.debug {
            println!(
                "[{}]. After Conversion Request: {:?}. ",
                request.uri(),
                request.headers()
            );
        }

        return Ok(TransformedRequest {
            req_parts,
            request,
            web_socket_upgrade: None,
        });
    }

    pub fn uri(&self) -> &Uri {
        &self.parts.uri
    }

    pub fn get_from_query(&self, param: &str) -> Option<String> {
        let query = self.get_uri().query()?;

        for itm in query.split("&") {
            let mut parts = itm.split("=");

            let left = parts.next().unwrap().trim();

            if let Some(right) = parts.next() {
                if left == param {
                    return Some(
                        my_settings_reader::flurl::url_utils::decode_from_url_string(right.trim())
                            .to_string(),
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

    async fn build_request(
        self,
        compress: bool,
        debug: bool,
    ) -> Result<(Parts, Full<Bytes>), ProxyPassError> {
        use http_body_util::BodyExt;

        let mut parts = self.parts.clone();

        let collected = self.body.collect().await?;
        let bytes = collected.to_bytes();

        let before_compress = bytes.len();

        let body = if compress && before_compress >= 2048 {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&bytes)?;
            let compressed_data = encoder.finish()?;
            let compressed_data_len = compressed_data.len();

            if debug {
                println!("Compressed: {} -> {}", before_compress, compressed_data_len);
            }

            parts.headers.remove(CONTENT_ENCODING);
            parts.headers.remove(CONTENT_TYPE);
            parts
                .headers
                .append(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
            parts.headers.append(
                CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );

            parts.headers.remove(CONTENT_LENGTH);

            parts.headers.append(
                CONTENT_LENGTH,
                HeaderValue::from_str(compressed_data_len.to_string().as_str()).unwrap(),
            );

            http_body_util::Full::new(compressed_data.into())
        } else {
            into_full_body(bytes, debug)
        };

        if debug {
            println!(
                "[{}]. After Conversion Request: {:?}.",
                parts.uri, parts.headers
            );
        }
        Ok((parts, body))
    }

    pub fn process_headers<'s>(
        &mut self,
        proxy_pass: &HttpProxyPass,
        inner: &'s HttpProxyPassInner,
        location: &ProxyPassLocation,
    ) {
        if let Some(modify_headers_settings) = proxy_pass
            .endpoint_info
            .modify_headers_settings
            .global_modify_headers_settings
            .as_ref()
        {
            modify_headers(inner, &mut self.parts, modify_headers_settings);
        }

        if let Some(modify_headers_settings) = proxy_pass
            .endpoint_info
            .modify_headers_settings
            .endpoint_modify_headers_settings
            .as_ref()
        {
            modify_headers(inner, &mut self.parts, modify_headers_settings);
        }

        if let Some(modify_headers_settings) = location.config.modify_headers.as_ref() {
            modify_headers(inner, &mut self.parts, modify_headers_settings);
        }
    }
}

fn into_full_body(src: Bytes, debug: bool) -> Full<Bytes> {
    if debug {
        println!("Body Len: {}", src.len());
    }

    http_body_util::Full::new(src)
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
        &self.parts.uri
    }

    fn get_headers(&self) -> &HeaderMap<HeaderValue> {
        &self.parts.headers
    }
}
