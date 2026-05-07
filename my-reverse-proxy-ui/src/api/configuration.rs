use crate::models::CurrentConfigurationModel;

const CURRENT_CONFIG_PATH: &str = "/api/configuration/Current";

pub async fn get_current_configuration() -> Result<CurrentConfigurationModel, String> {
    // reqwest's wasm backend rejects relative paths ("builder error" from
    // Url::parse). The SPA is always served from the same origin as the
    // admin API, so anchor against the page's origin.
    let origin = web_sys::window()
        .ok_or_else(|| "no window in current context".to_string())?
        .location()
        .origin()
        .map_err(|e| format!("could not read window.location.origin: {e:?}"))?;
    let url = format!("{origin}{CURRENT_CONFIG_PATH}");

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("GET {url} failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("GET {url} returned {}", resp.status()));
    }

    resp.json::<CurrentConfigurationModel>()
        .await
        .map_err(|e| format!("decoding {url} response failed: {e}"))
}
