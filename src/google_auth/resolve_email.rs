use my_settings_reader::flurl::FlUrl;
use serde::*;

use crate::{http_proxy_pass::HostPort, settings::GoogleAuthSettings};

pub async fn resolve_email<THostPort: HostPort + Send + Sync + 'static>(
    req: &THostPort,
    code: &str,
    settings: &GoogleAuthSettings,
) -> String {
    let response = FlUrl::new("https://oauth2.googleapis.com/token")
        .with_header("ContentType", "application/json")
        .post_json(GetData {
            code: code.to_string(),
            client_id: settings.client_id.to_string(),
            client_secret: settings.client_secret.clone(),
            redirect_uri: format!("https://{}", super::generate_redirect_url(req)),
            grant_type: "authorization_code".to_string(),
        })
        .await
        .unwrap()
        .receive_body()
        .await
        .unwrap();

    let token = serde_json::from_slice::<OAuthResponse>(response.as_slice()).unwrap();

    let resp = FlUrl::new("https://www.googleapis.com/oauth2/v1/userinfo")
        .with_header("Authorization", format!("Bearer {}", token.access_token))
        .get()
        .await
        .unwrap()
        .body_as_str()
        .await
        .unwrap()
        .to_string();

    let user_info = serde_json::from_str::<GoogleUserInfo>(resp.as_str()).unwrap();

    user_info.email
}

#[derive(Serialize, Deserialize)]
pub struct GetData {
    pub code: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub grant_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct OAuthResponse {
    pub access_token: String,
}

#[derive(Serialize, Deserialize)]
pub struct GoogleUserInfo {
    pub email: String,
}
