use base64::Engine;
use rand_core::{OsRng, RngCore};

/// Random bytes from the OS CSPRNG — the same source the gateway handshake and
/// the key generator already use, so there is one randomness story in the crate.
pub fn random_bytes(amount: usize) -> Vec<u8> {
    let mut buffer = vec![0u8; amount];
    OsRng.fill_bytes(&mut buffer);
    buffer
}

/// A URL-safe random secret — used for the signing key, authorization codes and
/// anything else that must be unguessable.
pub fn random_secret(bytes: usize) -> String {
    base64_url_encode(&random_bytes(bytes))
}

/// base64url without padding — what OAuth and PKCE use throughout.
pub fn base64_url_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn base64_url_decode(text: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(text.as_bytes())
        .map_err(|err| format!("Not a valid base64url value. Err: {}", err))
}

/// Standard base64 with padding — what HTTP Basic authentication uses.
pub fn base64_standard_decode(text: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(text.as_bytes())
        .map_err(|err| format!("Not a valid base64 value. Err: {}", err))
}

/// Compares without an early exit, so timing does not reveal how much of a
/// secret was guessed correctly.
pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut difference = 0u8;

    for (left_byte, right_byte) in left.iter().zip(right.iter()) {
        difference |= left_byte ^ right_byte;
    }

    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_bytes_are_the_requested_length_and_differ() {
        let first = random_bytes(32);
        let second = random_bytes(32);

        assert_eq!(first.len(), 32);
        assert_eq!(second.len(), 32);
        assert_ne!(first, second);
    }

    #[test]
    fn base64_url_round_trips() {
        // Bytes that encode differently in the URL-safe alphabet than in the
        // standard one, so a mixed-up engine would show up here.
        let bytes = vec![0xFB, 0xFF, 0xFE, 0x00, 0x01];

        let encoded = base64_url_encode(&bytes);

        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));
        assert_eq!(base64_url_decode(&encoded).unwrap(), bytes);
    }

    #[test]
    fn basic_credentials_decode_as_standard_base64() {
        // "claude:secret" as an HTTP Basic payload — padded, standard alphabet.
        assert_eq!(
            base64_standard_decode("Y2xhdWRlOnNlY3JldA==").unwrap(),
            b"claude:secret"
        );
    }

    #[test]
    fn compares_correctly() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secrft"));
        assert!(!constant_time_eq(b"secret", b"secret-longer"));
        assert!(constant_time_eq(b"", b""));
    }
}
