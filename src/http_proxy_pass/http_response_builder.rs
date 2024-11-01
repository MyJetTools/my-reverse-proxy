use hyper::{
    header::{self, HeaderName, HeaderValue},
    HeaderMap,
};

use crate::settings::ModifyHttpHeadersSettings;

use super::{HostPort, HttpProxyPass, HttpProxyPassInner, LocationIndex};

/*
pub async fn build_http_response<THostPort: HostPort + Send + Sync + 'static>(
    proxy_pass: &HttpProxyPass,
    inner: &HttpProxyPassInner,
    req_host_port: &THostPort,
    response: hyper::Response<Incoming>,
    location_index: &LocationIndex,
    dest_http1: bool,
) -> Result<hyper::Response<BoxBody<Bytes, String>>, ProxyPassError> {
    let (mut parts, incoming) = response.into_parts();

    modify_resp_headers(
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
 */
/*
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
           while let Some(chunk) = in_stream.next().await {
               match chunk {
                   Ok(chunk) => {
                       let data_len = chunk.len();

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

    // let box_body = stream_body.map_err(|e: hyper::Error| e.to_string()).boxed();
    Ok(hyper::Response::from_parts(
        parts,
        body.map_err(|e| e.to_string()).boxed(),
    ))
}
    */

/*
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
 */
pub fn modify_resp_headers<THostPort: HostPort + Send + Sync + 'static>(
    proxy_pass: &HttpProxyPass,
    inner: &HttpProxyPassInner,
    req_host_port: &THostPort,
    headers: &mut HeaderMap<HeaderValue>,
    location_index: &LocationIndex,
) {
    let proxy_pass_location = inner.locations.find(location_index);

    if let Some(dest_http1) = proxy_pass_location.is_http1() {
        if dest_http1 && !proxy_pass.listening_port_info.http_type.is_protocol_http1() {
            headers.remove(header::TRANSFER_ENCODING);
            headers.remove(header::CONNECTION);
            headers.remove(header::UPGRADE);
            headers.remove("keep-alive");
            headers.remove("proxy-connection");
            headers.remove("connection");
        }
    }

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
