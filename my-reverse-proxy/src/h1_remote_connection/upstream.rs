use std::time::Duration;

use crate::configurations::*;

use crate::h1_proxy_server::H1Writer;

use crate::{
    network_stream::*, tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream,
};

use super::*;

/// One upstream connection's write side, one variant per transport.
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
}

pub struct Upstream {
    pub inner: UpstreamInner,
    pub connection_id: u64,
}

/// A network upstream whose response READ half is handed to the caller (the
/// pipeline worker reads the response itself). The 5 transports have 5 concrete
/// read-half types, so the read half is type-erased behind
/// `Box<dyn NetworkStreamReadPart>`.
pub struct OwnedUpstream {
    pub upstream: Upstream,
    pub response_read: Box<dyn NetworkStreamReadPart + Send + Sync>,
    /// Shared flag the worker flips on EOF/read error so the pool can evict it.
    pub disconnect_trigger: std::sync::Arc<rust_extensions::UnsafeValue<bool>>,
    /// Kept alive for the lifetime of the read (ssh transports only).
    pub ssh_handler: Option<crate::app::SshSessionHandler>,
}

impl Upstream {
    /// Dial a network upstream and return its write side + the (type-erased)
    /// response read half + disconnect trigger + ssh handler. The caller drives
    /// the response read. Only network upstreams reach here — Static / LocalFiles
    /// / Drop / DynamicProxy are handled by the reader before the pool.
    pub async fn connect_owned(
        proxy_pass_to: &ProxyPassToConfig,
    ) -> Result<OwnedUpstream, NetworkError> {
        let connection_id = super::CONN_ID.get_next();

        macro_rules! owned {
            ($inner:expr, $result:expr, $read_part:expr, $ssh:expr) => {{
                let disconnect_trigger = $result.get_remote_disconnect_trigger();
                return Ok(OwnedUpstream {
                    upstream: Self {
                        connection_id,
                        inner: $inner,
                    },
                    response_read: Box::new($read_part),
                    disconnect_trigger,
                    ssh_handler: $ssh,
                });
            }};
        }

        match proxy_pass_to {
            ProxyPassToConfig::Http1(proxy_pass_to)
            | ProxyPassToConfig::McpHttp1(proxy_pass_to) => match &proxy_pass_to.remote_host {
                MyReverseProxyRemoteEndpoint::Gateway { id, remote_host } => {
                    let (result, read_part, ssh_handler) = Http1ConnectionInner::connect::<
                        TcpGatewayProxyForwardStream,
                        TcpGatewayProxyForwardStream,
                    >(
                        Some(id), None, None, remote_host, proxy_pass_to.connect_timeout
                    )
                    .await?;
                    owned!(
                        UpstreamInner::Http1OverGateway(result),
                        result,
                        read_part,
                        ssh_handler
                    );
                }
                MyReverseProxyRemoteEndpoint::OverSsh {
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
                    owned!(
                        UpstreamInner::Http1OverSsh(result),
                        result,
                        read_part,
                        ssh_handler
                    );
                }
                MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
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
                            owned!(
                                UpstreamInner::Https1Direct(result),
                                result,
                                read_part,
                                ssh_handler
                            );
                        }
                    }
                    let (result, read_part, ssh_handler) = Http1ConnectionInner::connect::<
                        tokio::io::ReadHalf<tokio::net::TcpStream>,
                        tokio::net::TcpStream,
                    >(
                        None, None, None, remote_host, proxy_pass_to.connect_timeout
                    )
                    .await?;
                    owned!(
                        UpstreamInner::Http1Direct(result),
                        result,
                        read_part,
                        ssh_handler
                    );
                }
            },
            ProxyPassToConfig::UnixHttp1(proxy_pass_to) => match &proxy_pass_to.remote_host {
                MyReverseProxyRemoteEndpoint::Direct { remote_host } => {
                    let (result, read_part, ssh_handler) = Http1ConnectionInner::connect::<
                        tokio::io::ReadHalf<tokio::net::UnixStream>,
                        tokio::net::UnixStream,
                    >(
                        None, None, None, remote_host, proxy_pass_to.connect_timeout
                    )
                    .await?;
                    owned!(
                        UpstreamInner::Http1UnixSocket(result),
                        result,
                        read_part,
                        ssh_handler
                    );
                }
                _ => todo!("UnixHttp1 over gateway/ssh not implemented"),
            },
            ProxyPassToConfig::Http2(_) => todo!("Http2 temporary is disabled"),
            ProxyPassToConfig::UnixHttp2(_) => todo!("Not Implemented"),
            ProxyPassToConfig::FilesPath(_)
            | ProxyPassToConfig::Static(_)
            | ProxyPassToConfig::Drop
            | ProxyPassToConfig::DynamicProxy(_) => {
                unreachable!(
                    "connect_owned is only for network upstreams; Static/LocalFiles/Drop/DynamicProxy are handled by the reader before the pool"
                );
            }
        }
    }

    /// Remote end is known to be gone — the flag is set by the worker when it
    /// observes EOF or a read error on the response.
    pub fn is_disconnected(&self) -> bool {
        match &self.inner {
            UpstreamInner::Http1Direct(c) => c.is_disconnected(),
            UpstreamInner::Http1UnixSocket(c) => c.is_disconnected(),
            UpstreamInner::Https1Direct(c) => c.is_disconnected(),
            UpstreamInner::Http1OverSsh(c) => c.is_disconnected(),
            UpstreamInner::Http1OverGateway(c) => c.is_disconnected(),
        }
    }

    /// Send an already-compiled request head (raw bytes) to the upstream. Returns
    /// false if the connection is gone or the write fails.
    pub async fn send_head_bytes(&mut self, head: &[u8], time_out: Duration) -> bool {
        macro_rules! send {
            ($c:expr) => {{
                if $c.is_disconnected() {
                    return false;
                }
                $c.send_with_timeout(head, time_out).await.is_ok()
            }};
        }
        match &mut self.inner {
            UpstreamInner::Http1Direct(c) => send!(c),
            UpstreamInner::Http1UnixSocket(c) => send!(c),
            UpstreamInner::Https1Direct(c) => send!(c),
            UpstreamInner::Http1OverSsh(c) => send!(c),
            UpstreamInner::Http1OverGateway(c) => send!(c),
        }
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
            UpstreamInner::Http1Direct(inner) => inner.send_with_timeout(buffer, timeout).await,
            UpstreamInner::Http1UnixSocket(inner) => inner.send_with_timeout(buffer, timeout).await,
            UpstreamInner::Https1Direct(inner) => inner.send_with_timeout(buffer, timeout).await,
            UpstreamInner::Http1OverSsh(inner) => inner.send_with_timeout(buffer, timeout).await,
            UpstreamInner::Http1OverGateway(inner) => inner.send_with_timeout(buffer, timeout).await,
        }
    }
}
