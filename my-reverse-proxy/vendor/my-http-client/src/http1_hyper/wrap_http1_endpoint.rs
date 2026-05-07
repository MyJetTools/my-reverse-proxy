use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http1::SendRequest;
use hyper_util::rt::TokioIo;

use crate::MyHttpClientError;

use super::MyHttpHyperClientInner;

pub async fn wrap_http1_endpoint<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
>(
    stream: TStream,
    remote_host: &str,
    inner: Arc<MyHttpHyperClientInner>,
    connection_id: u64,
) -> Result<SendRequest<Full<Bytes>>, MyHttpClientError> {
    let io = TokioIo::new(stream);
    let handshake_result = hyper::client::conn::http1::handshake(io).await;
    match handshake_result {
        Ok((mut sender, conn)) => {
            crate::spawn_named("myhttp_h1_hyper_conn_driver", async move {
                let _ = conn.with_upgrades().await;
                inner.disconnect(connection_id).await;
            });

            let result = sender.ready().await;

            if let Err(err) = result {
                return Err(MyHttpClientError::CanNotConnectToRemoteHost(format!(
                    "Can not establish Http connection to '{remote_host}'. Http handshake Error: {}",
                    err
                )));
            }

            Ok(sender)
        }
        Err(err) => {
            Err(MyHttpClientError::InvalidHttpHandshake(format!("{}", err)))
        }
    }
}
