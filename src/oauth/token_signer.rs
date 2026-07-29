use serde::{Deserialize, Serialize};

use super::{
    base64_url_decode, base64_url_encode, constant_time_eq, hmac_sha256, now_unix_seconds,
};

const VERSION: &str = "v1";

pub const DEFAULT_ACCESS_TOKEN_TTL_SEC: i64 = 60 * 60;
pub const DEFAULT_REFRESH_TOKEN_TTL_SEC: i64 = 30 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Access,
    Refresh,
}

impl TokenKind {
    fn as_str(&self) -> &'static str {
        match self {
            TokenKind::Access => "a",
            TokenKind::Refresh => "r",
        }
    }

    fn parse(src: &str) -> Option<Self> {
        match src {
            "a" => Some(TokenKind::Access),
            "r" => Some(TokenKind::Refresh),
            _ => None,
        }
    }
}

/// What a token grants, recovered from its own claims once the signature checks
/// out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedToken {
    pub scope: String,
    /// The RFC 8707 resource the token was issued for, when the client asked for
    /// one. `None` means the token is good for every protected path on the
    /// endpoint.
    pub audience: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct TokenPayload {
    /// Kind — an access token must never be accepted where a refresh token is
    /// expected, or the other way round.
    k: String,
    /// Expiry, unix seconds.
    exp: i64,
    /// Granted scope.
    sc: String,
    /// Audience (RFC 8707 `resource`), when the client requested one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aud: Option<String>,
}

/// Mints and verifies the tokens handed to the connector.
///
/// The tokens carry their own claims and a signature, so nothing has to be
/// stored server-side to validate one. That is what lets the connector keep
/// working across a restart: with the signing key persisted, a token minted
/// before the restart still verifies afterwards, and there is no token table to
/// reload or to grow without bound.
pub struct TokenSigner {
    key: Vec<u8>,
}

impl TokenSigner {
    pub fn new(key: Vec<u8>) -> Self {
        Self { key }
    }

    pub fn mint(
        &self,
        kind: TokenKind,
        scope: &str,
        audience: Option<&str>,
        ttl_sec: i64,
    ) -> Result<String, String> {
        let payload = TokenPayload {
            k: kind.as_str().to_string(),
            exp: now_unix_seconds() + ttl_sec,
            sc: scope.to_string(),
            aud: audience.map(|itm| itm.to_string()),
        };

        let payload = serde_json::to_vec(&payload)
            .map_err(|err| format!("Can not serialize the token. Err: {}", err))?;

        let body = format!("{}.{}", VERSION, base64_url_encode(&payload));
        let signature = hmac_sha256(&self.key, body.as_bytes());

        Ok(format!("{}.{}", body, base64_url_encode(&signature)))
    }

    /// Returns what was granted when the token is genuine, of the expected kind,
    /// and unexpired.
    pub fn verify(&self, token: &str, expected: TokenKind) -> Option<VerifiedToken> {
        let (body, signature) = token.rsplit_once('.')?;

        if !body.starts_with(VERSION) {
            return None;
        }

        let presented = base64_url_decode(signature).ok()?;

        // Checked before the payload is even parsed, so nothing unsigned is ever
        // acted upon.
        if !constant_time_eq(&hmac_sha256(&self.key, body.as_bytes()), &presented) {
            return None;
        }

        let payload = body.split_once('.')?.1;
        let payload: TokenPayload =
            serde_json::from_slice(&base64_url_decode(payload).ok()?).ok()?;

        if TokenKind::parse(&payload.k)? != expected {
            return None;
        }

        if payload.exp <= now_unix_seconds() {
            return None;
        }

        Some(VerifiedToken {
            scope: payload.sc,
            audience: payload.aud,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signer() -> TokenSigner {
        TokenSigner::new(b"a-test-signing-key".to_vec())
    }

    #[test]
    fn a_minted_token_verifies_and_carries_its_scope() {
        let signer = signer();

        let token = signer
            .mint(TokenKind::Access, "mcp", None, DEFAULT_ACCESS_TOKEN_TTL_SEC)
            .unwrap();

        assert_eq!(
            signer.verify(&token, TokenKind::Access),
            Some(VerifiedToken {
                scope: "mcp".to_string(),
                audience: None,
            })
        );
    }

    #[test]
    fn the_requested_resource_is_carried_as_the_audience() {
        let signer = signer();

        let token = signer
            .mint(
                TokenKind::Access,
                "mcp",
                Some("https://mcp-home.jetdev.eu/mt-risks"),
                DEFAULT_ACCESS_TOKEN_TTL_SEC,
            )
            .unwrap();

        assert_eq!(
            signer.verify(&token, TokenKind::Access).unwrap().audience,
            Some("https://mcp-home.jetdev.eu/mt-risks".to_string())
        );
    }

    #[test]
    fn an_access_token_is_not_accepted_as_a_refresh_token() {
        let signer = signer();

        let access = signer
            .mint(TokenKind::Access, "mcp", None, DEFAULT_ACCESS_TOKEN_TTL_SEC)
            .unwrap();
        let refresh = signer
            .mint(
                TokenKind::Refresh,
                "mcp",
                None,
                DEFAULT_REFRESH_TOKEN_TTL_SEC,
            )
            .unwrap();

        assert!(signer.verify(&access, TokenKind::Refresh).is_none());
        assert!(signer.verify(&refresh, TokenKind::Access).is_none());
    }

    #[test]
    fn a_token_signed_with_another_key_is_refused() {
        let token = signer()
            .mint(TokenKind::Access, "mcp", None, DEFAULT_ACCESS_TOKEN_TTL_SEC)
            .unwrap();

        let other = TokenSigner::new(b"a-different-key".to_vec());

        assert!(other.verify(&token, TokenKind::Access).is_none());
    }

    #[test]
    fn a_tampered_payload_is_refused() {
        let signer = signer();
        let token = signer
            .mint(TokenKind::Access, "mcp", None, DEFAULT_ACCESS_TOKEN_TTL_SEC)
            .unwrap();

        // Re-encode the payload with a far-future expiry, keeping the original
        // signature — the classic forgery attempt.
        let forged_payload = base64_url_encode(
            &serde_json::to_vec(&TokenPayload {
                k: "a".to_string(),
                exp: 99_999_999_999,
                sc: "mcp".to_string(),
                aud: None,
            })
            .unwrap(),
        );

        let signature = token.rsplit_once('.').unwrap().1;
        let forged = format!("{}.{}.{}", VERSION, forged_payload, signature);

        assert!(signer.verify(&forged, TokenKind::Access).is_none());
    }

    #[test]
    fn an_expired_token_is_refused() {
        let signer = signer();

        let token = signer.mint(TokenKind::Access, "mcp", None, -1).unwrap();

        assert!(signer.verify(&token, TokenKind::Access).is_none());
    }

    #[test]
    fn nonsense_is_refused_without_panicking() {
        let signer = signer();

        for token in ["", ".", "v1", "v1.x", "not-a-token", "v1.!!!.!!!"] {
            assert!(
                signer.verify(token, TokenKind::Access).is_none(),
                "{}",
                token
            );
        }
    }
}
