use sha2::{Digest, Sha256};

use super::{base64_url_encode, constant_time_eq};

/// Verifies a PKCE `code_verifier` against the `code_challenge` the client sent
/// when it started the flow.
///
/// Only S256 is supported, and that is deliberate: `plain` offers no protection
/// at all, OAuth 2.1 removed it, and Claude always sends S256 anyway.
pub fn verify_s256(code_verifier: &str, code_challenge: &str) -> bool {
    if code_verifier.is_empty() || code_challenge.is_empty() {
        return false;
    }

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());

    let expected = base64_url_encode(&hasher.finalize()[..]);

    constant_time_eq(expected.as_bytes(), code_challenge.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The worked example from RFC 7636 appendix B — if the encoding or the
    /// hashing is off by anything, this fails.
    #[test]
    fn matches_the_rfc_7636_example() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

        assert!(verify_s256(verifier, challenge));
    }

    #[test]
    fn rejects_the_wrong_verifier() {
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

        assert!(!verify_s256("some-other-verifier", challenge));
    }

    #[test]
    fn rejects_empty_input() {
        assert!(!verify_s256(
            "",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        ));
        assert!(!verify_s256(
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
            ""
        ));
    }

    #[test]
    fn a_plain_challenge_does_not_pass_as_s256() {
        // Sending the verifier itself as the challenge is the `plain` method,
        // which must never be accepted here.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";

        assert!(!verify_s256(verifier, verifier));
    }
}
