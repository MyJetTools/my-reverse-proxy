use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::client::conn::http1::SendRequest;
use hyper_util::rt::TokioIo;
use my_tls::ROOT_CERT_STORE;
use tokio::net::TcpStream;

use crate::settings::RemoteHost;

use super::HttpClientError;

pub async fn connect_to_tls_endpoint(
    remote_host: &RemoteHost,
    domain_name: &Option<String>,
) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
    use tokio_rustls::rustls::pki_types::ServerName;

    let connect_result = TcpStream::connect(remote_host.get_host_port()).await;

    println!(
        "Connecting to TLS remote host: {}",
        remote_host.get_host_port(),
    );
    match connect_result {
        Ok(tcp_stream) => {
            let config = tokio_rustls::rustls::ClientConfig::builder()
                .with_root_certificates(ROOT_CERT_STORE.clone())
                .with_no_client_auth();

            let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
            let domain = if let Some(domain_name) = domain_name {
                ServerName::try_from(domain_name.to_string()).unwrap()
            } else {
                ServerName::try_from(remote_host.get_host().to_string()).unwrap()
            };

            println!("TLS Domain Name: {:?}", domain);

            let tls_stream = connector
                .connect_with(domain, tcp_stream, |itm| {
                    println!("Debugging: {:?}", itm.alpn_protocol());
                })
                .await?;

            let io = TokioIo::new(tls_stream);

            let handshake_result = hyper::client::conn::http1::handshake(io).await;

            match handshake_result {
                Ok((mut sender, conn)) => {
                    let host_port = remote_host.to_string();
                    tokio::task::spawn(async move {
                        println!("Connected to TLS remote host: {}", host_port,);
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
                    println!(
                        "Can not connect to TLS remote host: {}. Err: {}",
                        remote_host.get_host_port(),
                        err
                    );
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
