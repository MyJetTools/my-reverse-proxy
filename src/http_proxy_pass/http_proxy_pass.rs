use std::sync::Arc;

use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use tokio::sync::Mutex;

use crate::{app::AppContext, configurations::*, http_server::ClientCertificateData};

use super::{
    GoogleAuthResult, HttpProxyPassIdentity, HttpProxyPassInner, HttpRequestBuilder,
    ProxyPassError, ProxyPassLocations,
};

pub struct HttpProxyPass {
    pub inner: Mutex<HttpProxyPassInner>,
    pub listening_port_info: HttpListenPortInfo,
    pub endpoint_info: Arc<HttpEndpointInfo>,
}

impl HttpProxyPass {
    pub fn new(
        endpoint_info: Arc<HttpEndpointInfo>,
        listening_port_info: HttpListenPortInfo,
        client_cert: Option<ClientCertificateData>,
    ) -> Self {
        let locations = ProxyPassLocations::new(&endpoint_info);
        Self {
            inner: Mutex::new(HttpProxyPassInner::new(
                HttpProxyPassIdentity::new(client_cert),
                locations,
                listening_port_info.clone(),
            )),

            listening_port_info,
            endpoint_info,
        }
    }

    pub async fn send_payload(
        &self,
        app: &Arc<AppContext>,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Result<hyper::Response<BoxBody<Bytes, String>>>, ProxyPassError> {
        if self.endpoint_info.debug {
            println!(
                "Request: {}. Uri: {}. Headers{:?}",
                self.endpoint_info.host_endpoint.as_str(),
                req.uri(),
                req.headers()
            );
        }

        let mut req = HttpRequestBuilder::new(self.endpoint_info.http_type.clone(), req);

        let (build_result, content_source) = {
            let mut inner = self.inner.lock().await;

            match self.handle_auth_with_g_auth(app, &req).await {
                GoogleAuthResult::Passed(user) => inner.identity.ga_user = user,
                GoogleAuthResult::Content(content) => return Ok(content),
                GoogleAuthResult::DomainIsNotAuthorized => {
                    return Err(ProxyPassError::Unauthorized);
                }
            }

            if let Some(allowed_users) = self.endpoint_info.allowed_user_list.as_ref() {
                if let Some(identity) = inner.identity.get_identity() {
                    if !allowed_users.is_allowed(identity) {
                        return Err(ProxyPassError::UserIsForbidden);
                    }
                }
            }

            let build_result = req.populate_and_build(self, &inner).await?;

            let proxy_pass_location = inner.locations.find_mut(build_result.get_location_index());

            if !proxy_pass_location
                .config
                .whitelisted_ip
                .is_whitelisted(&self.listening_port_info.socket_addr.ip())
            {
                return Err(ProxyPassError::IpRestricted(
                    self.listening_port_info.socket_addr.ip().to_string(),
                ));
            }

            (
                build_result,
                proxy_pass_location.connect_if_require(app).await?,
            )
        };

        let request = req.get();

        let (location_index, mut response) = match build_result {
            super::BuildResult::HttpRequest(location_index) => {
                (location_index, content_source.send_request(request).await?)
            }
            super::BuildResult::WebSocketUpgrade {
                location_index,
                upgrade_response,
                web_socket,
            } => {
                if self.endpoint_info.debug {
                    println!("Doing web_socket upgrade");
                }

                let (channel, _) = content_source.upgrade_websocket(request.clone()).await?;

                // let upgraded = hyper::upgrade::on(request).await?;

                if let Some(web_socket) = web_socket.lock().await.take() {
                    match channel {
                        super::WebSocketUpgradeStream::TcpStream(tcp_stream) => {
                            tokio::spawn(super::web_socket_loop(
                                web_socket,
                                tcp_stream,
                                self.endpoint_info.debug,
                            ));
                        }

                        super::WebSocketUpgradeStream::TlsStream(tcp_stream) => {
                            tokio::spawn(super::web_socket_loop(
                                web_socket,
                                tcp_stream,
                                self.endpoint_info.debug,
                            ));
                        }
                        super::WebSocketUpgradeStream::SshChannel(async_channel) => {
                            tokio::spawn(super::web_socket_loop(
                                web_socket,
                                async_channel,
                                self.endpoint_info.debug,
                            ));
                        }
                    }
                }

                let response = crate::utils::into_full_body_response(upgrade_response);
                (location_index, response)
            }
        };

        let inner = self.inner.lock().await;
        super::http_response_builder::modify_resp_headers(
            self,
            &inner,
            &req,
            response.headers_mut(),
            &location_index,
        );

        return Ok(Ok(response));

        /*
        let result = if let Some(future_http1) = future_http1 {
            match future_http1 {
                Ok(future_http1) => {
                    let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future_http1).await;

                    match super::error::handle_error(result, attempt_no).await {
                        Ok(result) => Ok(result),
                        Err(err) => match err {
                            super::ExecuteWithTimeoutError::ReconnectAndRetry => {
                                let mut inner = self.inner.lock().await;

                                let proxy_pass_location =
                                    inner.locations.find_mut(build_result.get_location_index());

                                if let Err(err) = proxy_pass_location
                                    .reconnect(&app, self.endpoint_info.debug)
                                    .await
                                {
                                    return Err(err);
                                }
                                continue;
                            }
                            super::ExecuteWithTimeoutError::ProxyPassError(
                                proxy_pass_error,
                            ) => {
                                return Err(proxy_pass_error);
                            }
                        },
                    }
                }
                Err(err) => Err(err),
            }
        } else if let Some(future_http2) = future_http2 {
            match future_http2 {
                Ok(future_http2) => {
                    let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future_http2).await;

                    match super::error::handle_error(result, attempt_no).await {
                        Ok(result) => Ok(result),
                        Err(err) => match err {
                            super::ExecuteWithTimeoutError::ReconnectAndRetry => {
                                let mut inner = self.inner.lock().await;

                                let proxy_pass_location =
                                    inner.locations.find_mut(build_result.get_location_index());

                                if let Err(err) = proxy_pass_location
                                    .reconnect(&app, self.endpoint_info.debug)
                                    .await
                                {
                                    return Err(err);
                                }
                                continue;
                            }
                            super::ExecuteWithTimeoutError::ProxyPassError(
                                proxy_pass_error,
                            ) => {
                                return Err(proxy_pass_error);
                            }
                        },
                    }
                }
                Err(err) => Err(err),
            }
        } else if let Some(request_executor) = request_executor {
            let response = request_executor.execute_request().await?;

            let inner = self.inner.lock().await;

            let result = super::http_response_builder::build_response_from_content(
                self,
                &inner,
                &req,
                build_result.get_location_index(),
                response.content_type,
                response.status_code,
                response.body,
            );
            return Ok(Ok(result));
        } else {
            panic!("Both futures are None")
        };

        match build_result {
            BuildResult::HttpRequest(location_index) => match result {
                Ok(response) => {
                    /*
                                           let mut chunked_response = false;
                                           if let Some(value) = response.headers().get("Transfer-Encoding") {
                                               chunked_response = value == "chunked";

                                               println!("Chunked response found");
                                           }
                    */
                    let inner = self.inner.lock().await;

                    /*
                    if chunked_response {
                        let response =
                            super::http_response_builder::build_chunked_http_response(
                                self,
                                &inner,
                                &req,
                                response,
                                &location_index,
                            )
                            .await?;
                        return Ok(Ok(response));
                    }
                     */

                    let response = super::http_response_builder::build_http_response(
                        self,
                        &inner,
                        &req,
                        response,
                        &location_index,
                        dest_http1.unwrap(),
                    )
                    .await?;
                    return Ok(Ok(response));
                }
                Err(err) => {
                    let retry = {
                        self.handle_error(app, &err, &location_index, self.endpoint_info.debug)
                            .await?
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
                            if self.endpoint_info.debug {
                                println!("Upgrade Ok");
                            }

                            if let Some(web_socket) = web_socket.lock().await.take() {
                                tokio::spawn(super::web_socket_loop(
                                    web_socket,
                                    upgraded,
                                    self.endpoint_info.debug,
                                ));
                            }

                            let (parts, body) = upgrade_response.into_parts();

                            return Ok(Ok(hyper::Response::from_parts(
                                parts,
                                body.map_err(|e| crate::to_hyper_error(e)).boxed(),
                            )));
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
             */
    }

    pub async fn dispose(&self) {
        let mut inner = self.inner.lock().await;
        inner.disposed = true;
    }
    /*
    pub async fn handle_error(
        &self,
        app: &AppContext,
        err: &ProxyPassError,
        location_index: &LocationIndex,
        debug: bool,
    ) -> Result<RetryType, ProxyPassError> {

        let mut do_retry = RetryType::NoRetry;

        if err.is_disposed() {
            if debug {
                println!(
                    "ProxyPassInner::handle_error. Connection with id {} and index {} is disposed. Trying to reconnect",
                    location_index.id,
                    location_index.index
                );
            }
            let mut inner = self.inner.lock().await;
            let location = inner.locations.find_mut(location_index);
            location.connect_if_require(app, debug).await?;
            return Ok(RetryType::Retry(None));
        }

        if let ProxyPassError::HyperError(err) = err {
            if err.is_canceled() {
                let mut inner = self.inner.lock().await;
                let location = inner.locations.find_mut(location_index);

                match &mut location.content_source {
                    HttpProxyPassContentSource::Http(remote_http_content_source) => {
                        let mut dispose_connection = false;

                        if let Some(connected_moment) =
                            remote_http_content_source.get_connected_moment()
                        {
                            let now = DateTimeAsMicroseconds::now();

                            if now.duration_since(connected_moment).as_positive_or_zero()
                                > OLD_CONNECTION_DELAY
                            {
                                dispose_connection = true;
                                do_retry = RetryType::Retry(None);
                            } else {
                                do_retry =
                                    RetryType::Retry(NEW_CONNECTION_NOT_READY_RETRY_DELAY.into());
                            }
                        }

                        if dispose_connection {
                            remote_http_content_source.dispose();
                            remote_http_content_source
                                .connect_if_require(app, &location.config.domain_name, debug)
                                .await?;
                        }
                    }
                    HttpProxyPassContentSource::LocalPath(_) => {}
                    HttpProxyPassContentSource::PathOverSsh(_) => {}
                    HttpProxyPassContentSource::Static(_) => {}
                }
            }
        }

        Ok(do_retry)



    }

             */
}
