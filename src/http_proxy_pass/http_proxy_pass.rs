use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::Full;
use tokio::sync::Mutex;

use crate::{
    app::AppContext, http_client::HTTP_CLIENT_TIMEOUT, settings::HttpEndpointModifyHeadersSettings,
};

use super::{HttpProxyPassInner, HttpRequestBuilder, ProxyPassError, RetryType};

pub struct HttpProxyPass {
    pub inner: Mutex<HttpProxyPassInner>,
}

impl HttpProxyPass {
    pub fn new(
        socket_addr: SocketAddr,
        modify_headers_settings: HttpEndpointModifyHeadersSettings,
    ) -> Self {
        Self {
            inner: Mutex::new(HttpProxyPassInner::new(
                socket_addr,
                modify_headers_settings,
            )),
        }
    }

    pub async fn update_client_cert_cn_name(&self, client_cert_cn: String) {
        println!("update_client_cert_cn_name{}", client_cert_cn);
        let mut inner = self.inner.lock().await;
        inner.src.client_cert_cn = Some(client_cert_cn);
    }

    pub async fn send_payload(
        &self,
        app: &Arc<AppContext>,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Result<hyper::Response<Full<Bytes>>>, ProxyPassError> {
        let mut req = HttpRequestBuilder::new(req);
        loop {
            let (future1, future2, location_index, request_executor) = {
                let mut inner = self.inner.lock().await;

                if !inner.initialized() {
                    let host_port = req.get_host_port();
                    inner.init(app, &host_port).await?;
                }

                let location_index = req.populate_and_build(&inner).await?;

                let proxy_pass_location = inner.locations.find_mut(&location_index);

                proxy_pass_location.connect_if_require(app).await?;

                let (future1, future2, request_executor) = {
                    match &mut proxy_pass_location.content_source {
                        super::ProxyPassContentSource::Http(http_content_source) => {
                            if http_content_source.remote_endpoint.is_http1() {
                                let result = http_content_source.send_http1_request(req.get());

                                (Some(result), None, None)
                            } else {
                                let future = http_content_source.send_http2_request(req.get());
                                (None, Some(future), None)
                            }
                        }
                        super::ProxyPassContentSource::File(file) => {
                            let executor = file.get_request_executor(req.uri())?;

                            (None, None, Some(executor))
                        }
                    }
                };

                (future1, future2, location_index, request_executor)
            };

            let result = if let Some(future1) = future1 {
                match future1 {
                    Ok(result) => {
                        let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, result).await;

                        if result.is_err() {
                            return Err(ProxyPassError::Timeout);
                        }

                        match result.unwrap() {
                            Ok(result) => Ok(result),
                            Err(err) => Err(err.into()),
                        }
                    }
                    Err(err) => Err(err),
                }
            } else if let Some(future2) = future2 {
                match future2 {
                    Ok(result) => {
                        let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, result).await;

                        if result.is_err() {
                            return Err(ProxyPassError::Timeout);
                        }

                        match result.unwrap() {
                            Ok(result) => Ok(result),
                            Err(err) => Err(err.into()),
                        }
                    }
                    Err(err) => Err(err),
                }
            } else if let Some(request_executor) = request_executor {
                let content = request_executor.execute_request().await?;
                let inner = self.inner.lock().await;
                return Ok(Ok(
                    super::http_response_builder::build_response_from_content(
                        req.uri(),
                        &inner,
                        &location_index,
                        content,
                    ),
                ));
            } else {
                panic!("Both futures are None")
            };

            match result {
                Ok(response) => {
                    let inner = self.inner.lock().await;
                    let response = super::http_response_builder::build_http_response(
                        req.uri(),
                        response,
                        &inner,
                        &location_index,
                    )
                    .await?;
                    return Ok(Ok(response));
                }
                Err(err) => {
                    let retry = {
                        let mut inner = self.inner.lock().await;
                        inner.handle_error(app, &err, &location_index).await?
                    };

                    match retry {
                        RetryType::NoRetry => return Err(err.into()),
                        RetryType::Retry(duration) => {
                            if let Some(duration) = duration {
                                tokio::time::sleep(duration).await;
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn dispose(&self) {
        print!("ProxyPassClient is disposed");
        let mut inner = self.inner.lock().await;
        inner.disposed = true;
    }
}
