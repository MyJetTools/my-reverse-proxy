use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::Full;
use tokio::sync::Mutex;

use crate::{app::AppContext, http_client::HTTP_CLIENT_TIMEOUT};

use super::{HostPort, ProxyPassError, ProxyPassInner, RetryType};

pub struct ProxyPassClient {
    pub inner: Mutex<ProxyPassInner>,
    pub server_addr: SocketAddr,
}

impl ProxyPassClient {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            server_addr: addr,
            inner: Mutex::new(ProxyPassInner::new()),
        }
    }

    pub async fn send_payload(
        &self,
        app: &Arc<AppContext>,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Result<hyper::Response<Full<Bytes>>>, ProxyPassError> {
        let req = into_client_request(req).await?;

        loop {
            let (future1, future2, proxy_pass_id) = {
                let mut inner = self.inner.lock().await;

                if !inner.configurations.has_configurations() {
                    let configurations =
                        crate::flows::get_configurations(app, &HostPort::new(&req)).await?;
                    inner.configurations.init(configurations);
                }

                let proxy_pass_configuration = inner.configurations.find(req.uri())?;

                proxy_pass_configuration.connect_if_require(app).await?;

                let id = proxy_pass_configuration.id;

                let (future1, future2) = if proxy_pass_configuration.remote_endpoint.is_http1() {
                    let result = proxy_pass_configuration.send_http1_request(req.clone());

                    (Some(result), None)
                } else {
                    let future = proxy_pass_configuration.send_http2_request(req.clone());
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

                    let body = read_bytes(incoming).await?;

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

async fn into_client_request(
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<hyper::Request<Full<Bytes>>, ProxyPassError> {
    let (parts, incoming) = req.into_parts();

    let body = read_bytes(incoming).await?;

    Ok(hyper::Request::from_parts(parts, body))
}

async fn read_bytes(
    incoming: impl hyper::body::Body<Data = hyper::body::Bytes, Error = hyper::Error>,
) -> Result<Full<Bytes>, ProxyPassError> {
    use http_body_util::BodyExt;

    let collected = incoming.collect().await?;
    let bytes = collected.to_bytes();

    let body = http_body_util::Full::new(bytes);
    Ok(body)
}
