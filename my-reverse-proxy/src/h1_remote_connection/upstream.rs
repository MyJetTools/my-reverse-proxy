use std::{sync::Arc, time::Duration};

use crate::configurations::*;

use crate::h1_proxy_server::{H1ServerWritePart, H1Writer, HttpConnectionInfo};

use crate::{
    h1_utils::*, http_content_source::local_path::LocalPathContent, network_stream::*,
    tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream,
};

use super::*;

#[derive(Clone)]
pub struct Http1ServerConnectionContext<
    ServerWritePart: NetworkStreamWritePart + Send + Sync + 'static,
    ServerReadPart: NetworkStreamReadPart + Send + Sync + 'static,
> {
    pub h1_server_write_part: H1ServerWritePart<ServerWritePart, ServerReadPart>,
    pub http_connection_info: HttpConnectionInfo,
    pub end_point_info: Arc<HttpEndpointInfo>,
}

impl<
        ServerWritePart: NetworkStreamWritePart + Send + Sync + 'static,
        ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    > Http1ServerConnectionContext<ServerWritePart, ReadPart>
{
    pub fn clone(&self) -> Self {
        Self {
            h1_server_write_part: self.h1_server_write_part.clone(),
            http_connection_info: self.http_connection_info.clone(),
            end_point_info: self.end_point_info.clone(),
        }
    }
}

pub enum UpstreamInner {
    Http1Direct(Http1ConnectionInner<tokio::io::WriteHalf<tokio::net::TcpStream>>),

    Http1UnixSocket(Http1ConnectionInner<tokio::io::WriteHalf<tokio::net::UnixStream>>),
    Https1Direct(
        Http1ConnectionInner<
            tokio::io::WriteHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
        >,
    ),
    Http1OverSsh(Http1ConnectionInner<tokio::io::WriteHalf<my_ssh::SshAsyncChannel>>),
    Http1OverGateway(Http1ConnectionInner<TcpGatewayProxyForwardStream>),
    StaticContent(Arc<StaticContentConfig>),

    LocalFiles(Arc<LocalPathContent>),
}

pub struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub struct Upstream {
    pub inner: UpstreamInner,
    pub connection_id: u64,
    #[allow(dead_code)]
    response_loop_handle: Option<AbortOnDrop>,
}

