/// RFC 8414 — authorization server metadata.
pub const AS_METADATA_PATH: &str = "/.well-known/oauth-authorization-server";
/// The OpenID Connect discovery path. Not an OIDC provider, but the MCP client's
/// discovery chain probes it as a fallback, and answering with the same document
/// saves a failed round trip.
pub const OPENID_CONFIGURATION_PATH: &str = "/.well-known/openid-configuration";
/// RFC 9728 — protected resource metadata. One document per protected path, so
/// the path of the resource follows this prefix.
pub const PROTECTED_RESOURCE_METADATA_PATH: &str = "/.well-known/oauth-protected-resource";
pub const AUTHORIZE_PATH: &str = "/oauth/authorize";
pub const TOKEN_PATH: &str = "/oauth/token";

/// The endpoints the proxy answers itself when an endpoint has an `oauth:`
/// block. Everything else on the endpoint is proxied, behind the bearer gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthRoute {
    AuthorizationServerMetadata,
    ProtectedResourceMetadata {
        /// The path of the resource the document describes, normalised with a
        /// leading slash and no trailing one.
        resource_path: String,
    },
    Authorize,
    Token,
}

/// Decides whether a request path belongs to the OAuth server rather than to a
/// configured location.
///
/// Matched case-insensitively and before `find_location`, which is what makes
/// `/.well-known/...` reachable at all — it matches no location and would
/// otherwise be answered with the 503 "location is not found" page.
pub fn route_oauth_path(path: &str) -> Option<OAuthRoute> {
    if path.eq_ignore_ascii_case(AUTHORIZE_PATH) {
        return Some(OAuthRoute::Authorize);
    }

    if path.eq_ignore_ascii_case(TOKEN_PATH) {
        return Some(OAuthRoute::Token);
    }

    // RFC 8414 places the metadata of an issuer that has a path component at
    // `/.well-known/oauth-authorization-server/<path>`. This issuer has no path,
    // but clients probe the suffixed form too, so both are answered.
    if matches_with_suffix(path, AS_METADATA_PATH).is_some()
        || matches_with_suffix(path, OPENID_CONFIGURATION_PATH).is_some()
    {
        return Some(OAuthRoute::AuthorizationServerMetadata);
    }

    if let Some(suffix) = matches_with_suffix(path, PROTECTED_RESOURCE_METADATA_PATH) {
        return Some(OAuthRoute::ProtectedResourceMetadata {
            resource_path: normalize_resource_path(suffix),
        });
    }

    None
}

/// `Some(suffix)` when `path` is `prefix`, or `prefix` followed by a further
/// path segment. A prefix that merely shares a leading substring (e.g.
/// `/.well-known/oauth-authorization-server-evil`) does not match.
fn matches_with_suffix<'s>(path: &'s str, prefix: &str) -> Option<&'s str> {
    if path.len() < prefix.len() {
        return None;
    }

    if !path[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }

    let suffix = &path[prefix.len()..];

    if suffix.is_empty() || suffix.starts_with('/') {
        Some(suffix)
    } else {
        None
    }
}

/// Brings a resource path to the shape location paths are written in: a leading
/// slash, and no trailing one except for the root.
pub fn normalize_resource_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');

    if trimmed.is_empty() {
        return "/".to_string();
    }

    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_oauth_endpoints_are_routed() {
        assert_eq!(
            route_oauth_path("/oauth/authorize"),
            Some(OAuthRoute::Authorize)
        );
        assert_eq!(route_oauth_path("/oauth/token"), Some(OAuthRoute::Token));
        assert_eq!(
            route_oauth_path("/OAuth/Token"),
            Some(OAuthRoute::Token),
            "paths are matched case-insensitively, like find_location"
        );
    }

    #[test]
    fn authorization_server_metadata_is_routed_with_or_without_a_suffix() {
        assert_eq!(
            route_oauth_path("/.well-known/oauth-authorization-server"),
            Some(OAuthRoute::AuthorizationServerMetadata)
        );
        assert_eq!(
            route_oauth_path("/.well-known/oauth-authorization-server/mt-risks"),
            Some(OAuthRoute::AuthorizationServerMetadata)
        );
        assert_eq!(
            route_oauth_path("/.well-known/openid-configuration"),
            Some(OAuthRoute::AuthorizationServerMetadata)
        );
    }

    #[test]
    fn protected_resource_metadata_carries_the_resource_path() {
        assert_eq!(
            route_oauth_path("/.well-known/oauth-protected-resource/mt-risks"),
            Some(OAuthRoute::ProtectedResourceMetadata {
                resource_path: "/mt-risks".to_string()
            })
        );
        // The root resource — RFC 9728 allows the bare well-known path.
        assert_eq!(
            route_oauth_path("/.well-known/oauth-protected-resource"),
            Some(OAuthRoute::ProtectedResourceMetadata {
                resource_path: "/".to_string()
            })
        );
        assert_eq!(
            route_oauth_path("/.well-known/oauth-protected-resource/mt-risks/"),
            Some(OAuthRoute::ProtectedResourceMetadata {
                resource_path: "/mt-risks".to_string()
            })
        );
    }

    #[test]
    fn everything_else_is_left_to_the_locations() {
        assert!(route_oauth_path("/mt-risks").is_none());
        assert!(route_oauth_path("/").is_none());
        assert!(route_oauth_path("/oauth").is_none());
        assert!(route_oauth_path("/oauth/authorize/extra").is_none());
    }

    #[test]
    fn a_lookalike_prefix_is_not_the_well_known_path() {
        assert!(route_oauth_path("/.well-known/oauth-protected-resource-evil").is_none());
        assert!(route_oauth_path("/.well-known/oauth-authorization-server-evil").is_none());
    }

    #[test]
    fn resource_paths_normalise_to_the_shape_locations_use() {
        assert_eq!(normalize_resource_path("/mt-risks"), "/mt-risks");
        assert_eq!(normalize_resource_path("mt-risks"), "/mt-risks");
        assert_eq!(normalize_resource_path("/mt-risks/"), "/mt-risks");
        assert_eq!(normalize_resource_path(""), "/");
        assert_eq!(normalize_resource_path("/"), "/");
    }
}
