use std::{sync::Arc, time::Duration};

use crate::configurations::*;

use crate::h1_proxy_server::{H1ServerWritePart, H1Writer, HttpConnectionInfo};

use crate::tcp_utils::LoopBuffer;
use crate::{
    h1_utils::*, http_content_source::local_path::LocalPathContent, network_stream::*,
    tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream,
};

use super::*;

pub struct Http1ConnectionContext<WritePart: NetworkStreamWritePart + Send + Sync + 'static> {
    pub h1_server_write_part: H1ServerWritePart<WritePart>,
    pub http_connection_info: HttpConnectionInfo,
    pub end_point_info: Arc<HttpEndpointInfo>,
    pub request_id: u64,
}

pub enum RemoteConnectionInner {
    Http1Direct(
        Http1ConnectionInner<
            tokio::io::WriteHalf<tokio::net::TcpStream>,
            tokio::io::ReadHalf<tokio::net::TcpStream>,
        >,
    ),

    Http1UnixSocket(
        Http1ConnectionInner<
            tokio::io::WriteHalf<tokio::net::UnixStream>,
            tokio::io::ReadHalf<tokio::net::UnixStream>,
        >,
    ),
    Https1Direct(
        Http1ConnectionInner<
            tokio::io::WriteHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
            tokio::io::ReadHalf<my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
        >,
    ),
    Http1OverSsh(
        Http1ConnectionInner<
            tokio::io::WriteHalf<my_ssh::SshAsyncChannel>,
            tokio::io::ReadHalf<my_ssh::SshAsyncChannel>,
        >,
    ),
    Http1OverGateway(
        Http1ConnectionInner<TcpGatewayProxyForwardStream, TcpGatewayProxyForwardStream>,
    ),
    StaticContent(Arc<StaticContentConfig>),
    LocalFiles(Arc<LocalPathContent>),
}

pub struct RemoteConnection {
    inner: RemoteConnectionInner,
    pub mcp_path: Option<String>,
}

