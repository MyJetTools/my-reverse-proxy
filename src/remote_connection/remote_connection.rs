use std::{sync::Arc, time::Duration};

use tokio::sync::Mutex;

use crate::configurations::HttpEndpointInfo;
use crate::h1_server::from_remote_to_server_loop;

use crate::{
    h1_utils::*, local_path::LocalPathContent, network_stream::*,
    remote_connection::Http1Connection, settings::ProxyPassTo,
    tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream,
};

pub enum RemoteConnection {
    Http1Direct(
        Http1Connection<
            tokio::io::WriteHalf<tokio::net::TcpStream>,
            tokio::io::ReadHalf<tokio::net::TcpStream>,
        >,
    ),

    Http1UnixSocket(
        Http1Connection<
            tokio::io::WriteHalf<tokio::net::UnixStream>,
            tokio::io::ReadHalf<tokio::net::UnixStream>,
        >,
    ),
    Https1Direct(
        Http1Connection<
            tokio::io::WriteHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
            tokio::io::ReadHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
        >,
    ),
    Http1OverSsh(
        Http1Connection<
            tokio::io::WriteHalf<my_ssh::SshAsyncChannel>,
            tokio::io::ReadHalf<my_ssh::SshAsyncChannel>,
        >,
    ),
    Http1OverGateway(
        Http1Connection<
            tokio::io::WriteHalf<TcpGatewayProxyForwardStream>,
            tokio::io::ReadHalf<TcpGatewayProxyForwardStream>,
        >,
    ),
    LocalFiles(LocalPathContent),
}

