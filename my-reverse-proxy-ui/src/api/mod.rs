mod configuration;
pub use configuration::*;
mod logs;
pub use logs::*;
mod ssl_certificates;
pub use ssl_certificates::*;

/// reqwest's wasm backend rejects relative paths ("builder error" from `Url::parse`), so every
/// request is anchored against the page origin. The SPA is always served by the same http server
/// as the admin API, so the origin is the right base.
fn build_url(path_and_query: &str) -> Result<String, String> {
    let origin = web_sys::window()
        .ok_or_else(|| "no window in current context".to_string())?
        .location()
        .origin()
        .map_err(|e| format!("could not read window.location.origin: {e:?}"))?;
    Ok(format!("{origin}{path_and_query}"))
}
