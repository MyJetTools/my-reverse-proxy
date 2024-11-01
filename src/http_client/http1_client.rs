use std::sync::Arc;

use my_tls::tokio_rustls::client::TlsStream;
use tokio::net::TcpStream;

use crate::my_http_client::MyHttpClient;

use crate::configurations::*;

use super::HttpClientError;

pub enum Http1Client {
    Http(MyHttpClient<TcpStream>),
    Https(MyHttpClient<TlsStream<tokio::net::TcpStream>>),
}

impl Http1Client {
    pub async fn connect(
        remote_host: &RemoteHost,
        domain_name: &Option<String>,
        debug: bool,
    ) -> Result<Self, HttpClientError> {
        if remote_host.is_https() {
            let tls_stream = connect_to_tls_endpoint(remote_host, domain_name, debug).await?;
            return Ok(Http1Client::Https(MyHttpClient::new(tls_stream)));
        }

        //println!("Connecting to remote host: {:?}", remote_host);
        let tcp_stream: TcpStream = TcpStream::connect(remote_host.get_host_port()).await?;
        //println!("Connected to remote host: {:?}", remote_host);
        let result = Self::Http(MyHttpClient::new(tcp_stream));

        Ok(result)
    }

    /*
       async fn connect_over_ssh_with_tunnel(
           app: &AppContext,
           ssh_credentials: &Arc<SshCredentials>,
           remote_host: &RemoteHost,
       ) -> Result<Self, ProxyPassError> {
           let (send_request, port_forward) =
               super::connect_to_http_over_ssh_with_tunnel(app, ssh_credentials, remote_host).await?;

           let result = Self {
               send_request,

               _port_forward: Some(port_forward),
           };

           Ok(result)
       }

    async fn connect_to_http(
        remote_host: &RemoteHost,
        domain_name: &Option<String>,
        debug: bool,
    ) -> Result<SendRequest<Full<Bytes>>, HttpClientError> {
        if remote_host.is_https() {
            let future = super::connect_to_tls_endpoint(remote_host, domain_name, debug);

            let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

            if result.is_err() {
                return Err(HttpClientError::TimeOut);
            }

            result.unwrap()
        } else {
            let future = super::connect_to_http_endpoint(remote_host);

            let result = tokio::time::timeout(HTTP_CLIENT_TIMEOUT, future).await;

            if result.is_err() {
                return Err(HttpClientError::TimeOut);
            }

            result.unwrap()
        }
    }
        */
}

pub async fn connect_to_tls_endpoint(
    remote_host: &RemoteHost,
    domain_name: &Option<String>,
    debug: bool,
) -> Result<TlsStream<tokio::net::TcpStream>, HttpClientError> {
    use my_tls::tokio_rustls::rustls::pki_types::ServerName;

    let host_port = remote_host.get_host_port();

    let tcp_stream = if host_port.find(":").is_none() {
        TcpStream::connect(format!("{}:443", remote_host.get_host_port())).await?
    } else {
        TcpStream::connect(host_port).await?
    };

    if debug {
        println!(
            "Connecting to TLS remote host: {}",
            remote_host.get_host_port(),
        );
    }

    let config = my_tls::tokio_rustls::rustls::ClientConfig::builder()
        .with_root_certificates(my_tls::ROOT_CERT_STORE.clone())
        .with_no_client_auth();

    let connector = my_tls::tokio_rustls::TlsConnector::from(Arc::new(config));
    let domain = if let Some(domain_name) = domain_name {
        ServerName::try_from(domain_name.to_string()).unwrap()
    } else {
        ServerName::try_from(remote_host.get_host().to_string()).unwrap()
    };

    if debug {
        println!("TLS Domain Name: {:?}", domain);
    }

    let tls_stream = connector
        .connect_with(domain, tcp_stream, |itm| {
            if debug {
                println!("Debugging: {:?}", itm.alpn_protocol());
            }
        })
        .await?;

    return Ok(tls_stream);

    /*

        let io = TokioIo::new(tls_stream);

        let handshake_result = hyper::client::conn::http1::handshake(io).await;

        match handshake_result {
            Ok((mut sender, conn)) => {
                let host_port = remote_host.to_string();
                tokio::task::spawn(async move {
                    if debug {
                        println!("Connected to TLS remote host: {}", host_port,);
                    }

                    if let Err(err) = conn.await {
                        if debug {
                            println!(
                                "Https Connection to https://{} is failed: {:?}",
                                host_port, err
                            );
                        }
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
     */
}
