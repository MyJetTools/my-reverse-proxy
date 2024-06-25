use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use http_body_util::{combinators::BoxBody, BodyExt, StreamBody};
use hyper::{
    body::Incoming,
    header::{self, HeaderName, HeaderValue},
    HeaderMap, Version,
};

use crate::{http_content_source::WebContentType, settings::ModifyHttpHeadersSettings};

use super::{HostPort, HttpProxyPass, HttpProxyPassInner, LocationIndex, ProxyPassError};

pub async fn build_http_response<THostPort: HostPort + Send + Sync + 'static>(
    proxy_pass: &HttpProxyPass,
    inner: &HttpProxyPassInner,
    req_host_port: &THostPort,
    response: hyper::Response<Incoming>,
    location_index: &LocationIndex,
    dest_http1: bool,
) -> Result<hyper::Response<BoxBody<Bytes, String>>, ProxyPassError> {
    let (mut parts, incoming) = response.into_parts();

    if dest_http1 && !proxy_pass.listening_port_info.http_type.is_protocol_http1() {
        parts.headers.remove(header::TRANSFER_ENCODING);
        parts.headers.remove(header::CONNECTION);
        parts.headers.remove(header::UPGRADE);
        parts.headers.remove("keep-alive");
        parts.headers.remove("proxy-connection");
        parts.headers.remove("connection");
    }

    modify_req_headers(
        proxy_pass,
        inner,
        req_host_port,
        &mut parts.headers,
        location_index,
    );

    //let body = into_full_bytes(incoming).await?;
    Ok(hyper::Response::from_parts(
        parts,
        incoming.map_err(|e| e.to_string()).boxed(),
    ))
}

pub async fn build_chunked_http_response<THostPort: HostPort + Send + Sync + 'static>(
    proxy_pass: &HttpProxyPass,
    inner: &HttpProxyPassInner,
    req_host_port: &THostPort,
    mut response: hyper::Response<Incoming>,
    location_index: &LocationIndex,
) -> Result<hyper::Response<BoxBody<Bytes, String>>, ProxyPassError> {
    modify_req_headers(
        proxy_pass,
        inner,
        req_host_port,
        response.headers_mut(),
        location_index,
    );

    let (parts, body) = response.into_parts();

    let mut in_stream = body.into_data_stream();
    let (mut sender, receiver) = futures::channel::mpsc::channel(1024);

    let stream_body = StreamBody::new(receiver);

    tokio::spawn(async move {
        println!("Http 1.1 Channel opened");
        while let Some(chunk) = in_stream.next().await {
            match chunk {
                Ok(chunk) => {
                    let data_len = chunk.len();
                    //println!("Chunk size: {}", bytes.len());
                    //if bytes.len() > 3 {
                    //    println!("Chunk: {:?}", &bytes[bytes.len() - 3..]);
                    // } else {
                    //    println!("Chunk: {:?}", &bytes);
                    // }

                    let chunk = hyper::body::Frame::data(chunk);
                    let send_result = sender.send(Ok(chunk)).await;

                    if let Err(err) = send_result {
                        println!("Channel send error: {:?}", err);
                        break;
                    } else {
                        println!("Sent to channel: {} bytes", data_len);
                    }
                }
                Err(e) => {
                    println!("Channel receive error: {:?}", e);
                    break;
                }
            }
        }

        println!("Http 1.1 Channel closed");
    });

    // let response = response.map_err(|e| e.to_string()).boxed();

    let box_body = stream_body.map_err(|e: hyper::Error| e.to_string()).boxed();
    Ok(hyper::Response::builder()
        .status(200)
        .header("Transfer-Encoding", "chunked")
        .body(box_body)
        .unwrap())
}

pub fn build_response_from_content<THostPort: HostPort + Send + Sync + 'static>(
    http_proxy_pass: &HttpProxyPass,
    inner: &HttpProxyPassInner,
    req_host_port: &THostPort,
    location_index: &LocationIndex,
    content_type: Option<WebContentType>,
    status_code: u16,
    content: Vec<u8>,
) -> hyper::Response<BoxBody<Bytes, String>> {
    let mut builder = hyper::Response::builder().status(status_code);

    if let Some(content_type) = content_type {
        builder = builder.header("Content-Type", content_type.as_str());
    }

    if let Some(headers) = builder.headers_mut() {
        modify_req_headers(
            http_proxy_pass,
            inner,
            req_host_port,
            headers,
            location_index,
        );
    }

    let full_body = http_body_util::Full::new(hyper::body::Bytes::from(content));
    builder
        .body(full_body.map_err(|e| crate::to_hyper_error(e)).boxed())
        .unwrap()
}

fn modify_req_headers<THostPort: HostPort + Send + Sync + 'static>(
    proxy_pass: &HttpProxyPass,
    inner: &HttpProxyPassInner,
    req_host_port: &THostPort,
    headers: &mut HeaderMap<HeaderValue>,
    location_index: &LocationIndex,
) {
    if let Some(modify_headers_settings) = proxy_pass
        .endpoint_info
        .modify_headers_settings
        .global_modify_headers_settings
        .as_ref()
    {
        modify_headers(inner, req_host_port, headers, modify_headers_settings);
    }

    if let Some(modify_headers_settings) = proxy_pass
        .endpoint_info
        .modify_headers_settings
        .endpoint_modify_headers_settings
        .as_ref()
    {
        modify_headers(inner, req_host_port, headers, modify_headers_settings);
    }

    let proxy_pass_location = inner.locations.find(location_index);

    if let Some(modify_headers_settings) = proxy_pass_location.config.modify_headers.as_ref() {
        modify_headers(inner, req_host_port, headers, modify_headers_settings);
    }
}

fn modify_headers<THostPort: HostPort + Send + Sync + 'static>(
    inner: &HttpProxyPassInner,
    req_host_port: &THostPort,
    headers: &mut HeaderMap<hyper::header::HeaderValue>,
    headers_settings: &ModifyHttpHeadersSettings,
) {
    if let Some(remove_header) = headers_settings.remove.as_ref() {
        if let Some(remove_headers) = remove_header.response.as_ref() {
            for remove_header in remove_headers {
                headers.remove(remove_header.as_str());
            }
        }
    }

    if let Some(add_headers) = headers_settings.add.as_ref() {
        if let Some(add_headers) = add_headers.response.as_ref() {
            for add_header in add_headers {
                headers.insert(
                    HeaderName::from_bytes(add_header.name.as_bytes()).unwrap(),
                    inner
                        .populate_value(&add_header.value, req_host_port)
                        .as_str()
                        .parse()
                        .unwrap(),
                );
            }
        }
    }
}
