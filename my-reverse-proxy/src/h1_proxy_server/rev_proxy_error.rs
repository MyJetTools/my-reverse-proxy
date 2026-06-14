#![allow(warnings)]
use my_ssh::ssh2::DisconnectCode::ProtocolError;

use crate::{google_auth::GoogleAuthError, network_stream::NetworkError};

#[derive(Debug)]
pub enum ProxyServerError {
    NetworkError(NetworkError),
    ParsingPayloadError(&'static str),
    BufferAllocationFail,
    ChunkHeaderParseError,
    HeadersParseError(&'static str),
    CanNotConnectToRemoteResource {
        remote_resource: String,
        err: NetworkError,
    },
    CanNotWriteContentToRemoteConnection(NetworkError),
    HttpConfigurationIsNotFound,
    LocationIsNotFound,
    NotAuthorized,
    DropConnection,
    HttpResponse(Vec<u8>),
    ProxyToHeaderMissing,
    ProxyToHeaderInvalid,
    ProxyToHostNotAllowed,
}

/// Everything `serve_reverse_proxy` needs to do with a failed request, derived
/// in one place from the error variant instead of being spread across several
/// `match`es in the request loop. Pure data — no I/O — so the whole
/// error-to-client mapping is unit-testable.
pub struct ErrorHandling<'a> {
    /// Bytes to write back to the client before closing, or `None` when the
    /// connection is simply dropped (desynced / dropped request, no page).
    pub page: Option<&'a [u8]>,
    /// `Some(code)` when this produced a 5xx page returned to the client — the
    /// caller records it in the proxy logs. `None` for 4xx / non-logged pages.
    pub status_5xx: Option<u16>,
    /// Whether the source IP should be penalised in the block-list (malformed /
    /// unroutable requests).
    pub register_ip_failure: bool,
}

impl ProxyServerError {
    /// Map this error to the client response + side effects. The request loop
    /// always closes the connection afterwards, so there is no "keep-alive"
    /// option here.
    pub fn error_handling(&self) -> ErrorHandling<'_> {
        use crate::error_templates as tpl;

        match self {
            // Transport gone / client dropped — nothing to send, just close.
            Self::NetworkError(_) | Self::DropConnection => ErrorHandling {
                page: None,
                status_5xx: None,
                register_ip_failure: false,
            },

            // Malformed or unroutable request — close and penalise the source.
            Self::HttpConfigurationIsNotFound
            | Self::ParsingPayloadError(_)
            | Self::ChunkHeaderParseError
            | Self::HeadersParseError(_) => ErrorHandling {
                page: None,
                status_5xx: None,
                register_ip_failure: true,
            },

            // Upstream could not be reached / written to → 5xx page.
            Self::BufferAllocationFail => ErrorHandling {
                page: Some(tpl::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()),
                status_5xx: Some(503),
                register_ip_failure: false,
            },
            Self::CanNotConnectToRemoteResource { err, .. } => {
                let page = if err.as_timeout().is_some() {
                    tpl::ERROR_TIMEOUT.as_slice()
                } else {
                    tpl::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()
                };
                ErrorHandling {
                    page: Some(page),
                    status_5xx: Some(503),
                    register_ip_failure: false,
                }
            }
            Self::CanNotWriteContentToRemoteConnection(_) => ErrorHandling {
                page: Some(tpl::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()),
                status_5xx: Some(503),
                register_ip_failure: false,
            },
            Self::LocationIsNotFound => ErrorHandling {
                page: Some(tpl::LOCATION_IS_NOT_FOUND.as_slice()),
                status_5xx: Some(503),
                register_ip_failure: false,
            },

            // Client-side rejections → non-5xx page, not logged as upstream 5xx.
            Self::NotAuthorized => ErrorHandling {
                page: Some(tpl::NOT_AUTHORIZED_PAGE.as_slice()),
                status_5xx: None,
                register_ip_failure: false,
            },
            Self::ProxyToHeaderMissing | Self::ProxyToHeaderInvalid => ErrorHandling {
                page: Some(tpl::PROXY_TO_HEADER_MISSING.as_slice()),
                status_5xx: None,
                register_ip_failure: false,
            },
            Self::ProxyToHostNotAllowed => ErrorHandling {
                page: Some(tpl::PROXY_TO_HOST_NOT_ALLOWED.as_slice()),
                status_5xx: None,
                register_ip_failure: false,
            },

            // A fully-formed response the proxy itself produced — pass it through.
            Self::HttpResponse(payload) => ErrorHandling {
                page: Some(payload.as_slice()),
                status_5xx: None,
                register_ip_failure: false,
            },
        }
    }

