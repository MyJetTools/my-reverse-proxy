use std::sync::Arc;

use crate::{http_proxy_pass::HttpProxyPassIdentity, settings::HttpEndpointModifyHeadersSettings};

use super::*;

pub enum AuthorizationRequired<'s> {
    GoogleAuth(&'s str),
    ClientCertificate,
}

pub struct HttpEndpointInfo {
    pub host_endpoint: EndpointHttpHostString,
    pub debug: bool,
    pub inject_country: bool,
    pub listen_endpoint_type: ListenHttpEndpointType,
    pub g_auth: Option<String>,
    /// Name of the `oauth:` block gating this endpoint, when it has one. The
    /// endpoint then also answers the OAuth server's own paths itself.
    pub oauth: Option<String>,
    pub ssl_certificate_id: Option<SslCertificateId>,
    pub client_certificate_id: Option<SslCertificateId>,
    pub locations: Vec<Arc<ProxyPassLocationConfig>>,
    pub allowed_user_list_id: Option<String>,
    pub modify_request_headers: ModifyHeadersConfig,
    pub modify_response_headers: ModifyHeadersConfig,
    pub whitelisted_ip_list_id: Option<String>,
    pub keep_alive: bool,
    pub track_metrics_by_all_domains: bool,
    pub hsts: bool,
    pub mcp_settings: McpEndpointSettings,
    /// Endpoint-scoped transport read/write idle timeouts (resolved cascade,
    /// global → endpoint). Used by every byte pump of this endpoint.
    pub timeouts: crate::types::HttpTimeouts,
}

/// Everything an endpoint is compiled from.
///
/// A struct rather than a positional argument list: seven of these are
/// `Option<String>` and two more are `Option<SslCertificateId>`, so transposing
/// a pair — `g_auth` for `oauth`, the server certificate for the client CA —
/// compiles cleanly and quietly mis-secures the endpoint.
pub struct HttpEndpointInfoParams {
    pub host_endpoint: EndpointHttpHostString,
    pub listen_endpoint_type: ListenHttpEndpointType,
    pub debug: bool,
    pub inject_country: bool,
    pub g_auth: Option<String>,
    pub oauth: Option<String>,
    pub ssl_certificate_id: Option<SslCertificateId>,
    pub client_certificate_id: Option<SslCertificateId>,
    pub whitelisted_ip_list_id: Option<String>,
    pub locations: Vec<Arc<ProxyPassLocationConfig>>,
    pub allowed_user_list_id: Option<String>,
    pub modify_headers_settings: HttpEndpointModifyHeadersSettings,
    pub keep_alive: bool,
    pub track_metrics_by_all_domains: bool,
    pub hsts: bool,
    pub mcp_settings: McpEndpointSettings,
    pub timeouts: crate::types::HttpTimeouts,
}

impl HttpEndpointInfo {
    pub fn new(params: HttpEndpointInfoParams) -> Self {
        let HttpEndpointInfoParams {
            host_endpoint,
            listen_endpoint_type,
            debug,
            inject_country,
            g_auth,
            oauth,
            ssl_certificate_id,
            client_certificate_id,
            whitelisted_ip_list_id,
            locations,
            allowed_user_list_id,
            mut modify_headers_settings,
            keep_alive,
            track_metrics_by_all_domains,
            hsts,
            mcp_settings,
            timeouts,
        } = params;

        if debug {
            println!("Endpoint {} is in debug mode", host_endpoint.as_str());
        }

        let mut modify_request_headers =
            ModifyHeadersConfig::new_request(&mut modify_headers_settings);

        // The access token on an oauth-gated endpoint is one this proxy minted
        // for itself. Stripping it here — at the endpoint level, since location
        // level `modify_http_headers` is ignored on the h1 path — keeps it out
        // of the upstream, which the MCP authorization spec requires.
        if oauth.is_some() {
            modify_request_headers.add_to_remove("authorization");
        }

        Self {
            host_endpoint,
            debug,
            inject_country,
            listen_endpoint_type,
            g_auth,
            oauth,
            client_certificate_id,
            locations,
            allowed_user_list_id,
            modify_request_headers,
            modify_response_headers: ModifyHeadersConfig::new_response(
                &mut modify_headers_settings,
            ),
            ssl_certificate_id,
            whitelisted_ip_list_id,
            keep_alive,
            track_metrics_by_all_domains,
            hsts,
            mcp_settings,
            timeouts,
        }
    }

