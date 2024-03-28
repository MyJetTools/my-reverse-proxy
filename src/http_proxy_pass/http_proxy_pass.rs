use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::Full;
use tokio::sync::Mutex;

use crate::{
    app::AppContext, http_client::HTTP_CLIENT_TIMEOUT, settings::HttpEndpointModifyHeadersSettings,
};

use super::{
    BuildResult, GoogleAuthResult, HttpProxyPassInner, HttpRequestBuilder, ProxyPassEndpointInfo,
    ProxyPassError, RetryType,
};

pub struct HttpProxyPass {
    pub inner: Mutex<HttpProxyPassInner>,
    pub endpoint_info: Arc<ProxyPassEndpointInfo>,
    socket_addr: SocketAddr,
}

impl HttpProxyPass {
    pub fn new(
        socket_addr: SocketAddr,
        modify_headers_settings: HttpEndpointModifyHeadersSettings,
        endpoint_info: Arc<ProxyPassEndpointInfo>,
    ) -> Self {
        Self {
            inner: Mutex::new(HttpProxyPassInner::new(
                socket_addr,
                modify_headers_settings,
            )),
            endpoint_info,
            socket_addr,
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
        let mut req = HttpRequestBuilder::new(self.endpoint_info.http_type.clone(), req);

        loop {
            let (future1, future2, build_result, request_executor, dest_http1) = {
                let mut inner = self.inner.lock().await;

                if !inner.initialized() {
                    inner.init(app, &self.endpoint_info, &req).await?;
                }

                match self.handle_auth_with_g_auth(app, &req).await {
                    GoogleAuthResult::Passed(user) => req.g_auth_user = user,
                    GoogleAuthResult::Content(content) => return Ok(content),
                    GoogleAuthResult::DomainIsNotAuthorized => {
                        return Err(ProxyPassError::Unauthorized);
                    }
                }

                let build_result = req.populate_and_build(&inner).await?;

                let proxy_pass_location =
                    inner.locations.find_mut(build_result.get_location_index());

                if !proxy_pass_location
                    .whitelisted_ip
                    .is_whitelisted(&self.socket_addr.ip())
                {
                    return Err(ProxyPassError::IpRestricted(
                        self.socket_addr.ip().to_string(),
                    ));
                }

                proxy_pass_location.connect_if_require(app).await?;

                let (future1, future2, request_executor, is_http_1) = {
                    match &mut proxy_pass_location.content_source {
                        super::HttpProxyPassContentSource::Http(http_content_source) => {
                            if http_content_source.remote_endpoint.is_http1() {
                                let result = http_content_source.send_http1_request(req.get());

                                (Some(result), None, None, Some(true))
                            } else {
                                let future = http_content_source.send_http2_request(req.get());
                                (None, Some(future), None, Some(false))
                            }
                        }
                        super::HttpProxyPassContentSource::LocalPath(file) => {
                            let executor = file.get_request_executor(req.uri())?;

                            (None, None, Some(executor), None)
                        }

                        super::HttpProxyPassContentSource::PathOverSsh(ssh) => {
                            let executor = ssh.get_request_executor(req.uri())?;

                            (None, None, Some(executor), None)
                        }

                        super::HttpProxyPassContentSource::Static(static_content_src) => {
                            let static_content_src = static_content_src.get_request_executor()?;
                            (None, None, Some(static_content_src), None)
                        }
                    }
                };

                (future1, future2, build_result, request_executor, is_http_1)
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
                let response = request_executor.execute_request().await?;

                let inner = self.inner.lock().await;

                let result = super::http_response_builder::build_response_from_content(
                    &req,
                    &inner,
                    build_result.get_location_index(),
                    response.content_type,
                    response.status_code,
                    response.body,
                    req.g_auth_user.as_ref(),
                );
                return Ok(Ok(result));
            } else {
                panic!("Both futures are None")
            };

            match build_result {
                BuildResult::HttpRequest(location_index) => match result {
                    Ok(response) => {
                        let inner = self.inner.lock().await;
                        let response = super::http_response_builder::build_http_response(
                            &req,
                            response,
                            &inner,
                            &location_index,
                            self.endpoint_info.http_type,
                            dest_http1.unwrap(),
                            req.g_auth_user.as_ref(),
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
                    if self.endpoint_info.debug {
                        println!("Doing web_socket upgrade");
                    }

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
                                if self.endpoint_info.debug {
                                    println!("Upgrade Error: {:?}", e);
                                }

                                return Err(e.into());
                            }
                        },
                        Err(err) => {
                            if self.endpoint_info.debug {
                                println!("Upgrade Request Error: {:?}", err);
                            }

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
