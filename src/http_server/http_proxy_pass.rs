use std::{net::SocketAddr, sync::Arc};

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::sync::Mutex;

use crate::{
    app::AppContext,
    http_client::{HttpClient, HttpClientConnection},
};

use super::{HttpProxyPassInner, ProxyPassError};

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

        let (future, proxy_pass_id) = {
            let mut inner = self.inner.lock().await;

            if inner.len() == 0 {
                let host = req.headers().get("host");
                if host.is_none() {
                    return Err(ProxyPassError::NoConfigurationFound);
                }

                let locations = app
                    .settings_reader
                    .get_configurations(host.unwrap().to_str().unwrap())
                    .await;

                if locations.len() == 0 {
                    return Err(ProxyPassError::NoConfigurationFound);
                }

                let mut id = DateTimeAsMicroseconds::now().unix_microseconds;
                for (location, uri) in locations {
                    inner.push(HttpProxyPassInner::new(location, uri, id));
                    id += 1;
                }
            }

            let mut found_proxy_pass = None;

            for proxy_pass in inner.iter_mut() {
                if proxy_pass.is_my_uri(req.uri()) {
                    found_proxy_pass = Some(proxy_pass);
                    break;
                }
            }

            if found_proxy_pass.is_none() {
                return Err(ProxyPassError::NoLocationFound);
            }

            let found_proxy_pass = found_proxy_pass.unwrap();
            let id = found_proxy_pass.id;

            if found_proxy_pass.http_client.connection.is_none() {
                let connection = HttpClient::connect(&found_proxy_pass.proxy_pass_uri).await?;

                found_proxy_pass.http_client.connection =
                    Some(HttpClientConnection::new(connection));
            }

            let result = found_proxy_pass
                .http_client
                .connection
                .as_mut()
                .unwrap()
                .send_request
                .send_request(req);

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
                for proxy_pass in inner.iter_mut() {
                    if proxy_pass.id == proxy_pass_id {
                        proxy_pass.http_client.connection = None;
                    }
                }
                return Ok(Err(err.into()));
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
