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
    /// the white-list is fully exempt. Attributable to a specific endpoint.
    ClientCertRequired {
        endpoint_host: String,
        message: String,
    },
    /// The ClientHello did not map to any configured endpoint — no SNI, or an
    /// SNI for a host we do not serve. Internet background noise; a few are
    /// harmless, but a flood is a scanner → soft failure. Not attributable.
    UnknownServerName(String),
    /// The peer never produced a valid ClientHello (non-TLS bytes on the TLS
    /// port, port scanner, garbage). Unambiguous abuse → hard failure. Not
    /// attributable.
    MalformedTls(String),
    /// Any other handshake failure (handshake abort, server misconfig, timeout,
    /// panic). Noisy → soft failure. `endpoint_host` is `Some` when the failure
    /// could be attributed to a resolved endpoint.
    Other {
        endpoint_host: Option<String>,
        message: String,
    },
    /// The endpoint matched, but its certificate is not loaded yet (e.g. a
    /// manually-provided cert still arriving in the background). Server-side and
    /// expected — cut the connection but do NOT penalise the waiting client.
    CertificateUnavailable {
        endpoint_host: String,
        message: String,
    },
}

impl TlsAcceptError {
    pub fn message(&self) -> &str {
        match self {
            Self::UnknownServerName(msg) | Self::MalformedTls(msg) => msg.as_str(),
            Self::ClientCertRequired { message, .. }
            | Self::Other { message, .. }
            | Self::CertificateUnavailable { message, .. } => message.as_str(),
        }
    }

    /// The endpoint this rejection is attributable to, when known. Used to also
    /// surface the rejection in that endpoint's (debug) log.
    pub fn endpoint_host(&self) -> Option<&str> {
        match self {
            Self::ClientCertRequired { endpoint_host, .. }
            | Self::CertificateUnavailable { endpoint_host, .. } => Some(endpoint_host.as_str()),
            Self::Other { endpoint_host, .. } => endpoint_host.as_deref(),
            Self::UnknownServerName(_) | Self::MalformedTls(_) => None,
        }
    }

    /// Whether the source IP should be penalised in the auto block-list for this
    /// rejection. Server-side conditions (the certificate simply isn't loaded yet)
    /// are the endpoint operator's problem, not the client's — the client is
    /// expected to keep retrying while the cert arrives, so it must not be blocked.
    pub fn should_penalise_client(&self) -> bool {
        !matches!(self, Self::CertificateUnavailable { .. })
    }

    /// How this rejection counts toward the auto IP block-list. Every rejection
    /// counts; the white-list (enforced in `register_failure`) is the only full
    /// exemption.
    pub fn block_severity(&self) -> FailureSeverity {
        match self {
            Self::MalformedTls(_) => FailureSeverity::Hard,
            Self::ClientCertRequired { .. }
            | Self::UnknownServerName(_)
            | Self::CertificateUnavailable { .. }
            | Self::Other { .. } => FailureSeverity::Soft,
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
        return Err(TlsAcceptError::Other {
            endpoint_host: None,
            message: format!(
                "Accepting TLS connection timeout for port: {}",
                endpoint_port
            ),
        });
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
                let endpoint_host =
                    match configuration.get_http_endpoint_info(Some(server_name.as_str())) {
                        Some(endpoint_info) => endpoint_info.host_endpoint.as_str().to_string(),
                        None => {
                            return Err(TlsAcceptError::UnknownServerName(format!(
                                "server name '{server_name}' is not configured on this port"
                            )));
                        }
                    };

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

                let (config, endpoint_info, client_cert_cell) = match config_result {
                    Ok(result) => result,
                    Err(super::tls_acceptor::CreateConfigError::CertificateUnavailable {
                        endpoint_host,
                        message,
                    }) => {
                        return Err(TlsAcceptError::CertificateUnavailable {
                            endpoint_host,
                            message: format!(
                                "certificate for '{server_name}' is not available yet: {message}"
                            ),
                        });
                    }
                    Err(super::tls_acceptor::CreateConfigError::Other(msg)) => {
                        return Err(TlsAcceptError::Other {
                            endpoint_host: Some(endpoint_host.clone()),
                            message: format!(
                                "Failed to create tls config for '{server_name}'. Err: {msg}"
                            ),
                        });
                    }
                };

                let tls_stream = start_handshake.into_stream(config.into()).await;

                if let Err(err) = &tls_stream {
                    // When the endpoint requires a client certificate, a failed
                    // handshake is almost always the client not presenting a
                    // valid cert — an expected condition we must not penalise.
                    if client_cert_cell.is_some() {
                        return Err(TlsAcceptError::ClientCertRequired {
                            endpoint_host: endpoint_host.clone(),
                            message: format!(
                                "failed to perform tls handshake for '{server_name}': {err:#} (endpoint requires a client certificate / mTLS)"
                            ),
                        });
                    }
                    return Err(TlsAcceptError::Other {
                        endpoint_host: Some(endpoint_host.clone()),
                        message: format!(
                            "failed to perform tls handshake for '{server_name}': {err:#}"
                        ),
                    });
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
            Err(TlsAcceptError::Other {
                endpoint_host: None,
                message: format!("tls handshake panicked: {msg}"),
            })
        }
    }
}
