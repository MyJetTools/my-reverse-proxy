use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::Full;
use tokio::sync::Mutex;

use crate::{
    app::AppContext, http_client::HTTP_CLIENT_TIMEOUT, settings::HttpEndpointModifyHeadersSettings,
};

use super::{BuildResult, HttpProxyPassInner, HttpRequestBuilder, ProxyPassError, RetryType};

pub struct HttpProxyPass {
    pub inner: Mutex<HttpProxyPassInner>,
    http_1: bool,
}

impl HttpProxyPass {
    pub fn new(
        socket_addr: SocketAddr,
        modify_headers_settings: HttpEndpointModifyHeadersSettings,
        http_1: bool,
    ) -> Self {
        Self {
            inner: Mutex::new(HttpProxyPassInner::new(
                socket_addr,
                modify_headers_settings,
            )),
            http_1,
        }
    }

    pub async fn update_client_cert_cn_name(&self, client_cert_cn: String) {
        let mut inner = self.inner.lock().await;
        inner.src.client_cert_cn = Some(client_cert_cn);
    }

    pub async fn send_payload(
        &self,
        app: &Arc<AppContext>,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Result<hyper::Response<Full<Bytes>>>, ProxyPassError> {
        println!("Executing send_payload: {}", req.uri());
        let mut req = HttpRequestBuilder::new(self.http_1, req);

        loop {
            let (future1, future2, build_result, request_executor) = {
                let mut inner = self.inner.lock().await;

                if !inner.initialized() {
                    let host_port = req.get_host_port();
                    inner.init(app, &host_port).await?;
                }

                let build_result = req.populate_and_build(&inner).await?;

                let proxy_pass_location =
                    inner.locations.find_mut(build_result.get_location_index());

                proxy_pass_location.connect_if_require(app).await?;

                let (future1, future2, request_executor) = {
                    match &mut proxy_pass_location.content_source {
                        super::HttpProxyPassContentSource::Http(http_content_source) => {
                            if http_content_source.remote_endpoint.is_http1() {
                                let result = http_content_source.send_http1_request(req.get());

                                (Some(result), None, None)
                            } else {
                                let future = http_content_source.send_http2_request(req.get());
                                (None, Some(future), None)
                            }
                        }
                        super::HttpProxyPassContentSource::LocalPath(file) => {
                            let executor = file.get_request_executor(req.uri())?;

                            (None, None, Some(executor))
                        }

                        super::HttpProxyPassContentSource::PathOverSsh(ssh) => {
                            let executor = ssh.get_request_executor(req.uri())?;

                            (None, None, Some(executor))
                        }
                    }
                };

                (future1, future2, build_result, request_executor)
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
                if let Some((content, content_type)) = request_executor.execute_request().await? {
                    let inner = self.inner.lock().await;

                    let result = super::http_response_builder::build_response_from_content(
                        req.uri(),
                        &inner,
                        build_result.get_location_index(),
                        content_type,
                        content,
                    );
                    return Ok(Ok(result));
                } else {
                    let result = hyper::Response::builder()
                        .status(404)
                        .body(http_body_util::Full::new(hyper::body::Bytes::new()))
                        .unwrap();

                    return Ok(Ok(result));
                }
            } else {
                panic!("Both futures are None")
            };

            match build_result {
                BuildResult::HttpRequest(location_index) => match result {
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
                },
                BuildResult::WebSocketUpgrade {
                    location_index: _, //todo!("Handle errors here properly")
                    upgrade_response,
                    web_socket,
                } => {
                    println!("Doing upgrade");

                    match result {
                        Ok(res) => match hyper::upgrade::on(res).await {
                            Ok(upgraded) => {
                                println!("Upgrade Ok");

                                if let Some(web_socket) = web_socket.lock().await.take() {
                                    tokio::spawn(super::web_socket_loop(web_socket, upgraded));
                                }

                                return Ok(Ok(upgrade_response));
                            }
                            Err(e) => {
                                println!("Upgrade Error: {:?}", e);
                                return Err(e.into());
                            }
                        },
                        Err(err) => {
                            println!("Upgrade Request Error: {:?}", err);
                            return Err(err);
                        }
                    }
                }
            }
        }
    }

    pub async fn dispose(&self) {
        let mut inner = self.inner.lock().await;
        inner.disposed = true;
    }
}