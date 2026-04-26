use bytes::Bytes;
use http_body_util::Full;

/// Detects HTTP/1.1 WebSocket upgrade signature in the incoming request:
/// `Connection: Upgrade` + `Upgrade: websocket` (RFC 6455).
pub fn is_h1_websocket_upgrade(req: &hyper::Request<Full<Bytes>>) -> bool {
    let headers = req.headers();

    let connection_has_upgrade = headers
        .get_all(hyper::header::CONNECTION)
        .iter()
        .any(|v| {
            v.as_bytes()
                .split(|&b| b == b',')
                .any(|tok| tok.trim_ascii().eq_ignore_ascii_case(b"upgrade"))
        });

    if !connection_has_upgrade {
        return false;
    }

    headers
        .get_all(hyper::header::UPGRADE)
        .iter()
        .any(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket"))
}
