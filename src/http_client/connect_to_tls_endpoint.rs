use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::{client::conn::http1::SendRequest, Uri};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;

use tokio_rustls::{rustls, TlsConnector};

use super::{cert_content::ROOT_CERT_STORE, HttpClientError};

pub async fn connect_to_tls_endpoint(
    uri: &Uri,
) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
    let host_port = super::utils::get_host_port(uri);
    let domain = uri.host().unwrap();

    let connect_result = TcpStream::connect(host_port.as_str()).await;

    match connect_result {
        Ok(tcp_stream) => {
            let config = rustls::ClientConfig::builder()
                .with_safe_defaults()
                .with_root_certificates(ROOT_CERT_STORE.clone())
                .with_no_client_auth();

            let connector = TlsConnector::from(Arc::new(config));

            let domain = rustls::ServerName::try_from(domain).unwrap();

            let tls_stream = connector.connect(domain, tcp_stream).await?;

            let io = TokioIo::new(tls_stream);

            let handshake_result = hyper::client::conn::http1::handshake(io).await;

            match handshake_result {
                Ok((mut sender, conn)) => {
                    let host_port = host_port.to_owned();
                    tokio::task::spawn(async move {
                        if let Err(err) = conn.await {
                            println!(
                                "Https Connection to https://{} is failed: {:?}",
                                host_port, err
                            );
                        }
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
