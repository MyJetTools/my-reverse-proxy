use crate::settings::OAuthSettings;

use super::{
    constant_time_eq, AuthCodes, TokenKind, TokenSigner, DEFAULT_ACCESS_TOKEN_TTL_SEC,
    DEFAULT_REFRESH_TOKEN_TTL_SEC,
};

/// The scope this server grants. One scope, because there is one thing to
/// authorize: reaching the endpoint behind the proxy.
pub const MCP_SCOPE: &str = "mcp";

/// Requested by Claude when the metadata advertises it, and required for a
/// refresh token to be issued.
pub const OFFLINE_SCOPE: &str = "offline_access";

/// One configured `oauth:` block, live: the credentials to check against, the
/// key to sign with, and the authorization codes currently in flight.
pub struct OAuthContext {
    /// Kept so a settings reload can tell whether anything actually changed —
    /// rebuilding on every reload would throw away in-flight codes.
    settings: OAuthSettings,
    public_url: Option<String>,
    access_token_ttl_sec: i64,
    refresh_token_ttl_sec: i64,
    pub signer: TokenSigner,
    pub codes: AuthCodes,
}

impl OAuthContext {
    pub fn new(settings: &OAuthSettings, signing_key: Vec<u8>) -> Self {
        Self {
            public_url: settings
                .public_url
                .as_ref()
                .map(|url| url.trim().trim_end_matches('/').to_string()),
            access_token_ttl_sec: settings
                .access_token_ttl_sec
                .unwrap_or(DEFAULT_ACCESS_TOKEN_TTL_SEC),
            refresh_token_ttl_sec: settings
                .refresh_token_ttl_sec
                .unwrap_or(DEFAULT_REFRESH_TOKEN_TTL_SEC),
            signer: TokenSigner::new(signing_key),
            codes: AuthCodes::new(),
            settings: settings.clone(),
        }
    }

    pub fn settings_are_the_same(&self, other: &OAuthSettings) -> bool {
        &self.settings == other
    }

    /// The issuer for this request: the configured `public_url` when there is
    /// one, otherwise the endpoint's own scheme and `Host`.
    pub fn base_url<'s>(&'s self, request_base_url: &'s str) -> &'s str {
        match self.public_url.as_deref() {
            Some(public_url) => public_url,
            None => request_base_url.trim_end_matches('/'),
        }
    }

    pub fn check_client_id(&self, presented: &str) -> bool {
        constant_time_eq(
            presented.trim().as_bytes(),
            self.settings.client_id.trim().as_bytes(),
        )
    }

    pub fn check_client_secret(&self, presented: &str) -> bool {
        constant_time_eq(
            presented.as_bytes(),
            self.settings.client_secret.trim().as_bytes(),
        )
    }

    pub fn check_consent_password(&self, presented: &str) -> bool {
        constant_time_eq(
            presented.trim().as_bytes(),
            self.settings.consent_password.trim().as_bytes(),
        )
    }

    /// Narrows whatever the client asked for down to what this server grants.
    pub fn granted_scope(&self, requested: Option<&str>) -> String {
        let requested = requested.unwrap_or_default();

        let mut granted = vec![MCP_SCOPE];

        if scope_includes(requested, OFFLINE_SCOPE) {
            granted.push(OFFLINE_SCOPE);
        }

        granted.join(" ")
    }

    /// Mints the pair returned from the token endpoint. A refresh token is only
    /// issued when `offline_access` was granted, which is what Claude asks for.
    pub fn mint_tokens(&self, scope: &str, audience: Option<&str>) -> Result<MintedTokens, String> {
        let access_token = self.signer.mint(
            TokenKind::Access,
            scope,
            audience,
            self.access_token_ttl_sec,
        )?;

        let refresh_token = if scope_includes(scope, OFFLINE_SCOPE) {
            Some(self.signer.mint(
                TokenKind::Refresh,
                scope,
                audience,
                self.refresh_token_ttl_sec,
            )?)
        } else {
            None
        };

        Ok(MintedTokens {
            access_token,
            refresh_token,
            expires_in: self.access_token_ttl_sec,
            scope: scope.to_string(),
        })
    }
}

pub struct MintedTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
    pub scope: String,
}

fn scope_includes(scope: &str, wanted: &str) -> bool {
    scope.split_whitespace().any(|entry| entry == wanted)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn settings() -> OAuthSettings {
        OAuthSettings {
            client_id: "claude".to_string(),
            client_secret: "the-secret".to_string(),
            consent_password: "the-password".to_string(),
            public_url: None,
            signing_key: None,
            signing_key_file: None,
            access_token_ttl_sec: None,
            refresh_token_ttl_sec: None,
        }
    }

    fn context() -> OAuthContext {
        OAuthContext::new(&settings(), b"a-test-signing-key".to_vec())
    }

    #[test]
    fn the_issuer_falls_back_to_the_request_host() {
        let context = context();

        assert_eq!(
            context.base_url("https://mcp-home.jetdev.eu"),
            "https://mcp-home.jetdev.eu"
        );
    }

    #[test]
    fn a_configured_public_url_wins_and_loses_its_trailing_slash() {
        let context = OAuthContext::new(
            &OAuthSettings {
                public_url: Some("https://public.example/".to_string()),
                ..settings()
            },
            b"key".to_vec(),
        );

        assert_eq!(
            context.base_url("https://internal.example"),
            "https://public.example"
        );
    }

    #[test]
    fn client_credentials_are_checked() {
        let context = context();

        assert!(context.check_client_id("claude"));
        assert!(!context.check_client_id("other"));
        assert!(context.check_client_secret("the-secret"));
        assert!(!context.check_client_secret("wrong"));
        assert!(context.check_consent_password("the-password"));
        assert!(!context.check_consent_password("wrong"));
    }

    #[test]
    fn scope_is_narrowed_to_what_is_granted() {
        let context = context();

        assert_eq!(context.granted_scope(None), "mcp");
        assert_eq!(context.granted_scope(Some("mcp")), "mcp");
        assert_eq!(
            context.granted_scope(Some("mcp offline_access")),
            "mcp offline_access"
        );
        // Anything unknown is dropped rather than echoed back.
        assert_eq!(context.granted_scope(Some("admin root")), "mcp");
    }

    #[test]
    fn a_refresh_token_comes_only_with_offline_access() {
        let context = context();

        assert!(context
            .mint_tokens("mcp", None)
            .unwrap()
            .refresh_token
            .is_none());
        assert!(context
            .mint_tokens("mcp offline_access", None)
            .unwrap()
            .refresh_token
            .is_some());
    }

    #[test]
    fn token_lifetimes_can_be_configured() {
        let context = OAuthContext::new(
            &OAuthSettings {
                access_token_ttl_sec: Some(120),
                ..settings()
            },
            b"key".to_vec(),
        );

        assert_eq!(context.mint_tokens("mcp", None).unwrap().expires_in, 120);
    }

    #[test]
    fn a_reload_that_changes_nothing_is_recognised() {
        let context = context();

        assert!(context.settings_are_the_same(&settings()));
        assert!(!context.settings_are_the_same(&OAuthSettings {
            client_secret: "rotated".to_string(),
            ..settings()
        }));
    }
}
