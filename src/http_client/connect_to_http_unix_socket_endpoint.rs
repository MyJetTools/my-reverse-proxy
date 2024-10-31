use http_body_util::Full;
use hyper::{
    body::Bytes,
    client::conn::http1::{Builder, SendRequest},
};
use hyper_util::rt::TokioIo;
use tokio::net::UnixSocket;

use super::HttpClientError;

pub async fn connect_to_http_unix_socket_endpoint(
    unix_socket_path: &str,
) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
    let unix_socket = UnixSocket::new_stream()?;
    let connect_result = unix_socket.connect(unix_socket_path).await;

    match connect_result {
        Ok(tcp_stream) => {
            let io = TokioIo::new(tcp_stream);
            let handshake_result = Builder::new()
                .max_buf_size(1024 * 1024 * 5)
                .allow_obsolete_multiline_headers_in_responses(true)
                .allow_spaces_after_header_name_in_responses(true)
                .ignore_invalid_headers_in_responses(true)
                .writev(true)
                .handshake(io)
                .await;
            match handshake_result {
                Ok((mut sender, conn)) => {
                    let unix_socket_path = unix_socket_path.to_string();
                    tokio::task::spawn(async move {
                        if let Err(err) = conn.with_upgrades().await {
                            println!(
                                "Http Connection to {} is failed: {:?}",
                                unix_socket_path, err
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
