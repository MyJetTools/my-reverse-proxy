use crate::models::ProxyLogsModel;

/// reqwest's wasm backend rejects relative paths, so anchor every request
/// against the page origin (the SPA is served from the same origin as the API).
fn build_url(path_and_query: &str) -> Result<String, String> {
    let origin = web_sys::window()
        .ok_or_else(|| "no window in current context".to_string())?
        .location()
        .origin()
        .map_err(|e| format!("could not read window.location.origin: {e:?}"))?;
    Ok(format!("{origin}{path_and_query}"))
}

async fn get_logs(path_and_query: String) -> Result<ProxyLogsModel, String> {
    let url = build_url(&path_and_query)?;

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("GET {url} failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GET {url} returned {}", resp.status()));
    }

    resp.json::<ProxyLogsModel>()
        .await
        .map_err(|e| format!("decoding {url} response failed: {e}"))
}

pub async fn get_port_logs(id: &str) -> Result<ProxyLogsModel, String> {
    get_logs(format!("/api/logs/port?id={}", urlencode(id))).await
}

pub async fn get_endpoint_logs(id: &str) -> Result<ProxyLogsModel, String> {
    get_logs(format!("/api/logs/endpoint?id={}", urlencode(id))).await
}

pub async fn get_location_logs(id: i64) -> Result<ProxyLogsModel, String> {
    get_logs(format!("/api/logs/location?id={id}")).await
}

async fn post(path_and_query: String) -> Result<(), String> {
    let url = build_url(&path_and_query)?;
    let resp = reqwest::Client::new()
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("POST {url} failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("POST {url} returned {}", resp.status()));
    }
    Ok(())
}

pub async fn set_endpoint_debug(id: &str, enabled: bool) -> Result<(), String> {
    post(format!(
        "/api/logs/endpoint/debug?id={}&enabled={}",
        urlencode(id),
        enabled
    ))
    .await
}

pub async fn set_location_debug(id: i64, enabled: bool) -> Result<(), String> {
    post(format!(
        "/api/logs/location/debug?id={id}&enabled={enabled}"
    ))
    .await
}

/// Minimal percent-encoding for query values (host strings, unix paths).
fn urlencode(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char)
            }
            _ => result.push_str(&format!("%{:02X}", byte)),
        }
    }
    result
}
