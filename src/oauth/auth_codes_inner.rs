use ahash::AHashMap;

use super::now_unix_seconds;

/// Authorization codes are exchanged within moments of being issued, so a short
/// life is both enough and what OAuth 2.1 asks for.
pub const AUTH_CODE_TTL_SEC: i64 = 300;

/// A guard against a stuck flow filling memory; far above any real use, since
/// codes live five minutes and one is issued per consent.
const MAX_CODES: usize = 256;

/// What was agreed when the user pressed "Approve", held until the code is
/// exchanged for tokens.
#[derive(Debug, Clone)]
pub struct AuthCode {
    pub redirect_uri: String,
    pub code_challenge: String,
    pub scope: String,
    /// The RFC 8707 `resource` the client asked for, echoed into the minted
    /// token's audience.
    pub resource: Option<String>,
    pub expires_at_sec: i64,
}

pub(super) struct AuthCodesInner {
    codes: AHashMap<String, AuthCode>,
}

impl AuthCodesInner {
    pub(super) fn new() -> Self {
        Self {
            codes: AHashMap::new(),
        }
    }

    pub(super) fn add(&mut self, code: String, issued: AuthCode) {
        self.remove_expired(now_unix_seconds());

        // Drop the oldest rather than refuse to issue: a fresh consent should
        // always work, and stale codes are worthless anyway.
        if self.codes.len() >= MAX_CODES {
            if let Some(oldest) = self.oldest_code() {
                self.codes.remove(&oldest);
            }
        }

        self.codes.insert(code, issued);
    }

    /// Takes the code out — single use by construction, so a replayed code
    /// finds nothing.
    pub(super) fn take(&mut self, code: &str) -> Option<AuthCode> {
        let now = now_unix_seconds();

        self.remove_expired(now);

        let issued = self.codes.remove(code)?;

        if issued.expires_at_sec <= now {
            return None;
        }

        Some(issued)
    }

    fn remove_expired(&mut self, now: i64) {
        self.codes.retain(|_, issued| issued.expires_at_sec > now);
    }

    fn oldest_code(&self) -> Option<String> {
        self.codes
            .iter()
            .min_by_key(|(_, issued)| issued.expires_at_sec)
            .map(|(code, _)| code.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(ttl: i64) -> AuthCode {
        AuthCode {
            redirect_uri: "https://claude.ai/api/mcp/auth_callback".to_string(),
            code_challenge: "challenge".to_string(),
            scope: "mcp".to_string(),
            resource: None,
            expires_at_sec: now_unix_seconds() + ttl,
        }
    }

    #[test]
    fn a_code_can_be_redeemed_once() {
        let mut inner = AuthCodesInner::new();

        inner.add("abc".to_string(), code(AUTH_CODE_TTL_SEC));

        assert!(inner.take("abc").is_some());
        // Replay finds nothing.
        assert!(inner.take("abc").is_none());
    }

    #[test]
    fn an_expired_code_is_not_redeemable() {
        let mut inner = AuthCodesInner::new();

        inner.add("old".to_string(), code(-1));

        assert!(inner.take("old").is_none());
    }

    #[test]
    fn an_unknown_code_is_refused() {
        let mut inner = AuthCodesInner::new();

        assert!(inner.take("never-issued").is_none());
    }

    #[test]
    fn the_store_stays_bounded() {
        let mut inner = AuthCodesInner::new();

        for index in 0..(MAX_CODES + 50) {
            inner.add(format!("code-{}", index), code(AUTH_CODE_TTL_SEC));
        }

        assert!(inner.codes.len() <= MAX_CODES);
    }
}
