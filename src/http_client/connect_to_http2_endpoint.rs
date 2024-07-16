use http_body_util::Full;
use hyper::{body::Bytes, client::conn::http2::SendRequest};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::TcpStream;

use crate::configurations::*;

use super::HttpClientError;

pub async fn connect_to_http2_endpoint(
    remote_host: &RemoteHost,
) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
    let connect_result = TcpStream::connect(remote_host.get_host_port()).await;

    match connect_result {
        Ok(tcp_stream) => {
            let io = TokioIo::new(tcp_stream);
            let handshake_result =
                hyper::client::conn::http2::handshake(TokioExecutor::new(), io).await;
            match handshake_result {
                Ok((mut sender, conn)) => {
                    let host_port = remote_host.to_string();
                    tokio::task::spawn(async move {
                        if let Err(err) = conn.await {
                            println!(
                                "Http Connection to http://{} is failed: {:?}",
                                host_port, err
                            );
                        }

                        //Here
                    });

                    sender.ready().await?;
                    return Ok(sender);
                }
                Err(err) => {
                    return Err(HttpClientError::InvalidHttp1HandShake(format!("{}", err)));
                }
            }
        }
        Err(err) => {
            return Err(HttpClientError::CanNotEstablishConnection(format!(
                "{}",
                err
            )));
        }
    }
}
