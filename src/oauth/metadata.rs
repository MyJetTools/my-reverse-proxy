use serde::Serialize;

use super::{
    MCP_SCOPE, OFFLINE_SCOPE, {AUTHORIZE_PATH, TOKEN_PATH},
};

/// RFC 8414 authorization server metadata.
///
/// `code_challenge_methods_supported` is not optional in practice: Claude treats
/// its absence as "this server does not do PKCE" and refuses to start the flow.
#[derive(Serialize)]
struct AuthorizationServerMetadata<'s> {
    issuer: &'s str,
    authorization_endpoint: String,
    token_endpoint: String,
    response_types_supported: Vec<&'s str>,
    grant_types_supported: Vec<&'s str>,
    code_challenge_methods_supported: Vec<&'s str>,
    token_endpoint_auth_methods_supported: Vec<&'s str>,
    scopes_supported: Vec<&'s str>,
}

/// RFC 9728 protected resource metadata.
///
/// `resource` has to be byte-for-byte the URL the user typed into the connector
/// dialog, path included — Claude compares them and aborts on a mismatch.
#[derive(Serialize)]
struct ProtectedResourceMetadata<'s> {
    resource: String,
    authorization_servers: Vec<&'s str>,
    scopes_supported: Vec<&'s str>,
    bearer_methods_supported: Vec<&'s str>,
}

pub fn build_authorization_server_metadata(base_url: &str) -> Vec<u8> {
    let metadata = AuthorizationServerMetadata {
        issuer: base_url,
        authorization_endpoint: format!("{}{}", base_url, AUTHORIZE_PATH),
        token_endpoint: format!("{}{}", base_url, TOKEN_PATH),
        response_types_supported: vec!["code"],
        grant_types_supported: vec!["authorization_code", "refresh_token"],
        // S256 only — OAuth 2.1 removed `plain` and it protects nothing.
        code_challenge_methods_supported: vec!["S256"],
        token_endpoint_auth_methods_supported: vec!["client_secret_post", "client_secret_basic"],
        // `offline_access` is advertised so Claude asks for it and gets a
        // refresh token; without it the connector re-prompts every hour.
        scopes_supported: vec![MCP_SCOPE, OFFLINE_SCOPE],
    };

    serialize(&metadata)
}

pub fn build_protected_resource_metadata(base_url: &str, resource_path: &str) -> Vec<u8> {
    let metadata = ProtectedResourceMetadata {
        resource: build_resource_url(base_url, resource_path),
        // Claude reads only the first entry.
        authorization_servers: vec![base_url],
        scopes_supported: vec![MCP_SCOPE, OFFLINE_SCOPE],
        bearer_methods_supported: vec!["header"],
    };

    serialize(&metadata)
}

/// The `resource` identifier of one protected path — the URL the user typed.
pub fn build_resource_url(base_url: &str, resource_path: &str) -> String {
    if resource_path == "/" || resource_path.is_empty() {
        return base_url.to_string();
    }

    format!("{}{}", base_url, resource_path)
}

/// Where the protected resource metadata for one path lives.
pub fn build_resource_metadata_url(base_url: &str, resource_path: &str) -> String {
    if resource_path == "/" || resource_path.is_empty() {
        return format!("{}{}", base_url, super::PROTECTED_RESOURCE_METADATA_PATH);
    }

    format!(
        "{}{}{}",
        base_url,
        super::PROTECTED_RESOURCE_METADATA_PATH,
        resource_path
    )
}

fn serialize(value: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_else(|_| {
        super::error_body(
            super::ERROR_SERVER_ERROR,
            "Can not build the metadata document",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(bytes: &[u8]) -> serde_json::Value {
        serde_json::from_slice(bytes).unwrap()
    }

    #[test]
    fn the_authorization_server_metadata_has_what_claude_requires() {
        let metadata = parse(&build_authorization_server_metadata(
            "https://mcp-home.jetdev.eu",
        ));

        assert_eq!(metadata["issuer"], "https://mcp-home.jetdev.eu");
        assert_eq!(
            metadata["authorization_endpoint"],
            "https://mcp-home.jetdev.eu/oauth/authorize"
        );
        assert_eq!(
            metadata["token_endpoint"],
            "https://mcp-home.jetdev.eu/oauth/token"
        );
        // Without this Claude refuses to start the flow at all.
        assert_eq!(metadata["code_challenge_methods_supported"][0], "S256");
        assert_eq!(
            metadata["code_challenge_methods_supported"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(metadata["response_types_supported"][0], "code");
    }

    #[test]
    fn offline_access_is_advertised_so_a_refresh_token_is_requested() {
        let metadata = parse(&build_authorization_server_metadata(
            "https://mcp-home.jetdev.eu",
        ));

        let scopes = metadata["scopes_supported"].as_array().unwrap();

        assert!(scopes.iter().any(|scope| scope == "offline_access"));
    }

    #[test]
    fn the_resource_matches_the_url_the_user_typed() {
        let metadata = parse(&build_protected_resource_metadata(
            "https://mcp-home.jetdev.eu",
            "/mt-risks",
        ));

        assert_eq!(metadata["resource"], "https://mcp-home.jetdev.eu/mt-risks");
        assert_eq!(
            metadata["authorization_servers"][0],
            "https://mcp-home.jetdev.eu"
        );
        assert_eq!(metadata["bearer_methods_supported"][0], "header");
    }

    #[test]
    fn a_root_resource_has_no_trailing_slash() {
        let metadata = parse(&build_protected_resource_metadata(
            "https://mcp-home.jetdev.eu",
            "/",
        ));

        assert_eq!(metadata["resource"], "https://mcp-home.jetdev.eu");
    }

    #[test]
    fn the_metadata_url_carries_the_resource_path_as_a_suffix() {
        assert_eq!(
            build_resource_metadata_url("https://mcp-home.jetdev.eu", "/mt-risks"),
            "https://mcp-home.jetdev.eu/.well-known/oauth-protected-resource/mt-risks"
        );
        assert_eq!(
            build_resource_metadata_url("https://mcp-home.jetdev.eu", "/"),
            "https://mcp-home.jetdev.eu/.well-known/oauth-protected-resource"
        );
    }
}
