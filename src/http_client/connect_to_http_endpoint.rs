use http_body_util::Full;
use hyper::{body::Bytes, client::conn::http1::SendRequest, Uri};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

use super::HttpClientError;

pub async fn connect_to_http_endpoint(
    uri: &Uri,
) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
    let host_port = super::utils::get_host_port(uri);

    let connect_result = TcpStream::connect(host_port.as_str()).await;

    match connect_result {
        Ok(tcp_stream) => {
            let io = TokioIo::new(tcp_stream);
            let handshake_result = hyper::client::conn::http1::handshake(io).await;
            match handshake_result {
                Ok((mut sender, conn)) => {
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
