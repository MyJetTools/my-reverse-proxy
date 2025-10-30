use crate::types::*;

use super::*;

pub async fn handle_google_auth(
    req: &impl HttpRequestReader,
    g_auth_settings: &str,
    debug: bool,
) -> Result<GoogleAuthOkResult, GoogleAuthError> {
    let google_auth_credentials = crate::app::APP_CTX
        .current_configuration
        .get(|config| config.google_auth_credentials.get(g_auth_settings))
        .await;

    if google_auth_credentials.is_none() {
        panic!("Google Auth Credentials not found");
    }

    let google_auth_credentials = google_auth_credentials.unwrap();

    let path = req.get_path();

    if path.eq_ignore_ascii_case(LOGOUT_PATH) {
        return Err(GoogleAuthError::ShowLogoutPage);
    }

    if path.eq_ignore_ascii_case(AUTHORIZED_PATH) {
        if let Some(token) = req.get_authorization_token() {
            if let Some(email) = crate::google_auth::token::resolve(token) {
                if !google_auth_credentials.domain_is_allowed(&email) {
                    return Err(GoogleAuthError::EmailDomainIsNotAuthorized);
                }

                return Err(GoogleAuthError::ShowUserAuthenticatedPage(email));
            }
        }

        let code = req.get_query_string_param("code").unwrap();

        let email = match crate::google_auth::resolve_email(
            req,
            code.as_str(),
            &google_auth_credentials,
            debug,
        )
        .await
        {
            Ok(email) => email,
            Err(err) => {
                return Err(GoogleAuthError::ShowError(err));
            }
        };

        if !google_auth_credentials.domain_is_allowed(&email) {
            return Err(GoogleAuthError::ShowEmailDomainIsNotAuthorizedPage);
        }

        return Ok(GoogleAuthOkResult::SetToken(email));
    }

    if let Some(token) = req.get_authorization_token() {
        if let Some(email) = crate::google_auth::token::resolve(token) {
            if !google_auth_credentials.domain_is_allowed(&email) {
                return Err(GoogleAuthError::EmailDomainIsNotAuthorized);
            }
            return Ok(GoogleAuthOkResult::Passed(email));
        }
    }

    Ok(GoogleAuthOkResult::ShowLoginPage(google_auth_credentials))
}