impl Upstream {
    pub async fn connect<
        ServerWritePart: NetworkStreamWritePart + Send + Sync + 'static,
        ServerReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    >(
        proxy_pass_to: &ProxyPassToConfig,
        http1_connection_ctx: &Http1ServerConnectionContext<ServerWritePart, ServerReadPart>,
    ) -> Result<Self, NetworkError> {
        let connection_id = super::CONN_ID.get_next();
        match proxy_pass_to {
            ProxyPassToConfig::Http1(proxy_pass_to)
            | ProxyPassToConfig::McpHttp1(proxy_pass_to) => match &proxy_pass_to.remote_host {
                crate::configurations::MyReverseProxyRemoteEndpoint::Gateway {
                    id,
                    remote_host,
                } => {
                    let (result, read_part, ssh_handler) = Http1ConnectionInner::connect::<
                        TcpGatewayProxyForwardStream,
                        TcpGatewayProxyForwardStream,
                    >(
                        Some(id),
                        None,
                        None,
                        remote_host,
                        proxy_pass_to.connect_timeout,
                    )
                    .await?;

                    let response_loop_handle = crate::app::spawn_named(
                        "h1_response_read_loop_gateway",
                        super::response_read_loop(
                            connection_id,
                            read_part,
                            result.get_remote_disconnect_trigger(),
                            http1_connection_ctx.clone(),
                            ssh_handler,
                        ),
                    );

                    return Ok(Self {
                        connection_id,
                        inner: UpstreamInner::Http1OverGateway(result),
                        response_loop_handle: Some(AbortOnDrop(response_loop_handle)),
                    });
                }
                crate::configurations::MyReverseProxyRemoteEndpoint::OverSsh {
                    ssh_credentials,
                    remote_host,
                } => {
                    let (result, read_part, ssh_handler) = Http1ConnectionInner::connect::<
                        tokio::io::ReadHalf<my_ssh::SshAsyncChannel>,
                        my_ssh::SshAsyncChannel,
                    >(
                        None,
                        Some(ssh_credentials.clone()),
                        None,
                        remote_host,
                        proxy_pass_to.connect_timeout,
                    )
                    .await?;

                    let response_loop_handle = crate::app::spawn_named(
                        "h1_response_read_loop_ssh",
                        super::response_read_loop(
                            connection_id,
                            read_part,
                            result.get_remote_disconnect_trigger(),
                            http1_connection_ctx.clone(),
                            ssh_handler,
                        ),
                    );

                    return Ok(Self {
                        connection_id,
                        inner: UpstreamInner::Http1OverSsh(result),
                        response_loop_handle: Some(AbortOnDrop(response_loop_handle)),
                    });
                }
                crate::configurations::MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    if let Some(scheme) = remote_host.get_scheme() {
                        if scheme.is_https() {
                            let (result, read_part, ssh_handler) =
                                Http1ConnectionInner::connect::<
                                    tokio::io::ReadHalf<
                                        my_tls::tokio_rustls::client::TlsStream<
                                            tokio::net::TcpStream,
                                        >,
                                    >,
                                    my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
                                >(
                                    None, None, None, remote_host, proxy_pass_to.connect_timeout
                                )
                                .await?;

                            let response_loop_handle = crate::app::spawn_named(
                                "h1_response_read_loop_https",
                                super::response_read_loop(
                                    connection_id,
                                    read_part,
                                    result.get_remote_disconnect_trigger(),
                                    http1_connection_ctx.clone(),
                                    ssh_handler,
                                ),
                            );

                            return Ok(Self {
                                connection_id,
                                inner: UpstreamInner::Https1Direct(result),
                                response_loop_handle: Some(AbortOnDrop(response_loop_handle)),
                            });
                        }
                    }
                    let (result, read_part, ssh_handler) =
                        Http1ConnectionInner::connect::<
                            tokio::io::ReadHalf<tokio::net::TcpStream>,
                            tokio::net::TcpStream,
                        >(
                            None, None, None, remote_host, proxy_pass_to.connect_timeout
                        )
                        .await?;

                    let response_loop_handle = crate::app::spawn_named(
                        "h1_response_read_loop_direct",
                        super::response_read_loop(
                            connection_id,
                            read_part,
                            result.get_remote_disconnect_trigger(),
                            http1_connection_ctx.clone(),
                            ssh_handler,
                        ),
                    );

                    return Ok(Self {
                        connection_id,
                        inner: UpstreamInner::Http1Direct(result),
                        response_loop_handle: Some(AbortOnDrop(response_loop_handle)),
                    });
                }
            },
            ProxyPassToConfig::Http2(_) => {
                todo!("Http2 temporary is disabled")
            }
            ProxyPassToConfig::UnixHttp1(proxy_pass_to) => match &proxy_pass_to.remote_host {
                crate::configurations::MyReverseProxyRemoteEndpoint::Gateway { .. } => todo!(),
                crate::configurations::MyReverseProxyRemoteEndpoint::OverSsh { .. } => todo!(),
                crate::configurations::MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    println!("Doing connection");
                    let (result, read_part, ssh_handler) =
                        Http1ConnectionInner::connect::<
                            tokio::io::ReadHalf<tokio::net::UnixStream>,
                            tokio::net::UnixStream,
                        >(
                            None, None, None, remote_host, proxy_pass_to.connect_timeout
                        )
                        .await?;

                    println!("Starting response read loop");
                    let response_loop_handle = crate::app::spawn_named(
                        "h1_response_read_loop_unix",
                        super::response_read_loop(
                            connection_id,
                            read_part,
                            result.get_remote_disconnect_trigger(),
                            http1_connection_ctx.clone(),
                            ssh_handler,
                        ),
                    );

                    return Ok(Self {
                        connection_id,
                        inner: UpstreamInner::Http1UnixSocket(result),
                        response_loop_handle: Some(AbortOnDrop(response_loop_handle)),
                    });
                }
            },
            ProxyPassToConfig::UnixHttp2(_) => todo!("Not Implemented"),
            ProxyPassToConfig::FilesPath(model) => {
                let path_content = LocalPathContent::new(
                    model.files_path.to_string().as_str(),
                    model.default_file.clone(),
                );

                return Ok(Self {
                    connection_id,
                    inner: UpstreamInner::LocalFiles(Arc::new(path_content)),
                    response_loop_handle: None,
                });
            }
            ProxyPassToConfig::Static(config) => {
                return Ok(Self {
                    connection_id,
                    inner: UpstreamInner::StaticContent(config.clone()),
                    response_loop_handle: None,
                });
            }
            ProxyPassToConfig::Drop => {
                unreachable!(
                    "Upstream::connect must not be called for Drop proxy_pass — caller should short-circuit before reaching upstream connect"
                );
            }
        }
    }

    pub async fn send_h1_header(
        &mut self,
        h1_headers: &Http1HeadersBuilder,
        time_out: Duration,
    ) -> bool {
        match &mut self.inner {
            UpstreamInner::Http1Direct(connection) => {
                let disconnected = connection.is_disconnected();

                if disconnected {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await
                    .is_ok()
            }
            UpstreamInner::Http1UnixSocket(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await
                    .is_ok()
            }
            UpstreamInner::Https1Direct(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await
                    .is_ok()
            }
            UpstreamInner::Http1OverSsh(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await
                    .is_ok()
            }

            UpstreamInner::Http1OverGateway(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await
                    .is_ok()
            }
            UpstreamInner::StaticContent { .. } => true,
            UpstreamInner::LocalFiles(local_files) => {
                local_files.send_headers(self.connection_id, h1_headers);
                true
            }
        }
    }

    pub fn read_http_response<
        ServerWritePart: NetworkStreamWritePart + Send + Sync + 'static,
        ServerReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    >(
        &self,
        http1_connection_ctx: Http1ServerConnectionContext<ServerWritePart, ServerReadPart>,
    ) -> bool {
        match &self.inner {
            UpstreamInner::Http1Direct(_) => {}
            UpstreamInner::Http1UnixSocket(_) => {}
            UpstreamInner::Https1Direct(_) => {}
            UpstreamInner::Http1OverSsh(_) => {}
            UpstreamInner::Http1OverGateway(_) => {}
            UpstreamInner::StaticContent(static_content) => {
                crate::app::spawn_named(
                    "h1_execute_static_content",
                    execute_static_content(
                        http1_connection_ctx,
                        static_content.clone(),
                        self.connection_id,
                    ),
                );
            }
            UpstreamInner::LocalFiles(local_files) => {
                crate::app::spawn_named(
                    "h1_execute_local_path",
                    execute_local_path(
                        self.connection_id,
                        http1_connection_ctx,
                        local_files.clone(),
                    ),
                );
            }
        }
        true
    }
}

