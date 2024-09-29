use my_settings_reader::flurl::FlUrl;
use serde::*;

use crate::{http_proxy_pass::HostPort, settings::GoogleAuthSettings, types::Email};

pub async fn resolve_email<THostPort: HostPort + Send + Sync + 'static>(
    req: &THostPort,
    code: &str,
    settings: &GoogleAuthSettings,
    debug: bool,
) -> Result<Email, String> {
    let response = FlUrl::new("https://oauth2.googleapis.com/token")
        .do_not_reuse_connection()
        .with_header("ContentType", "application/json")
        .post_json(&GetData {
            code: code.to_string(),
            client_id: settings.client_id.to_string(),
            client_secret: settings.client_secret.clone(),
            redirect_uri: format!("https://{}", super::generate_redirect_url(req)),
            grant_type: "authorization_code".to_string(),
        })
        .await
        .unwrap();

    if debug {
        println!("status_code: {:?}", response.get_status_code());
    }

    let response = response.receive_body().await.unwrap();

    if debug {
        println!(
            "response: {}",
            std::str::from_utf8(response.as_slice()).unwrap()
        );
    }

    let o_auth_response = serde_json::from_slice::<OAuthResponse>(response.as_slice()).unwrap();

    if o_auth_response.access_token.is_none() {
        return Err(String::from_utf8(response).unwrap());
    }

    let resp = FlUrl::new("https://www.googleapis.com/oauth2/v1/userinfo")
        .do_not_reuse_connection()
        .with_header(
            "Authorization",
            format!("Bearer {}", o_auth_response.access_token.unwrap()),
        )
        .get()
        .await
        .unwrap()
        .body_as_str()
        .await
        .unwrap()
        .to_string();

    if debug {
        println!("response_with_email: {}", resp);
    }

    let user_info = serde_json::from_str::<GoogleUserInfo>(resp.as_str()).unwrap();

    match user_info.email {
        Some(email) => Ok(Email::new(email)),
        None => Err(resp),
    }
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
    pub access_token: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct GoogleUserInfo {
    pub email: Option<String>,
}