    /// Returns `Some(domain)` if this request should emit per-domain metrics,
    /// `None` otherwise. `request_host` is the value parsed from the inbound
    /// `Host:` header (or h2 `:authority`), already stripped of port.
    pub fn tracked_domain<'a>(&'a self, request_host: Option<&'a str>) -> Option<&'a str> {
        if self.track_metrics_by_all_domains {
            request_host
        } else {
            self.host_endpoint.get_server_name()
        }
    }

    pub fn is_my_endpoint(&self, server_name: &str) -> bool {
        self.host_endpoint.is_my_server_name(server_name)
    }

    pub fn as_str(&self) -> &str {
        self.host_endpoint.as_str()
    }

    pub fn find_location(&self, path: &str) -> Option<&ProxyPassLocationConfig> {
        for location in self.locations.iter() {
            if location.path.len() > path.len() {
                continue;
            }

            let path_prefix = &path[..location.path.len()];

            if path_prefix.eq_ignore_ascii_case(&location.path) {
                return Some(&location);
            }
        }

        None
    }

    /// Guards against HTTP/2 connection coalescing (and any cross-SNI
    /// `Host`/`:authority` reuse): mTLS is enforced only during the TLS
    /// handshake, which is bound to the SNI that opened the connection.
    /// `connection_cert_ca` is the `client_certificate_id` that verified the
    /// connection's client certificate (None — no certificate presented).
    /// Returns `false` when this endpoint requires a client certificate but the
    /// connection has none, OR has one verified by a DIFFERENT CA — presence
    /// alone is not enough: a certificate trusted by another vhost's CA says
    /// nothing about this endpoint's mTLS requirement. The caller answers 421
    /// so the client re-opens a dedicated connection and performs mTLS here.
    pub fn connection_satisfies_client_cert(&self, connection_cert_ca: Option<&str>) -> bool {
        match self.client_certificate_id.as_ref() {
            Some(required_ca) => connection_cert_ca == Some(required_ca.as_str()),
            None => true,
        }
    }

    /// Whether the connection's client-certificate identity (CN) is meaningful
    /// for THIS endpoint: only when the endpoint itself requires a client
    /// certificate from the same CA that verified it. A certificate presented
    /// for another vhost's handshake must not become an identity here — it
    /// would leak into `allowed_users` checks and `${CLIENT_CERT_CN}` headers
    /// for a vhost whose CA never saw that certificate. This also keeps a
    /// coalesced request's outcome identical to a dedicated-connection one
    /// (where a no-mTLS endpoint never asks for a certificate at all).
    pub fn client_cert_identity_applies(&self, connection_cert_ca: &str) -> bool {
        match self.client_certificate_id.as_ref() {
            Some(required_ca) => required_ca.as_str() == connection_cert_ca,
            None => false,
        }
    }

    pub fn must_be_authorized<'s>(&'s self) -> Option<AuthorizationRequired<'s>> {
        if let Some(g_auth) = self.g_auth.as_deref() {
            return Some(AuthorizationRequired::GoogleAuth(g_auth));
        }

        if self.ssl_certificate_id.is_some() {
            return Some(AuthorizationRequired::ClientCertificate);
        }

        None
    }

    pub async fn user_is_allowed(&self, identity: &Option<HttpProxyPassIdentity>) -> bool {
        let Some(allowed_user_list_id) = self.allowed_user_list_id.as_ref() else {
            return true;
        };

        let Some(identity) = identity else {
            return false;
        };

        crate::app::APP_CTX
            .allowed_users_list
            .is_allowed(allowed_user_list_id, identity.as_str())
            .await
    }
}
