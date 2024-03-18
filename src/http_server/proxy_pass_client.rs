use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::Full;
use tokio::sync::Mutex;

use crate::{app::AppContext, http_client::TIMEOUT};

use super::{ProxyPassError, ProxyPassInner};

pub struct ProxyPassClient {
    pub inner: Mutex<ProxyPassInner>,
    pub server_addr: SocketAddr,
}

impl ProxyPassClient {
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            server_addr: addr,
            inner: Mutex::new(ProxyPassInner::Unknown),
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

                let proxy_pass_configuration =
                    inner.get_proxy_pass_configuration(app, &req).await?;

                proxy_pass_configuration.connect_if_require(app).await?;

                let id = proxy_pass_configuration.id;

                let result = proxy_pass_configuration.send_request(req.clone());

                (result, id)
            };

            let result = tokio::time::timeout(TIMEOUT, future).await;

            if result.is_err() {
                return Err(ProxyPassError::Timeout);
            }

            match result.unwrap() {
                Ok(response) => {
                    let (parts, incoming) = response.into_parts();

                    let body = read_bytes(incoming).await?;

                    let response = hyper::Response::from_parts(parts, body);

                    return Ok(Ok(response));
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    let mut inner = self.inner.lock().await;
                    if inner.handle_error(&err, proxy_pass_id).await? {
                        return Ok(Err(err.into()));
                    }
                }
            }
        }
    }

    pub async fn dispose(&self) {
        let mut inner = self.inner.lock().await;
        *inner = ProxyPassInner::Disposed;
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