impl RemoteConnection {
    pub async fn connect(proxy_pass_to: &ProxyPassToConfig) -> Result<Self, NetworkError> {
        match proxy_pass_to {
            ProxyPassToConfig::Http1(proxy_pass_to) => match &proxy_pass_to.remote_host {
                crate::configurations::MyReverseProxyRemoteEndpoint::Gateway {
                    id,
                    remote_host,
                } => {
                    let result = Http1ConnectionInner::connect::<TcpGatewayProxyForwardStream>(
                        Some(id),
                        None,
                        None,
                        remote_host,
                        proxy_pass_to.connect_timeout,
                    )
                    .await?;

                    return Ok(Self {
                        inner: RemoteConnectionInner::Http1OverGateway(result),
                        mcp_path: if proxy_pass_to.is_mcp {
                            Some(proxy_pass_to.remote_host.get_path_and_query().to_string())
                        } else {
                            None
                        },
                    });
                }
                crate::configurations::MyReverseProxyRemoteEndpoint::OverSsh {
                    ssh_credentials,
                    remote_host,
                } => {
                    let result = Http1ConnectionInner::connect::<my_ssh::SshAsyncChannel>(
                        None,
                        Some(ssh_credentials.clone()),
                        None,
                        remote_host,
                        proxy_pass_to.connect_timeout,
                    )
                    .await?;

                    return Ok(Self {
                        inner: RemoteConnectionInner::Http1OverSsh(result),
                        mcp_path: if proxy_pass_to.is_mcp {
                            Some(proxy_pass_to.remote_host.get_path_and_query().to_string())
                        } else {
                            None
                        },
                    });
                }
                crate::configurations::MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    if let Some(scheme) = remote_host.get_scheme() {
                        if scheme.is_https() {
                            let result =
                                Http1ConnectionInner::connect::<
                                    my_tls::tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
                                >(
                                    None, None, None, remote_host, proxy_pass_to.connect_timeout
                                )
                                .await?;

                            return Ok(Self {
                                inner: RemoteConnectionInner::Https1Direct(result),
                                mcp_path: if proxy_pass_to.is_mcp {
                                    Some(proxy_pass_to.remote_host.get_path_and_query().to_string())
                                } else {
                                    None
                                },
                            });
                        }
                    }
                    let result = Http1ConnectionInner::connect::<tokio::net::TcpStream>(
                        None,
                        None,
                        None,
                        remote_host,
                        proxy_pass_to.connect_timeout,
                    )
                    .await?;

                    return Ok(Self {
                        inner: RemoteConnectionInner::Http1Direct(result),
                        mcp_path: if proxy_pass_to.is_mcp {
                            Some(proxy_pass_to.remote_host.get_path_and_query().to_string())
                        } else {
                            None
                        },
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
                    let result = Http1ConnectionInner::connect::<tokio::net::UnixStream>(
                        None,
                        None,
                        None,
                        remote_host,
                        proxy_pass_to.connect_timeout,
                    )
                    .await?;

                    return Ok(Self {
                        inner: RemoteConnectionInner::Http1UnixSocket(result),
                        mcp_path: if proxy_pass_to.is_mcp {
                            Some(proxy_pass_to.remote_host.get_path_and_query().to_string())
                        } else {
                            None
                        },
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
                    inner: RemoteConnectionInner::LocalFiles(Arc::new(path_content)),
                    mcp_path: None,
                });
            }
            ProxyPassToConfig::Static(config) => {
                return Ok(Self {
                    inner: RemoteConnectionInner::StaticContent(config.clone()),
                    mcp_path: None,
                });
            }
        }
    }

    pub async fn send_h1_header(
        &mut self,
        request_id: u64,
        h1_headers: &Http1HeadersBuilder,
        time_out: Duration,
    ) -> bool {
        match &mut self.inner {
            RemoteConnectionInner::Http1Direct(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await
                    .is_ok()
            }
            RemoteConnectionInner::Http1UnixSocket(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await
                    .is_ok()
            }
            RemoteConnectionInner::Https1Direct(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await
                    .is_ok()
            }
            RemoteConnectionInner::Http1OverSsh(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await
                    .is_ok()
            }

            RemoteConnectionInner::Http1OverGateway(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await
                    .is_ok()
            }
            RemoteConnectionInner::StaticContent(_) => true,
            RemoteConnectionInner::LocalFiles(connection) => {
                connection.send_headers(request_id, h1_headers).await;

                true
            }
        }
    }

    pub fn read_http_response<MyWritePart: NetworkStreamWritePart + Send + Sync + 'static>(
        &self,
        connection_context: Http1ConnectionContext<MyWritePart>,
    ) -> bool {
        match &self.inner {
            RemoteConnectionInner::Http1Direct(inner) => {
                let read_part = inner.get_read_part();
                if read_part.get_disconnected() {
                    return false;
                }
                tokio::spawn(send_response_loop(connection_context, read_part));
            }
            RemoteConnectionInner::Http1UnixSocket(inner) => {
                let read_part = inner.get_read_part();
                if read_part.get_disconnected() {
                    return false;
                }
                tokio::spawn(send_response_loop(connection_context, read_part));
            }
            RemoteConnectionInner::Https1Direct(inner) => {
                let read_part = inner.get_read_part();
                if read_part.get_disconnected() {
                    return false;
                }
                tokio::spawn(send_response_loop(connection_context, read_part));
            }
            RemoteConnectionInner::Http1OverSsh(inner) => {
                let read_part = inner.get_read_part();
                if read_part.get_disconnected() {
                    return false;
                }
                tokio::spawn(send_response_loop(connection_context, read_part));
            }
            RemoteConnectionInner::Http1OverGateway(inner) => {
                let read_part = inner.get_read_part();
                if read_part.get_disconnected() {
                    return false;
                }
                tokio::spawn(send_response_loop(connection_context, read_part));
            }
            RemoteConnectionInner::StaticContent(static_content) => {
                tokio::spawn(execute_static_content(
                    connection_context,
                    static_content.clone(),
                ));
            }
            RemoteConnectionInner::LocalFiles(local_files) => {
                tokio::spawn(execute_local_path(connection_context, local_files.clone()));
            }
        }
        true
    }

    pub async fn web_socket_upgrade<
        ServerReadPart: NetworkStreamReadPart + Send + Sync + 'static,
        ServerWritePart: NetworkStreamWritePart + Send + Sync + 'static,
    >(
        self,
        server_read_part: ServerReadPart,
        server_write_part: ServerWritePart,

        server_loop_buffer: LoopBuffer,
    ) -> Result<(), (ServerReadPart, ServerWritePart)> {
        match self.inner {
            RemoteConnectionInner::Http1Direct(inner) => {
                let (remote_read_part, remote_write_part, remote_loop_buffer) =
                    inner.get_read_and_write_parts().await;

                tokio::spawn(crate::tcp_utils::copy_streams(
                    server_read_part,
                    remote_write_part,
                    server_loop_buffer,
                    None,
                ));

                tokio::spawn(crate::tcp_utils::copy_streams(
                    remote_read_part,
                    server_write_part,
                    remote_loop_buffer,
                    None,
                ));

                return Ok(());
            }
            RemoteConnectionInner::Http1UnixSocket(inner) => {
                let (remote_read_part, remote_write_part, remote_loop_buffer) =
                    inner.get_read_and_write_parts().await;

                tokio::spawn(crate::tcp_utils::copy_streams(
                    server_read_part,
                    remote_write_part,
                    server_loop_buffer,
                    None,
                ));

                tokio::spawn(crate::tcp_utils::copy_streams(
                    remote_read_part,
                    server_write_part,
                    remote_loop_buffer,
                    None,
                ));

                return Ok(());
            }
            RemoteConnectionInner::Https1Direct(inner) => {
                let (remote_read_part, remote_write_part, remote_loop_buffer) =
                    inner.get_read_and_write_parts().await;

                tokio::spawn(crate::tcp_utils::copy_streams(
                    server_read_part,
                    remote_write_part,
                    server_loop_buffer,
                    None,
                ));

                tokio::spawn(crate::tcp_utils::copy_streams(
                    remote_read_part,
                    server_write_part,
                    remote_loop_buffer,
                    None,
                ));

                return Ok(());
            }
            RemoteConnectionInner::Http1OverSsh(mut inner) => {
                let ssh_session_handler = inner.ssh_session_handler.take();
                let (remote_read_part, remote_write_part, remote_loop_buffer) =
                    inner.get_read_and_write_parts().await;

                tokio::spawn(crate::tcp_utils::copy_streams(
                    server_read_part,
                    remote_write_part,
                    server_loop_buffer,
                    ssh_session_handler,
                ));

                tokio::spawn(crate::tcp_utils::copy_streams(
                    remote_read_part,
                    server_write_part,
                    remote_loop_buffer,
                    None,
                ));

                return Ok(());
            }
            RemoteConnectionInner::Http1OverGateway(inner) => {
                let (remote_read_part, remote_write_part, remote_loop_buffer) =
                    inner.get_read_and_write_parts().await;

                tokio::spawn(crate::tcp_utils::copy_streams(
                    server_read_part,
                    remote_write_part,
                    server_loop_buffer,
                    None,
                ));

                tokio::spawn(crate::tcp_utils::copy_streams(
                    remote_read_part,
                    server_write_part,
                    remote_loop_buffer,
                    None,
                ));

                return Ok(());
            }
            RemoteConnectionInner::StaticContent(_) => {
                return Err((server_read_part, server_write_part))
            }
            RemoteConnectionInner::LocalFiles(_) => {
                return Err((server_read_part, server_write_part))
            }
        }
    }
}

#[async_trait::async_trait]
impl H1Writer for RemoteConnection {
    async fn write_http_payload(
        &mut self,
        _request_id: u64,
        buffer: &[u8],
        timeout: Duration,
    ) -> Result<(), NetworkError> {
        match &mut self.inner {
            RemoteConnectionInner::Http1Direct(inner) => {
                inner.send_with_timeout(buffer, timeout).await?;
                Ok(())
            }
            RemoteConnectionInner::Http1UnixSocket(inner) => {
                inner.send_with_timeout(buffer, timeout).await?;
                Ok(())
            }
            RemoteConnectionInner::Https1Direct(inner) => {
                inner.send_with_timeout(buffer, timeout).await?;
                Ok(())
            }
            RemoteConnectionInner::Http1OverSsh(inner) => {
                inner.send_with_timeout(buffer, timeout).await?;
                Ok(())
            }
            RemoteConnectionInner::Http1OverGateway(inner) => {
                inner.send_with_timeout(buffer, timeout).await?;
                Ok(())
            }
            RemoteConnectionInner::StaticContent(_) => Ok(()),
            RemoteConnectionInner::LocalFiles(_) => Ok(()),
        }
    }
}

#[async_trait::async_trait]
impl NetworkStreamWritePart for RemoteConnection {
    async fn shutdown_socket(&mut self) {
        match &mut self.inner {
            RemoteConnectionInner::Http1Direct(connection) => connection.shutdown_socket().await,
            RemoteConnectionInner::Http1UnixSocket(connection) => {
                connection.shutdown_socket().await
            }
            RemoteConnectionInner::Https1Direct(connection) => connection.shutdown_socket().await,
            RemoteConnectionInner::Http1OverSsh(connection) => connection.shutdown_socket().await,
            RemoteConnectionInner::Http1OverGateway(connection) => {
                connection.shutdown_socket().await
            }
            RemoteConnectionInner::StaticContent(_) => {}
            RemoteConnectionInner::LocalFiles(_) => {}
        }
    }
    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        const DISCONNECTED: &'static str = "Disconnected";
        match &mut self.inner {
            RemoteConnectionInner::Http1Direct(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            RemoteConnectionInner::Http1UnixSocket(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            RemoteConnectionInner::Https1Direct(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            RemoteConnectionInner::Http1OverSsh(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            RemoteConnectionInner::Http1OverGateway(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            RemoteConnectionInner::StaticContent(_) => Ok(()),
            RemoteConnectionInner::LocalFiles(_) => Ok(()),
        }
    }
    async fn flush_it(&mut self) -> Result<(), NetworkError> {
        match &mut self.inner {
            RemoteConnectionInner::Http1Direct(inner) => inner.flush_it().await,
            RemoteConnectionInner::Http1UnixSocket(inner) => inner.flush_it().await,
            RemoteConnectionInner::Https1Direct(inner) => inner.flush_it().await,
            RemoteConnectionInner::Http1OverSsh(inner) => inner.flush_it().await,
            RemoteConnectionInner::Http1OverGateway(inner) => inner.flush_it().await,
            RemoteConnectionInner::StaticContent(_) => Ok(()),
            RemoteConnectionInner::LocalFiles(_) => Ok(()),
        }
    }
}

async fn send_response_loop<
    RemoteReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
>(
    mut connection_context: Http1ConnectionContext<WritePart>,
    remote_connection: Arc<H1RemoteConnectionReadPart<RemoteReadPart>>,
) {
    let mut remote_read_part = remote_connection.h1_reader.lock().await;

    let Some(h1_reader) = remote_read_part.as_mut() else {
        connection_context
            .h1_server_write_part
            .write_http_payload_with_timeout(
                connection_context.request_id,
                crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                crate::consts::WRITE_TIMEOUT,
            )
            .await
            .unwrap();

        connection_context
            .h1_server_write_part
            .request_is_done(connection_context.request_id)
            .await;
        return;
    };

    let resp_headers = match h1_reader.read_headers().await {
        Ok(headers) => headers,
        Err(err) => {
            println!("Reading header from remote: {:?}", err);

            drop(remote_read_part);

            remote_connection.set_disconnected();

            connection_context
                .h1_server_write_part
                .write_http_payload_with_timeout(
                    connection_context.request_id,
                    crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                    crate::consts::WRITE_TIMEOUT,
                )
                .await
                .unwrap();

            connection_context
                .h1_server_write_part
                .request_is_done(connection_context.request_id)
                .await;
            return;
        }
    };

    let content_length = resp_headers.content_length;

    if let Err(err) = h1_reader.compile_headers(
        resp_headers,
        &connection_context.end_point_info.modify_response_headers,
        &connection_context.http_connection_info,
        &None,
        None,
    ) {
        println!("Compile headers from remote: {:?}", err);

        drop(remote_read_part);
        remote_connection.set_disconnected();
        connection_context
            .h1_server_write_part
            .write_http_payload_with_timeout(
                connection_context.request_id,
                crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                crate::consts::WRITE_TIMEOUT,
            )
            .await
            .unwrap();
        connection_context
            .h1_server_write_part
            .request_is_done(connection_context.request_id)
            .await;
        return;
    }

    if let Err(err) = connection_context
        .h1_server_write_part
        .write_http_payload_with_timeout(
            connection_context.request_id,
            h1_reader.h1_headers_builder.as_slice(),
            crate::consts::WRITE_TIMEOUT,
        )
        .await
    {
        println!("Sending headers from remote to server: {:?}", err);
        drop(remote_read_part);
        remote_connection.set_disconnected();
        let _ = connection_context
            .h1_server_write_part
            .write_http_payload_with_timeout(
                connection_context.request_id,
                crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                crate::consts::WRITE_TIMEOUT,
            )
            .await;

        connection_context
            .h1_server_write_part
            .request_is_done(connection_context.request_id)
            .await;
        return;
    }

    if let Err(err) = h1_reader
        .transfer_body(
            connection_context.request_id,
            &mut connection_context.h1_server_write_part,
            content_length,
        )
        .await
    {
        println!("Sending body from remote to server: {:?}", err);
        drop(remote_read_part);
        remote_connection.set_disconnected();
        let _ = connection_context
            .h1_server_write_part
            .write_http_payload(
                connection_context.request_id,
                crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                crate::consts::WRITE_TIMEOUT,
            )
            .await;

        connection_context
            .h1_server_write_part
            .request_is_done(connection_context.request_id)
            .await;
        return;
    }

    connection_context
        .h1_server_write_part
        .request_is_done(connection_context.request_id)
        .await;
}

async fn execute_local_path<WritePart: NetworkStreamWritePart + Send + Sync + 'static>(
    connection_context: Http1ConnectionContext<WritePart>,
    local_path_content: Arc<LocalPathContent>,
) {
    let content = local_path_content
        .get_content(connection_context.request_id)
        .await;

    let _ = connection_context
        .h1_server_write_part
        .write_http_payload_with_timeout(
            connection_context.request_id,
            content.as_slice(),
            crate::consts::WRITE_TIMEOUT,
        )
        .await;

    connection_context
        .h1_server_write_part
        .request_is_done(connection_context.request_id)
        .await;
}

async fn execute_static_content<WritePart: NetworkStreamWritePart + Send + Sync + 'static>(
    connection_context: Http1ConnectionContext<WritePart>,
    static_content: Arc<StaticContentConfig>,
) {
    let mut result = Http1ResponseBuilder::new(static_content.status_code);

    if let Some(content_type) = static_content.content_type.as_ref() {
        result = result.add_content_type(content_type);
    }

    let content = result.build_with_body(&static_content.body.as_slice());

    let _ = connection_context
        .h1_server_write_part
        .write_http_payload_with_timeout(
            connection_context.request_id,
            content.as_slice(),
            crate::consts::WRITE_TIMEOUT,
        )
        .await;

    connection_context
        .h1_server_write_part
        .request_is_done(connection_context.request_id)
        .await;
}
