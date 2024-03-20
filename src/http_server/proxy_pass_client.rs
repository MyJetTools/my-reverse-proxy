use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::Full;
use tokio::sync::Mutex;

use crate::{app::AppContext, http_client::HTTP_CLIENT_TIMEOUT};

use super::{into_full_bytes, HttpContentBuilder, ProxyPassError, ProxyPassInner, RetryType};

pub struct ProxyPassClient {
    pub inner: Mutex<ProxyPassInner>,
}

impl ProxyPassClient {
    pub fn new(socket_addr: SocketAddr) -> Self {
        Self {
            inner: Mutex::new(ProxyPassInner::new(socket_addr)),
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
        let mut req = HttpContentBuilder::new(req);
        loop {
            let (future1, future2, proxy_pass_id) = {
                let mut inner = self.inner.lock().await;

                if !inner.configurations.has_configurations() {
                    let host_port = req.get_host_port();
                    let configurations = crate::flows::get_configurations(app, &host_port).await?;
                    inner.configurations.init(configurations);
                    inner.update_src_info(&host_port);
                }

                req.populate_and_build(&inner).await?;

                let proxy_pass_configuration = inner.configurations.find(req.uri())?;

                proxy_pass_configuration.connect_if_require(app).await?;

                let id = proxy_pass_configuration.id;

                let (future1, future2) = if proxy_pass_configuration.remote_endpoint.is_http1() {
                    let result = proxy_pass_configuration.send_http1_request(req.get());

                    (Some(result), None)
                } else {
                    let future = proxy_pass_configuration.send_http2_request(req.get());
                    (None, Some(future))
                };

                (future1, future2, id)
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
            } else {
                panic!("Both futures are None")
            };

            match result {
                Ok(response) => {
                    let (parts, incoming) = response.into_parts();
                    let body = into_full_bytes(incoming).await?;
                    let response = hyper::Response::from_parts(parts, body);
                    return Ok(Ok(response));
                }
                Err(err) => {
                    let retry = {
                        let mut inner = self.inner.lock().await;
                        inner.handle_error(app, &err, proxy_pass_id).await?
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
