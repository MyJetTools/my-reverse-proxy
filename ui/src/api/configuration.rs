use crate::models::CurrentConfigurationModel;

const CURRENT_CONFIG_PATH: &str = "/api/configuration/Current";

pub async fn get_current_configuration() -> Result<CurrentConfigurationModel, String> {
    let url = super::build_url(CURRENT_CONFIG_PATH)?;

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
