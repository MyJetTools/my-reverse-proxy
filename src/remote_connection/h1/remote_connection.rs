use std::{sync::Arc, time::Duration};

use tokio::sync::Mutex;

use crate::configurations::*;
use crate::h1_server::server_loop::HttpServerSingleThreadedPart;
use crate::h1_server::HttpConnectionInfo;

use crate::{
    h1_utils::*, http_content_source::local_path::LocalPathContent, network_stream::*,
    tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream,
};

use super::*;

pub struct Http1ConnectionContext<WritePart: NetworkStreamWritePart + Send + Sync + 'static> {
    pub server_single_threaded_part: Arc<Mutex<HttpServerSingleThreadedPart<WritePart>>>,
    pub http_connection_info: HttpConnectionInfo,
    pub end_point_info: Arc<HttpEndpointInfo>,
    pub request_id: u64,
}

impl<WritePart: NetworkStreamWritePart + Send + Sync + 'static> Http1ConnectionContext<WritePart> {
    async fn request_is_done(&self) {
        let mut write_access = self.server_single_threaded_part.lock().await;

        for itm in write_access.current_requests.iter_mut() {
            if itm.request_id == self.request_id {
                itm.done = true;
                break;
            }
        }

        loop {
            let done = match write_access.current_requests.get(0) {
                Some(itm) => itm.done,
                None => {
                    break;
                }
            };

            if done {
                let done_item = write_access.current_requests.remove(0);

                if done_item.buffer.len() > 0 {
                    write_access
                        .server_write_part
                        .write_all_with_timeout(&done_item.buffer, crate::consts::WRITE_TIMEOUT)
                        .await
                        .unwrap();
                }
            }
        }

        //println!("Requests: {}", write_access.current_requests.len());
    }
}

#[async_trait::async_trait]
impl<WritePart: NetworkStreamWritePart + Send + Sync + 'static> NetworkStreamWritePart
    for Http1ConnectionContext<WritePart>
{
    async fn shutdown_socket(&mut self) {
        let mut write_access = self.server_single_threaded_part.lock().await;
        write_access.server_write_part.shutdown_socket().await;
    }

    async fn write_to_socket(&mut self, _buffer: &[u8]) -> Result<(), std::io::Error> {
        panic!("Should not be used. Instead  write_http_payload should be used");
    }

    async fn write_http_payload(
        &mut self,
        request_id: u64,
        buffer: &[u8],
        timeout: Duration,
    ) -> Result<(), NetworkError> {
        let mut write_access = self.server_single_threaded_part.lock().await;

        if write_access.current_requests.get(0).unwrap().request_id == request_id {
            return write_access
                .server_write_part
                .write_all_with_timeout(buffer, timeout)
                .await;
        }

        for itm in write_access.current_requests.iter_mut() {
            if itm.request_id == request_id {
                itm.buffer.extend_from_slice(buffer);
            }
        }

        Ok(())
    }
}

pub enum RemoteConnection {
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
        Http1ConnectionInner<
            tokio::io::WriteHalf<TcpGatewayProxyForwardStream>,
            tokio::io::ReadHalf<TcpGatewayProxyForwardStream>,
        >,
    ),
    StaticContent(Arc<StaticContentConfig>),
    LocalFiles(Arc<LocalPathContent>),
}