#[async_trait::async_trait]
impl H1Writer for Upstream {
    async fn write_http_payload(
        &mut self,
        _request_id: u64,
        buffer: &[u8],
        timeout: Duration,
    ) -> Result<(), NetworkError> {
        match &mut self.inner {
            UpstreamInner::Http1Direct(inner) => {
                inner.send_with_timeout(buffer, timeout).await?;
                Ok(())
            }
            UpstreamInner::Http1UnixSocket(inner) => {
                inner.send_with_timeout(buffer, timeout).await?;
                Ok(())
            }
            UpstreamInner::Https1Direct(inner) => {
                inner.send_with_timeout(buffer, timeout).await?;
                Ok(())
            }
            UpstreamInner::Http1OverSsh(inner) => {
                inner.send_with_timeout(buffer, timeout).await?;
                Ok(())
            }
            UpstreamInner::Http1OverGateway(inner) => {
                inner.send_with_timeout(buffer, timeout).await?;
                Ok(())
            }
            UpstreamInner::StaticContent { .. } => Ok(()),
            UpstreamInner::LocalFiles { .. } => Ok(()),
        }
    }
}

#[async_trait::async_trait]
impl NetworkStreamWritePart for Upstream {
    async fn shutdown_socket(&mut self) {
        match &mut self.inner {
            UpstreamInner::Http1Direct(connection) => connection.shutdown_socket().await,
            UpstreamInner::Http1UnixSocket(connection) => connection.shutdown_socket().await,
            UpstreamInner::Https1Direct(connection) => connection.shutdown_socket().await,
            UpstreamInner::Http1OverSsh(connection) => connection.shutdown_socket().await,
            UpstreamInner::Http1OverGateway(connection) => connection.shutdown_socket().await,
            UpstreamInner::StaticContent { .. } => {}
            UpstreamInner::LocalFiles { .. } => {}
        }
    }
    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        const DISCONNECTED: &'static str = "Disconnected";
        match &mut self.inner {
            UpstreamInner::Http1Direct(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            UpstreamInner::Http1UnixSocket(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            UpstreamInner::Https1Direct(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            UpstreamInner::Http1OverSsh(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            UpstreamInner::Http1OverGateway(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            UpstreamInner::StaticContent { .. } => Ok(()),
            UpstreamInner::LocalFiles { .. } => Ok(()),
        }
    }
    async fn flush_it(&mut self) -> Result<(), NetworkError> {
        match &mut self.inner {
            UpstreamInner::Http1Direct(inner) => inner.flush_it().await,
            UpstreamInner::Http1UnixSocket(inner) => inner.flush_it().await,
            UpstreamInner::Https1Direct(inner) => inner.flush_it().await,
            UpstreamInner::Http1OverSsh(inner) => inner.flush_it().await,
            UpstreamInner::Http1OverGateway(inner) => inner.flush_it().await,
            UpstreamInner::StaticContent { .. } => Ok(()),
            UpstreamInner::LocalFiles { .. } => Ok(()),
        }
    }
}

async fn execute_local_path<
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
>(
    connection_id: u64,
    connection_context: Http1ServerConnectionContext<WritePart, ReadPart>,
    local_path_content: Arc<LocalPathContent>,
) {
    let content = local_path_content.get_content(connection_id).await;

    let _ = connection_context
        .h1_server_write_part
        .write_http_payload_with_timeout(
            connection_id,
            content.as_slice(),
            crate::consts::WRITE_TIMEOUT,
        )
        .await;

    connection_context
        .h1_server_write_part
        .request_is_done(connection_id)
        .await;
}

async fn execute_static_content<
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
>(
    connection_context: Http1ServerConnectionContext<WritePart, ReadPart>,
    static_content: Arc<StaticContentConfig>,
    connection_id: u64,
) {
    let mut result = Http1ResponseBuilder::new(static_content.status_code);

    if let Some(content_type) = static_content.content_type.as_ref() {
        result = result.add_content_type(content_type);
    }

    let content = result.build_with_body(&static_content.body.as_slice());

    let _ = connection_context
        .h1_server_write_part
        .write_http_payload_with_timeout(
            connection_id,
            content.as_slice(),
            crate::consts::WRITE_TIMEOUT,
        )
        .await;

    connection_context
        .h1_server_write_part
        .request_is_done(connection_id)
        .await;
}
