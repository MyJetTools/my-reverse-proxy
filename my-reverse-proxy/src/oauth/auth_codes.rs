use parking_lot::Mutex;

use super::{now_unix_seconds, AuthCode, AuthCodesInner, AUTH_CODE_TTL_SEC};

/// What the consent screen agreed to, on its way into the code store.
pub struct IssuedAuthCode {
    pub redirect_uri: String,
    pub code_challenge: String,
    pub scope: String,
    pub resource: Option<String>,
}

/// The one piece of OAuth state that lives only in memory.
///
/// It does not need to survive a restart: a code is redeemed seconds after it is
/// issued, so the worst a restart mid-flow costs is one retry of the consent
/// screen — unlike the tokens, which are signed and therefore restart-proof.
///
/// `parking_lot` because nothing is awaited under the lock.
pub struct AuthCodes {
    inner: Mutex<AuthCodesInner>,
}

impl AuthCodes {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(AuthCodesInner::new()),
        }
    }

    pub fn issue(&self, code: String, issued: IssuedAuthCode) {
        self.inner.lock().add(
            code,
            AuthCode {
                redirect_uri: issued.redirect_uri,
                code_challenge: issued.code_challenge,
                scope: issued.scope,
                resource: issued.resource,
                expires_at_sec: now_unix_seconds() + AUTH_CODE_TTL_SEC,
            },
        );
    }

    pub fn redeem(&self, code: &str) -> Option<AuthCode> {
        self.inner.lock().take(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_issued_code_round_trips_once() {
        let codes = AuthCodes::new();

        codes.issue(
            "the-code".to_string(),
            IssuedAuthCode {
                redirect_uri: "https://claude.ai/api/mcp/auth_callback".to_string(),
                code_challenge: "challenge".to_string(),
                scope: "mcp offline_access".to_string(),
                resource: Some("https://mcp-home.jetdev.eu/mt-risks".to_string()),
            },
        );

        let redeemed = codes.redeem("the-code").unwrap();

        assert_eq!(redeemed.scope, "mcp offline_access");
        assert_eq!(
            redeemed.resource.as_deref(),
            Some("https://mcp-home.jetdev.eu/mt-risks")
        );
        assert!(codes.redeem("the-code").is_none());
    }
}