    pub fn can_be_printed_as_debug(&self) -> bool {
        match self {
            Self::HttpResponse(_) => {
                return false;
            }
            Self::LocationIsNotFound => {
                return false;
            }
            Self::NotAuthorized => {
                return false;
            }
            _ => {
                return true;
            }
        }
    }
}

impl From<NetworkError> for ProxyServerError {
    fn from(value: NetworkError) -> Self {
        Self::NetworkError(value)
    }
}

impl From<&'static str> for ProxyServerError {
    fn from(value: &'static str) -> Self {
        Self::ParsingPayloadError(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error_templates as tpl;
    use std::time::Duration;

    #[test]
    fn network_error_closes_silently() {
        let h = ProxyServerError::NetworkError(NetworkError::Disconnected).error_handling();
        assert!(h.page.is_none());
        assert_eq!(h.status_5xx, None);
        assert!(!h.register_ip_failure);
    }

    #[test]
    fn drop_connection_closes_silently() {
        let h = ProxyServerError::DropConnection.error_handling();
        assert!(h.page.is_none());
        assert_eq!(h.status_5xx, None);
        assert!(!h.register_ip_failure);
    }

    #[test]
    fn malformed_request_penalises_source_without_page() {
        for err in [
            ProxyServerError::HttpConfigurationIsNotFound,
            ProxyServerError::ParsingPayloadError("x"),
            ProxyServerError::ChunkHeaderParseError,
            ProxyServerError::HeadersParseError("x"),
        ] {
            let h = err.error_handling();
            assert!(h.page.is_none(), "{:?}", err);
            assert_eq!(h.status_5xx, None);
            assert!(h.register_ip_failure, "{:?}", err);
        }
    }

    #[test]
    fn connect_failure_is_503_unavailable_page() {
        let err = ProxyServerError::CanNotConnectToRemoteResource {
            remote_resource: "http://x:1/".to_string(),
            err: NetworkError::Disconnected,
        };
        let h = err.error_handling();
        assert_eq!(h.status_5xx, Some(503));
        assert_eq!(h.page, Some(tpl::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()));
        assert!(!h.register_ip_failure);
    }

    #[test]
    fn connect_timeout_uses_timeout_page() {
        let err = ProxyServerError::CanNotConnectToRemoteResource {
            remote_resource: "http://x:1/".to_string(),
            err: NetworkError::Timeout(Duration::from_secs(1)),
        };
        let h = err.error_handling();
        assert_eq!(h.status_5xx, Some(503));
        assert_eq!(h.page, Some(tpl::ERROR_TIMEOUT.as_slice()));
    }

    #[test]
    fn write_failure_is_503_unavailable_page() {
        let h = ProxyServerError::CanNotWriteContentToRemoteConnection(NetworkError::Disconnected)
            .error_handling();
        assert_eq!(h.status_5xx, Some(503));
        assert_eq!(h.page, Some(tpl::REMOTE_RESOURCE_IS_NOT_AVAILABLE.as_slice()));
    }

    #[test]
    fn location_not_found_is_logged_503() {
        let h = ProxyServerError::LocationIsNotFound.error_handling();
        assert_eq!(h.status_5xx, Some(503));
        assert_eq!(h.page, Some(tpl::LOCATION_IS_NOT_FOUND.as_slice()));
    }

    #[test]
    fn not_authorized_has_page_but_is_not_logged_5xx() {
        let h = ProxyServerError::NotAuthorized.error_handling();
        assert_eq!(h.status_5xx, None);
        assert_eq!(h.page, Some(tpl::NOT_AUTHORIZED_PAGE.as_slice()));
        assert!(!h.register_ip_failure);
    }

    #[test]
    fn http_response_passes_payload_through() {
        let payload = b"HTTP/1.1 418 I am a teapot\r\n\r\n".to_vec();
        let err = ProxyServerError::HttpResponse(payload.clone());
        let h = err.error_handling();
        assert_eq!(h.status_5xx, None);
        assert_eq!(h.page, Some(payload.as_slice()));
    }
}
