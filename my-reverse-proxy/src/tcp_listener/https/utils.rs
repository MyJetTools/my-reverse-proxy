use std::{panic::AssertUnwindSafe, sync::Arc, time::Duration};

use futures::FutureExt;
use my_tls::tokio_rustls::{rustls::server::Acceptor, LazyConfigAcceptor};

use crate::{
    app::FailureSeverity,
    configurations::{HttpEndpointInfo, HttpListenPortConfiguration},
    tcp_listener::https::ClientCertificateData,
};

const RESOLVE_TLS_TIMEOUT: Duration = Duration::from_secs(10);

/// Why a TLS connection was rejected before it could be served. The block-list
/// policy is derived from the variant via [`TlsAcceptError::block_severity`].
pub enum TlsAcceptError {
    /// The endpoint requires a client certificate (mTLS) and the client
    /// presented none or an invalid one (a browser that hasn't selected a cert,
    /// an HTTP/2 coalesced connection, a probe). Lenient — soft failure; only
    /// the white-list is fully exempt.
    ClientCertRequired(String),
    /// The ClientHello did not map to any configured endpoint — no SNI, or an
    /// SNI for a host we do not serve. Internet background noise; a few are
    /// harmless, but a flood is a scanner → soft failure.
    UnknownServerName(String),
    /// The peer never produced a valid ClientHello (non-TLS bytes on the TLS
    /// port, port scanner, garbage). Unambiguous abuse → hard failure.
    MalformedTls(String),
    /// Any other handshake failure on a configured endpoint (handshake abort,
    /// server misconfig, timeout, panic). Noisy → soft failure.
    Other(String),
}

impl TlsAcceptError {
    pub fn message(&self) -> &str {
        match self {
            Self::ClientCertRequired(msg)
            | Self::UnknownServerName(msg)
            | Self::MalformedTls(msg)
            | Self::Other(msg) => msg.as_str(),
        }
    }

    /// How this rejection counts toward the auto IP block-list. Every rejection
    /// counts; the white-list (enforced in `register_failure`) is the only full
    /// exemption.
    pub fn block_severity(&self) -> FailureSeverity {
        match self {
            Self::MalformedTls(_) => FailureSeverity::Hard,
            Self::ClientCertRequired(_) | Self::UnknownServerName(_) | Self::Other(_) => {
                FailureSeverity::Soft
            }
        }
    }
}

pub async fn lazy_accept_tcp_stream(
    endpoint_port: u16,
    tcp_stream: tokio::net::TcpStream,
    configuration: Arc<HttpListenPortConfiguration>,
) -> Result<
    (
        my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
        Arc<HttpEndpointInfo>,
        Option<Arc<ClientCertificateData>>,
    ),
    TlsAcceptError,
> {
    let future = lazy_accept_tcp_stream_internal(endpoint_port, tcp_stream, configuration);

    let result = tokio::time::timeout(RESOLVE_TLS_TIMEOUT, future).await;

    if result.is_err() {
        return Err(TlsAcceptError::Other(format!(
            "Accepting TLS connection timeout for port: {}",
            endpoint_port
        )));
    }

    result.unwrap()
}

async fn lazy_accept_tcp_stream_internal(
    endpoint_port: u16,
    tcp_stream: tokio::net::TcpStream,
    configuration: Arc<HttpListenPortConfiguration>,
) -> Result<
    (
        my_tls::tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
        Arc<HttpEndpointInfo>,
        Option<Arc<ClientCertificateData>>,
    ),
    TlsAcceptError,
> {
    let handshake = async move {
        let lazy_acceptor = LazyConfigAcceptor::new(Acceptor::default(), tcp_stream);

        tokio::pin!(lazy_acceptor);

        let (tls_stream, endpoint_info, client_certificate) = match lazy_acceptor.as_mut().await {
            Ok(start_handshake) => {
                let client_hello = start_handshake.client_hello();
                let server_name = if let Some(server_name) = client_hello.server_name() {
                    server_name.to_string()
                } else {
                    return Err(TlsAcceptError::UnknownServerName(
                        "no server name (SNI) in client hello".to_string(),
                    ));
                };

                // SNI for a host we do not serve at all — unroutable noise, do
                // not penalise the source IP.
                if configuration
                    .get_http_endpoint_info(Some(server_name.as_str()))
                    .is_none()
                {
                    return Err(TlsAcceptError::UnknownServerName(format!(
                        "server name '{server_name}' is not configured on this port"
                    )));
                }

                if let Some(client_cert) = client_hello.client_cert_types() {
                    for client_cert in client_cert {
                        crate::app::APP_CTX.proxy_logs.write_port(
                            endpoint_port.to_string().as_str(),
                            None,
                            format!("Client_CERT: {:?}", client_cert.as_str()),
                        );
                    }
                }

                if let Some(ca) = client_hello.certificate_authorities() {
                    for cn in ca {
                        crate::app::APP_CTX.proxy_logs.write_port(
                            endpoint_port.to_string().as_str(),
                            None,
                            format!("DistName: {:?}", cn),
                        );
                    }
                }

                let config_result =
                    super::tls_acceptor::create_config(configuration, &server_name, endpoint_port)
                        .await;

                if let Err(err) = &config_result {
                    return Err(TlsAcceptError::Other(format!(
                        "Failed to create tls config for '{server_name}'. Err: {err:#}"
                    )));
                }

                let (config, endpoint_info, client_cert_cell) = config_result.unwrap();

                let tls_stream = start_handshake.into_stream(config.into()).await;

                if let Err(err) = &tls_stream {
                    // When the endpoint requires a client certificate, a failed
                    // handshake is almost always the client not presenting a
                    // valid cert — an expected condition we must not penalise.
                    if client_cert_cell.is_some() {
                        return Err(TlsAcceptError::ClientCertRequired(format!(
                            "failed to perform tls handshake for '{server_name}': {err:#} (endpoint requires a client certificate / mTLS)"
                        )));
                    }
                    return Err(TlsAcceptError::Other(format!(
                        "failed to perform tls handshake for '{server_name}': {err:#}"
                    )));
                }

                let tls_stream = tls_stream.unwrap();

                let client_certificate = if let Some(client_cert_cell) = client_cert_cell {
                    client_cert_cell.get()
                } else {
                    None
                };

                (tls_stream, endpoint_info, client_certificate)
            }
            Err(err) => {
                // Could not even parse a ClientHello — non-TLS traffic on the
                // TLS port / scanner. Unambiguous abuse → hard failure.
                return Err(TlsAcceptError::MalformedTls(format!(
                    "failed to perform tls handshake: {err:#}"
                )));
            }
        };

        Ok((tls_stream, endpoint_info, client_certificate))
    };

    match AssertUnwindSafe(handshake).catch_unwind().await {
        Ok(result) => result,
        Err(panic) => {
            let msg = if let Some(s) = panic.downcast_ref::<&'static str>() {
                (*s).to_string()
            } else if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic payload".to_string()
            };
            Err(TlsAcceptError::Other(format!(
                "tls handshake panicked: {msg}"
            )))
        }
    }
}
