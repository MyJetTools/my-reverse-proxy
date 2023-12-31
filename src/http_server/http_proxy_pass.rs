use std::{net::SocketAddr, sync::Arc, time::Duration};

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::sync::Mutex;

use crate::app::AppContext;

use super::{HttpProxyPassInner, ProxyPassError};

pub enum RetryType {
    Retry(Option<Duration>),
    NoRetry,
}

pub struct HttpProxyPass {
    pub inner: Mutex<Vec<HttpProxyPassInner>>,
    pub server_addr: SocketAddr,
}

impl HttpProxyPass {
    pub fn new(server_addr: SocketAddr) -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
            server_addr,
        }
    }

    pub async fn send_payload(
        &self,
        app: &Arc<AppContext>,
        req: hyper::Request<hyper::body::Incoming>,
    ) -> Result<hyper::Result<hyper::Response<Full<Bytes>>>, ProxyPassError> {
        let req = into_client_request(req).await?;

        loop {
            let (future, proxy_pass_id) = {
                let mut inner = self.inner.lock().await;

                if inner.len() == 0 {
                    let host = req.headers().get("host");
                    if host.is_none() {
                        return Err(ProxyPassError::NoHostHeaderFound);
                    }

                    crate::flows::populate_configurations(
                        app,
                        host.unwrap().to_str().unwrap(),
                        &mut inner,
                    )
                    .await?;
                }

                let proxy_pass =
                    crate::flows::find_proxy_pass_by_uri(&mut inner, req.uri()).await?;
                let id = proxy_pass.id;

                let result = proxy_pass
                    .http_client
                    .connection
                    .as_mut()
                    .unwrap()
                    .send_request
                    .send_request(req.clone());

                (result, id)
            };

            let result = future.await;

            match result {
                Ok(response) => {
                    let (parts, incoming) = response.into_parts();

                    let body = read_bytes(incoming).await?;

                    let response = hyper::Response::from_parts(parts, body);
                    return Ok(Ok(response));
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    let mut inner = self.inner.lock().await;

                    let mut do_retry = RetryType::NoRetry;

                    if err.is_canceled() {
                        let mut found_proxy_pass = None;
                        for proxy_pass in inner.iter_mut() {
                            if proxy_pass.id == proxy_pass_id {
                                found_proxy_pass = Some(proxy_pass);
                                break;
                            }
                        }

                        if let Some(found_proxy_pass) = found_proxy_pass {
                            let mut dispose_connection = false;

                            if let Some(connection) = &found_proxy_pass.http_client.connection {
                                let now = DateTimeAsMicroseconds::now();

                                if now
                                    .duration_since(connection.connected)
                                    .as_positive_or_zero()
                                    > Duration::from_secs(10)
                                {
                                    dispose_connection = true;
                                    do_retry = RetryType::Retry(None);
                                } else {
                                    do_retry = RetryType::Retry(Duration::from_millis(50).into());
                                }
                            }

                            if dispose_connection {
                                found_proxy_pass.http_client.connection = None;
                            }
                        }
                    }

                    match do_retry {
                        RetryType::Retry(delay) => {
                            if let Some(delay) = delay {
                                tokio::time::sleep(delay).await;
                            }
                        }
                        RetryType::NoRetry => {
                            return Ok(Err(err.into()));
                        }
                    }
                }
            }
        }
    }
}

async fn read_bytes(
    incoming: impl hyper::body::Body<Data = hyper::body::Bytes, Error = hyper::Error>,
) -> Result<Full<Bytes>, ProxyPassError> {
    let collected = incoming.collect().await?;
    let bytes = collected.to_bytes();

    let body = http_body_util::Full::new(bytes);
    Ok(body)
}

async fn into_client_request(
    req: hyper::Request<hyper::body::Incoming>,
) -> Result<hyper::Request<Full<Bytes>>, ProxyPassError> {
    let (parts, incoming) = req.into_parts();

    let body = read_bytes(incoming).await?;

    Ok(hyper::Request::from_parts(parts, body))
}