impl RemoteConnection {
    pub async fn connect(proxy_pass_to: &ProxyPassToConfig) -> Result<Self, String> {
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

                    return Ok(Self::Http1OverGateway(result));
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

                    return Ok(Self::Http1OverSsh(result));
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

                            return Ok(Self::Https1Direct(result));
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

                    return Ok(Self::Http1Direct(result));
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

                    return Ok(Self::Http1UnixSocket(result));
                }
            },
            ProxyPassToConfig::UnixHttp2(_) => todo!("Not Implemented"),
            ProxyPassToConfig::FilesPath(model) => {
                let path_content = LocalPathContent::new(
                    model.files_path.to_string().as_str(),
                    model.default_file.clone(),
                );

                return Ok(Self::LocalFiles(Arc::new(path_content)));
            }
            ProxyPassToConfig::Static(config) => return Ok(Self::StaticContent(config.clone())),
        }
    }

    pub async fn send_h1_header(
        &mut self,
        request_id: u64,
        h1_headers: &Http1HeadersBuilder,
        time_out: Duration,
    ) -> bool {
        match self {
            Self::Http1Direct(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await;
                true
            }
            Self::Http1UnixSocket(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await;
                true
            }
            Self::Https1Direct(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await;
                true
            }
            Self::Http1OverSsh(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await;
                true
            }

            Self::Http1OverGateway(connection) => {
                if connection.is_disconnected() {
                    return false;
                }
                connection
                    .send_with_timeout(h1_headers.as_slice(), time_out)
                    .await;
                true
            }
            Self::StaticContent(_) => true,
            Self::LocalFiles(connection) => {
                connection.send_headers(request_id, h1_headers).await;

                true
            }
        }
    }

    pub fn read_http_response<MyWritePart: NetworkStreamWritePart + Send + Sync + 'static>(
        &self,
        connection_context: Http1ConnectionContext<MyWritePart>,
    ) -> bool {
        match self {
            RemoteConnection::Http1Direct(inner) => {
                let read_part = inner.get_read_part();
                if read_part.get_disconnected() {
                    return false;
                }
                tokio::spawn(send_response_loop(connection_context, read_part));
            }
            RemoteConnection::Http1UnixSocket(inner) => {
                let read_part = inner.get_read_part();
                if read_part.get_disconnected() {
                    return false;
                }
                tokio::spawn(send_response_loop(connection_context, read_part));
            }
            RemoteConnection::Https1Direct(inner) => {
                let read_part = inner.get_read_part();
                if read_part.get_disconnected() {
                    return false;
                }
                tokio::spawn(send_response_loop(connection_context, read_part));
            }
            RemoteConnection::Http1OverSsh(inner) => {
                let read_part = inner.get_read_part();
                if read_part.get_disconnected() {
                    return false;
                }
                tokio::spawn(send_response_loop(connection_context, read_part));
            }
            RemoteConnection::Http1OverGateway(inner) => {
                let read_part = inner.get_read_part();
                if read_part.get_disconnected() {
                    return false;
                }
                tokio::spawn(send_response_loop(connection_context, read_part));
            }
            RemoteConnection::StaticContent(static_content) => {
                tokio::spawn(execute_static_content(
                    connection_context,
                    static_content.clone(),
                ));
            }
            RemoteConnection::LocalFiles(local_files) => {
                tokio::spawn(execute_local_path(connection_context, local_files.clone()));
            }
        }
        true
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
            Self::StaticContent(_) => {}
            Self::LocalFiles(_) => {}
        }
    }
    async fn write_to_socket(&mut self, buffer: &[u8]) -> Result<(), std::io::Error> {
        const DISCONNECTED: &'static str = "Disconnected";
        match self {
            Self::Http1Direct(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            Self::Http1UnixSocket(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            Self::Https1Direct(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            Self::Http1OverSsh(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            Self::Http1OverGateway(connection) => {
                if connection.is_disconnected() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        DISCONNECTED,
                    ));
                }
                connection.write_to_socket(buffer).await
            }
            Self::StaticContent(_) => Ok(()),
            Self::LocalFiles(_) => Ok(()),
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
    let mut remote_read_part = remote_connection.read_half.lock().await;

    let http_headers = match remote_read_part.read_headers().await {
        Ok(headers) => headers,
        Err(err) => {
            println!("Reading header from remote: {:?}", err);

            drop(remote_read_part);

            remote_connection.set_disconnected();

            connection_context
                .write_http_payload(
                    connection_context.request_id,
                    crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                    crate::consts::WRITE_TIMEOUT,
                )
                .await
                .unwrap();

            connection_context.request_is_done().await;
            return;
        }
    };

    let content_length = http_headers.content_length;

    if let Err(err) = remote_read_part.compile_headers(
        http_headers,
        &connection_context.end_point_info.modify_response_headers,
        &connection_context.http_connection_info,
        &None,
    ) {
        println!("Compile headers from remote: {:?}", err);

        drop(remote_read_part);
        remote_connection.set_disconnected();
        connection_context
            .write_http_payload(
                connection_context.request_id,
                crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                crate::consts::WRITE_TIMEOUT,
            )
            .await
            .unwrap();
        connection_context.request_is_done().await;
        return;
    }

    if let Err(err) = connection_context
        .write_http_payload(
            connection_context.request_id,
            remote_read_part.h1_headers_builder.as_slice(),
            crate::consts::WRITE_TIMEOUT,
        )
        .await
    {
        println!("Sending headers from remote to server: {:?}", err);
        drop(remote_read_part);
        remote_connection.set_disconnected();
        let _ = connection_context
            .write_http_payload(
                connection_context.request_id,
                crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                crate::consts::WRITE_TIMEOUT,
            )
            .await;

        connection_context.request_is_done().await;
        return;
    }

    if let Err(err) = remote_read_part
        .transfer_body(
            connection_context.request_id,
            &mut connection_context,
            content_length,
        )
        .await
    {
        println!("Sending body from remote to server: {:?}", err);
        drop(remote_read_part);
        remote_connection.set_disconnected();
        let _ = connection_context
            .write_http_payload(
                connection_context.request_id,
                crate::error_templates::ERROR_GETTING_CONTENT_FROM_REMOTE_RESOURCE.as_slice(),
                crate::consts::WRITE_TIMEOUT,
            )
            .await;

        connection_context.request_is_done().await;
        return;
    }

    connection_context.request_is_done().await;
}

async fn execute_local_path<WritePart: NetworkStreamWritePart + Send + Sync + 'static>(
    mut connection_context: Http1ConnectionContext<WritePart>,
    local_path_content: Arc<LocalPathContent>,
) {
    let content = local_path_content
        .get_content(connection_context.request_id)
        .await;

    let _ = connection_context
        .write_http_payload(
            connection_context.request_id,
            content.as_slice(),
            crate::consts::WRITE_TIMEOUT,
        )
        .await;

    connection_context.request_is_done().await;
}

async fn execute_static_content<WritePart: NetworkStreamWritePart + Send + Sync + 'static>(
    mut connection_context: Http1ConnectionContext<WritePart>,
    static_content: Arc<StaticContentConfig>,
) {
    let mut result = Http1ResponseBuilder::new(static_content.status_code);

    if let Some(content_type) = static_content.content_type.as_ref() {
        result = result.add_content_type(content_type);
    }

    let content = result.build_with_body(&static_content.body.as_slice());

    let _ = connection_context
        .write_http_payload(
            connection_context.request_id,
            content.as_slice(),
            crate::consts::WRITE_TIMEOUT,
        )
        .await;

    connection_context.request_is_done().await;
}
