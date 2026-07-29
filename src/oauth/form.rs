use std::collections::HashMap;

/// Parses `application/x-www-form-urlencoded` — the encoding OAuth uses for both
/// the token request body and the authorize query string.
///
/// Parsed here rather than through a framework helper because the token endpoint
/// must accept form-urlencoded bodies specifically (a JSON-only body parser is a
/// documented cause of `415` failures in this flow), and because the same rules
/// then apply to the query string and the consent form without a second code
/// path.
pub fn parse_form_urlencoded(src: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();

    for pair in src.split('&') {
        if pair.is_empty() {
            continue;
        }

        let (name, value) = match pair.split_once('=') {
            Some((name, value)) => (name, value),
            None => (pair, ""),
        };

        let name = form_url_decode(name);

        // First wins: a repeated parameter is a malformed request, and taking
        // the first is the conservative reading.
        result.entry(name).or_insert_with(|| form_url_decode(value));
    }

    result
}

/// Decodes `%XX` escapes, and `+` as a space, which is the form-urlencoded rule.
pub fn form_url_decode(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                match (hex_value(bytes[index + 1]), hex_value(bytes[index + 2])) {
                    (Some(high), Some(low)) => {
                        out.push(high << 4 | low);
                        index += 3;
                    }
                    // Not a valid escape — keep the '%' as written.
                    _ => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            other => {
                out.push(other);
                index += 1;
            }
        }
    }

    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Percent-encodes a value for use in a query string, keeping only the
/// unreserved characters as-is.
pub fn percent_encode(src: &str) -> String {
    let mut out = String::with_capacity(src.len());

    for byte in src.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{:02X}", other)),
        }
    }

    out
}

/// Escapes text for embedding in HTML, so a value echoed back into the consent
/// form can not inject markup.
pub fn html_escape(src: &str) -> String {
    let mut out = String::with_capacity(src.len());

    for character in src.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            other => out.push(other),
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_token_request_body() {
        let parsed = parse_form_urlencoded(
            "grant_type=authorization_code&code=abc123&redirect_uri=https%3A%2F%2Fclaude.ai%2Fapi%2Fmcp%2Fauth_callback&code_verifier=xyz",
        );

        assert_eq!(parsed.get("grant_type").unwrap(), "authorization_code");
        assert_eq!(parsed.get("code").unwrap(), "abc123");
        assert_eq!(
            parsed.get("redirect_uri").unwrap(),
            "https://claude.ai/api/mcp/auth_callback"
        );
        assert_eq!(parsed.get("code_verifier").unwrap(), "xyz");
    }

    #[test]
    fn decodes_plus_as_space_and_keeps_empty_values() {
        let parsed = parse_form_urlencoded("scope=mcp+offline_access&state=&flag");

        assert_eq!(parsed.get("scope").unwrap(), "mcp offline_access");
        assert_eq!(parsed.get("state").unwrap(), "");
        assert_eq!(parsed.get("flag").unwrap(), "");
    }

    #[test]
    fn survives_malformed_escapes() {
        let parsed = parse_form_urlencoded("a=100%&b=%zz&c=%");

        assert_eq!(parsed.get("a").unwrap(), "100%");
        assert_eq!(parsed.get("b").unwrap(), "%zz");
        assert_eq!(parsed.get("c").unwrap(), "%");
    }

    #[test]
    fn an_empty_body_parses_to_nothing() {
        assert!(parse_form_urlencoded("").is_empty());
    }

    #[test]
    fn percent_encoding_round_trips() {
        let original = "https://claude.ai/api/mcp/auth_callback?a=b c&d=e";

        let encoded = percent_encode(original);

        assert!(!encoded.contains('/'));
        assert!(!encoded.contains(' '));
        assert_eq!(
            parse_form_urlencoded(&format!("x={}", encoded))
                .get("x")
                .unwrap(),
            original
        );
    }

    #[test]
    fn html_is_escaped() {
        assert_eq!(
            html_escape("<script>\"x\"&'y'</script>"),
            "&lt;script&gt;&quot;x&quot;&amp;&#x27;y&#x27;&lt;/script&gt;"
        );
    }
}
