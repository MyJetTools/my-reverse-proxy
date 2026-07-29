/// Where the hosted Claude surfaces send the user back after consent.
pub const CLAUDE_REDIRECT_URI: &str = "https://claude.ai/api/mcp/auth_callback";

/// True when `redirect_uri` is one this server will send a code to.
///
/// The hosted Claude surfaces use one fixed callback. Claude Code is a native
/// client on an RFC 8252 loopback redirect whose port changes every session, so
/// loopback is matched with the port ignored — which that RFC requires for the
/// IP-literal form and which Claude Code needs for `localhost` too.
pub fn is_allowed_redirect_uri(redirect_uri: &str) -> bool {
    if redirect_uri == CLAUDE_REDIRECT_URI {
        return true;
    }

    is_loopback_redirect(redirect_uri)
}

fn is_loopback_redirect(redirect_uri: &str) -> bool {
    let rest = match redirect_uri.strip_prefix("http://") {
        Some(rest) => rest,
        // Loopback is the only case where plain http is acceptable.
        None => return false,
    };

    // Never follow a userinfo or an embedded credential shape.
    if rest.contains('@') {
        return false;
    }

    let host_and_port = match rest.split_once('/') {
        Some((host_and_port, _path)) => host_and_port,
        None => rest,
    };

    let host = match host_and_port.rsplit_once(':') {
        Some((host, port)) => {
            if !port.is_empty() && port.parse::<u16>().is_err() {
                return false;
            }
            host
        }
        None => host_and_port,
    };

    host == "127.0.0.1" || host == "localhost" || host == "[::1]"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_hosted_claude_callback() {
        assert!(is_allowed_redirect_uri(
            "https://claude.ai/api/mcp/auth_callback"
        ));
    }

    #[test]
    fn accepts_loopback_on_any_port() {
        assert!(is_allowed_redirect_uri("http://localhost:3118/callback"));
        assert!(is_allowed_redirect_uri("http://127.0.0.1:51234/callback"));
        assert!(is_allowed_redirect_uri("http://127.0.0.1/callback"));
        assert!(is_allowed_redirect_uri("http://[::1]:8080/callback"));
    }

    #[test]
    fn refuses_anywhere_else() {
        assert!(!is_allowed_redirect_uri("https://evil.example/callback"));
        assert!(!is_allowed_redirect_uri("http://evil.example/callback"));
        // A lookalike host that merely contains the allowed one.
        assert!(!is_allowed_redirect_uri("http://localhost.evil.example/x"));
        assert!(!is_allowed_redirect_uri(
            "https://claude.ai/api/mcp/auth_callback/../elsewhere"
        ));
        // Credentials smuggled into the authority.
        assert!(!is_allowed_redirect_uri("http://localhost@evil.example/x"));
        assert!(!is_allowed_redirect_uri("http://127.0.0.1:notaport/x"));
        assert!(!is_allowed_redirect_uri(""));
    }
}
