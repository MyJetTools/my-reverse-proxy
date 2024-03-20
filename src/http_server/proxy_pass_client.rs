use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http_body_util::Full;
use tokio::sync::Mutex;

use crate::{app::AppContext, http_client::HTTP_CLIENT_TIMEOUT};

use super::{HostPort, ProxyPassError, ProxyPassInner};

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
                    let future = proxy_pass_configuration.send_http1_request(req.clone());
                    (Some(future), None)
                } else {
                    let future = proxy_pass_configuration.send_http2_request(req.clone());
                    (None, Some(future))
                };

                (future1, future2, id)
            };

            let result = if let Some(future1) = future1 {
                let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future1).await;

                if result.is_err() {
                    return Err(ProxyPassError::Timeout);
                }

                result.unwrap()
            } else if let Some(future2) = future2 {
                let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future2).await;

                if result.is_err() {
                    return Err(ProxyPassError::Timeout);
                }
                result.unwrap()
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
                    println!("Error: {:?}", err);
                    let mut inner = self.inner.lock().await;
                    if inner.handle_error(&err, proxy_pass_id).await? {
                        return Ok(Err(err.into()));
                    }
                }
            }
        }
    }

    /*
       pub async fn send_payload_http1(
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

                   let result = proxy_pass_configuration.send_http1_request(req.clone());

                   (result, id)
               };

               let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

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

       pub async fn send_payload_http2(
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

                   let result = proxy_pass_configuration.send_http2_request(req.clone());

                   (result, id)
               };

               let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

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
    */
    pub async fn dispose(&self) {
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
