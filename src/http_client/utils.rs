use hyper::Uri;

pub fn get_host_port(uri: &Uri) -> String {
    let host = uri.host().unwrap().to_owned();
    let port = uri.port_u16().unwrap_or(443);
    format!("{}:{}", host, port)
}

pub fn is_https(uri: &Uri) -> bool {
    match uri.scheme_str() {
        Some("https") => true,
        _ => false,
    }
}
