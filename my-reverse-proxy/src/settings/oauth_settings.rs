use serde::*;

/// A named `oauth:` block — the proxy acting as an OAuth 2.1 authorization
/// server in front of one or more endpoints.
///
/// The client is pre-registered rather than dynamically registered: the user
/// types the same `client_id` / `client_secret` into the "Add custom connector"
/// dialog on claude.ai, so there is nothing for a registration endpoint to do.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct OAuthSettings {
    pub client_id: String,
    pub client_secret: String,
    /// What the user types on the consent screen to approve the connector.
    pub consent_password: String,
    /// Public base URL of the authorization server, without a trailing slash.
    /// Defaults to the request's own scheme + `Host`, which is right unless
    /// another proxy in front rewrites either.
    pub public_url: Option<String>,
    /// base64url key the tokens are signed with. Leave unset to have one
    /// generated and kept in `signing_key_file`.
    pub signing_key: Option<String>,
    /// Where a generated signing key is kept. Defaults to
    /// `~/.my-reverse-proxy-oauth/{block_id}.json`.
    pub signing_key_file: Option<String>,
    pub access_token_ttl_sec: Option<i64>,
    pub refresh_token_ttl_sec: Option<i64>,
}
