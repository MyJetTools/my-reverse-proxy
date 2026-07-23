/// This endpoint's own origin, built from the request's scheme and `Host`.
///
/// The default port is dropped on purpose: a client sending `Host:
/// example.com:443` still typed `https://example.com` into the connector
/// dialog, and RFC 9728 requires the `resource` in the metadata to be that
/// string exactly — Claude compares them and gives up on a mismatch.
pub fn build_base_url(is_https: bool, host: &str) -> String {
    let scheme = if is_https { "https" } else { "http" };
    let default_port = if is_https { "443" } else { "80" };

    let host = host.trim().trim_end_matches('/');

    let host = match host.rsplit_once(':') {
        // `[::1]:443` splits on the last colon; a bare `::1` must not, so a
        // name that still holds a colon only counts when it is bracketed.
        Some((name, port)) if !name.is_empty() && (!name.contains(':') || name.ends_with(']')) => {
            if port == default_port {
                name
            } else {
                host
            }
        }
        _ => host,
    };

    format!("{}://{}", scheme, host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_host_keeps_its_scheme() {
        assert_eq!(
            build_base_url(true, "mcp-home.jetdev.eu"),
            "https://mcp-home.jetdev.eu"
        );
        assert_eq!(build_base_url(false, "localhost"), "http://localhost");
    }

    #[test]
    fn the_default_port_is_dropped() {
        assert_eq!(
            build_base_url(true, "mcp-home.jetdev.eu:443"),
            "https://mcp-home.jetdev.eu"
        );
        assert_eq!(build_base_url(false, "localhost:80"), "http://localhost");
    }

    #[test]
    fn a_non_default_port_is_kept() {
        assert_eq!(
            build_base_url(true, "mcp-home.jetdev.eu:8443"),
            "https://mcp-home.jetdev.eu:8443"
        );
        assert_eq!(
            build_base_url(false, "localhost:8000"),
            "http://localhost:8000"
        );
    }

    #[test]
    fn ipv6_hosts_survive_intact() {
        assert_eq!(build_base_url(true, "[::1]:443"), "https://[::1]");
        assert_eq!(build_base_url(true, "[::1]:8443"), "https://[::1]:8443");
        assert_eq!(build_base_url(true, "[::1]"), "https://[::1]");
    }
}
