use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http2::SendRequest;
use hyper_util::rt::{TokioExecutor, TokioIo};

use crate::MyHttpClientError;

use super::MyHttp2ClientInner;

pub async fn wrap_http2_endpoint<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync + 'static,
>(
    stream: TStream,
    remote_host: &str,
    inner: Arc<MyHttp2ClientInner>,
    connection_id: u64,
) -> Result<SendRequest<Full<Bytes>>, MyHttpClientError> {
    let io = TokioIo::new(stream);

    let handshake_result = hyper::client::conn::http2::handshake(TokioExecutor::new(), io).await;

    match handshake_result {
        Ok((mut sender, conn)) => {
            crate::spawn_named("myhttp_h2_hyper_conn_driver", async move {
                let _ = conn.await;
                inner.disconnect(connection_id).await;
            });

            if let Err(err) = sender.ready().await {
                return Err(MyHttpClientError::CanNotConnectToRemoteHost(format!(
                    "Can not establish Http2 connection to '{remote_host}'. Reading awaiting is finished with {}",
                    err
                )));
            }

            Ok(sender)
        }
        Err(err) => {
            Err(MyHttpClientError::CanNotConnectToRemoteHost(format!(
                "Can not establish Http2 connection to '{remote_host}'. Http2 handshake Error: {}",
                err
            )))
        }
    }
}
