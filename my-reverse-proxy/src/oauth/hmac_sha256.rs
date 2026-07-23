use sha2::{Digest, Sha256};

const BLOCK_SIZE: usize = 64;

pub const HMAC_SHA256_LEN: usize = 32;

/// HMAC-SHA256 (RFC 2104) over the crate's existing `sha2` dependency.
///
/// Written out rather than pulled in as a crate: the `hmac` crate is split
/// across two incompatible `digest` generations (0.10 / 0.11) and the version
/// that matches this crate's `sha2` is not the one `*` resolves to, so adding it
/// would pin a second SHA-256 implementation into the build for twenty lines of
/// pad-and-hash. The RFC 4231 vectors below — including the longer-than-block
/// key case, which is the only branch with room for an error — are what make
/// that trade safe.
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; HMAC_SHA256_LEN] {
    let mut block_key = [0u8; BLOCK_SIZE];

    if key.len() > BLOCK_SIZE {
        let hashed = Sha256::digest(key);
        block_key[..HMAC_SHA256_LEN].copy_from_slice(&hashed[..]);
    } else {
        block_key[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = [0x36u8; BLOCK_SIZE];
    let mut outer_pad = [0x5Cu8; BLOCK_SIZE];

    for index in 0..BLOCK_SIZE {
        inner_pad[index] ^= block_key[index];
        outer_pad[index] ^= block_key[index];
    }

    let mut inner = Sha256::new();
    inner.update(&inner_pad[..]);
    inner.update(message);
    let inner = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(&outer_pad[..]);
    outer.update(&inner[..]);

    let mut result = [0u8; HMAC_SHA256_LEN];
    result.copy_from_slice(&outer.finalize()[..]);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_hex(bytes: &[u8]) -> String {
        let mut result = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            result.push_str(&format!("{:02x}", byte));
        }
        result
    }

    /// RFC 4231 test case 1.
    #[test]
    fn rfc_4231_case_1() {
        let key = [0x0bu8; 20];

        assert_eq!(
            as_hex(&hmac_sha256(&key, b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    /// RFC 4231 test case 2 — a key shorter than the block, so the zero padding
    /// is what is under test.
    #[test]
    fn rfc_4231_case_2() {
        assert_eq!(
            as_hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    /// RFC 4231 test case 3 — key and message both exactly at their edge cases.
    #[test]
    fn rfc_4231_case_3() {
        let key = [0xaau8; 20];
        let message = [0xddu8; 50];

        assert_eq!(
            as_hex(&hmac_sha256(&key, &message)),
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"
        );
    }

    /// RFC 4231 test case 6 — a key LONGER than the 64-byte block, which must be
    /// hashed down first. This is the branch a hand-written HMAC gets wrong.
    #[test]
    fn rfc_4231_case_6_key_longer_than_the_block() {
        let key = [0xaau8; 131];

        assert_eq!(
            as_hex(&hmac_sha256(
                &key,
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn a_different_key_gives_a_different_tag() {
        assert_ne!(
            hmac_sha256(b"key-a", b"message"),
            hmac_sha256(b"key-b", b"message")
        );
    }

    #[test]
    fn a_different_message_gives_a_different_tag() {
        assert_ne!(
            hmac_sha256(b"key", b"message-a"),
            hmac_sha256(b"key", b"message-b")
        );
    }
}