impl RemoteConnection {
    pub async fn connect<ServerWritePart: NetworkStreamWritePart + Send + Sync + 'static>(
        proxy_pass_to: &ProxyPassTo,
        server_write_part: &Arc<Mutex<ServerWritePart>>,
        end_point_info: Arc<HttpEndpointInfo>,
    ) -> Result<Self, String> {
        match proxy_pass_to {
            ProxyPassTo::Http1(proxy_pass_to) => match &proxy_pass_to.remote_host {
                crate::configurations::MyReverseProxyRemoteEndpoint::Gateway {
                    id,
                    remote_host,
                } => {
                    let mut result = Http1Connection::connect::<TcpGatewayProxyForwardStream>(
                        Some(id),
                        None,
                        None,
                        remote_host,
                        proxy_pass_to.connect_timeout,
                    )
                    .await?;

                    let read_part = result.get_read_part();

                    let server_write_part = server_write_part.clone();
                    tokio::spawn(from_remote_to_server_loop(
                        server_write_part.clone(),
                        end_point_info,
                        read_part,
                    ));

                    return Ok(Self::Http1OverGateway(result));
                }
                crate::configurations::MyReverseProxyRemoteEndpoint::OverSsh {
                    ssh_credentials,
                    remote_host,
                } => {
                    let mut result = Http1Connection::connect::<my_ssh::SshAsyncChannel>(
                        None,
                        Some(ssh_credentials.clone()),
                        None,
                        remote_host,
                        proxy_pass_to.connect_timeout,
                    )
                    .await?;

                    let read_part = result.get_read_part();

                    let server_write_part = server_write_part.clone();
                    tokio::spawn(from_remote_to_server_loop(
                        server_write_part.clone(),
                        end_point_info,
                        read_part,
                    ));

                    return Ok(Self::Http1OverSsh(result));
                }
                crate::configurations::MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    if let Some(scheme) = remote_host.get_scheme() {
                        if scheme.is_https() {
                            let mut result =
                                Http1Connection::connect::<
                                    my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
                                >(
                                    None, None, None, remote_host, proxy_pass_to.connect_timeout
                                )
                                .await?;

                            let read_part = result.get_read_part();

                            let server_write_part = server_write_part.clone();
                            tokio::spawn(from_remote_to_server_loop(
                                server_write_part.clone(),
                                end_point_info,
                                read_part,
                            ));

                            return Ok(Self::Https1Direct(result));
                        }
                    }
                    let mut result = Http1Connection::connect::<tokio::net::TcpStream>(
                        None,
                        None,
                        None,
                        remote_host,
                        proxy_pass_to.connect_timeout,
                    )
                    .await?;

                    let read_part = result.get_read_part();

                    let server_write_part = server_write_part.clone();
                    tokio::spawn(from_remote_to_server_loop(
                        server_write_part.clone(),
                        end_point_info,
                        read_part,
                    ));

                    return Ok(Self::Http1Direct(result));
                }
            },
            ProxyPassTo::Http2(_) => {
                todo!("Http2 temporary is disabled")
            }
            ProxyPassTo::UnixHttp1(proxy_pass_to) => match &proxy_pass_to.remote_host {
                crate::configurations::MyReverseProxyRemoteEndpoint::Gateway { .. } => todo!(),
                crate::configurations::MyReverseProxyRemoteEndpoint::OverSsh { .. } => todo!(),
                crate::configurations::MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    let mut result = Http1Connection::connect::<tokio::net::UnixStream>(
                        None,
                        None,
                        None,
                        remote_host,
                        proxy_pass_to.connect_timeout,
                    )
                    .await?;

                    let read_part = result.get_read_part();

                    let server_write_part = server_write_part.clone();
                    tokio::spawn(from_remote_to_server_loop(
                        server_write_part.clone(),
                        end_point_info,
                        read_part,
                    ));

                    return Ok(Self::Http1UnixSocket(result));
                }
            },
            ProxyPassTo::UnixHttp2(_) => todo!("Not Implemented"),
            ProxyPassTo::FilesPath(model) => {
                let mut path_content = LocalPathContent::new(
                    model.files_path.to_string().as_str(),
                    model.default_file.clone(),
                );

                let read_part = path_content.get_read_path();

                tokio::spawn(from_remote_to_server_loop(
                    server_write_part.clone(),
                    end_point_info,
                    read_part,
                ));
                return Ok(Self::LocalFiles(path_content));
            }
            ProxyPassTo::Static(model) => todo!("Not Implemented"),
        }
    }

    pub async fn send_h1_header(&mut self, h1_headers: &Http1HeadersBuilder, time_out: Duration) {
        match self {
            Self::Http1Direct(connection) => {
                connection.send(h1_headers.as_slice(), time_out).await;
            }
            Self::Http1UnixSocket(connection) => {
                connection.send(h1_headers.as_slice(), time_out).await;
            }
            Self::Https1Direct(connection) => {
                connection.send(h1_headers.as_slice(), time_out).await;
            }
            Self::Http1OverSsh(connection) => {
                connection.send(h1_headers.as_slice(), time_out).await;
            }

            Self::Http1OverGateway(connection) => {
                connection.send(h1_headers.as_slice(), time_out).await;
            }
            Self::LocalFiles(connection) => {
                connection.send_headers(h1_headers).await;
            }
        }
    }
}

#[async_trait::async_trait]
impl NetworkStreamWritePart for RemoteConnection {
    async fn shutdown_socket(&mut self) {
        match self {
            Self::Http1Direct(connection) => connection.shutdown_socket().await,
            Self::Http1UnixSocket(connection) => connection.shutdown_socket().await,
            Self::Https1Direct(connection) => connection.shutdown_socket().await,
            Self::Http1OverSsh(connection) => connection.shutdown_socket().await,
            Self::Http1OverGateway(connection) => connection.shutdown_socket().await,
            Self::LocalFiles(_) => {}
        }
    }
    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        match self {
            Self::Http1Direct(connection) => connection.write_to_socket(buffer).await,
            Self::Http1UnixSocket(connection) => connection.write_to_socket(buffer).await,
            Self::Https1Direct(connection) => connection.write_to_socket(buffer).await,
            Self::Http1OverSsh(connection) => connection.write_to_socket(buffer).await,
            Self::Http1OverGateway(connection) => connection.write_to_socket(buffer).await,
            Self::LocalFiles(_) => Ok(()),
        }
    }
}
