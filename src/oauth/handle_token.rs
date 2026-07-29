use std::collections::HashMap;

use serde::Serialize;

use super::{
    base64_standard_decode, form_url_decode, method_not_allowed, oauth_error_response, verify_s256,
    MintedTokens, OAuthContext, OAuthHttpResponse, OAuthMethod, OAuthRequest, TokenKind,
    ERROR_INVALID_CLIENT, ERROR_INVALID_GRANT, ERROR_INVALID_REQUEST, ERROR_UNSUPPORTED_GRANT_TYPE,
};

const FORM_CONTENT_TYPE: &str = "application/x-www-form-urlencoded";

#[derive(Serialize)]
struct TokenResponseBody {
    access_token: String,
    token_type: &'static str,
    expires_in: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    scope: String,
}

/// The token endpoint: `authorization_code` for the first exchange and
/// `refresh_token` for every renewal afterwards.
pub fn handle_token(context: &OAuthContext, request: &OAuthRequest) -> OAuthHttpResponse {
    if request.method != OAuthMethod::Post {
        return method_not_allowed("POST");
    }

    let parameters = request.parameters();

    if parameters.is_empty() {
        // Sending JSON here is the classic cause of a connector that never gets
        // past the first token exchange, so the error says what was received.
        let description = match request.content_type {
            Some(content_type) => format!(
                "The request body must be {}, and this one was sent as '{}'",
                FORM_CONTENT_TYPE, content_type
            ),
            None => format!("The request body must be {}", FORM_CONTENT_TYPE),
        };

        return oauth_error_response(400, ERROR_INVALID_REQUEST, &description);
    }

    let Some(presented) = presented_client(request.authorization, &parameters) else {
        return invalid_client(false);
    };

    if !matches_client_id(context, &presented.client_id)
        || !matches_client_secret(context, &presented.client_secret)
    {
        return invalid_client(presented.from_basic).into_credential_failure();
    }

    match parameters.get("grant_type").map(String::as_str) {
        Some("authorization_code") => exchange_authorization_code(context, &parameters),
        Some("refresh_token") => refresh(context, &parameters),
        Some(other) => oauth_error_response(
            400,
            ERROR_UNSUPPORTED_GRANT_TYPE,
            &format!("The grant type '{}' is not supported", other),
        ),
        None => oauth_error_response(400, ERROR_INVALID_REQUEST, "grant_type is required"),
    }
}

fn exchange_authorization_code(
    context: &OAuthContext,
    parameters: &HashMap<String, String>,
) -> OAuthHttpResponse {
    let code = get(parameters, "code");
    if code.is_empty() {
        return oauth_error_response(400, ERROR_INVALID_REQUEST, "code is required");
    }

    // Taken out of the store, so a replay of the same code finds nothing even if
    // everything else about the request is right.
    let Some(issued) = context.codes.redeem(code) else {
        return oauth_error_response(
            400,
            ERROR_INVALID_GRANT,
            "The authorization code is unknown, already used, or expired",
        );
    };

    let redirect_uri = get(parameters, "redirect_uri");
    if redirect_uri != issued.redirect_uri {
        return oauth_error_response(
            400,
            ERROR_INVALID_GRANT,
            "redirect_uri does not match the one the code was issued for",
        );
    }

    let code_verifier = get(parameters, "code_verifier");
    if code_verifier.is_empty() {
        return oauth_error_response(400, ERROR_INVALID_REQUEST, "code_verifier is required");
    }

    if !verify_s256(code_verifier, &issued.code_challenge) {
        return oauth_error_response(
            400,
            ERROR_INVALID_GRANT,
            "The code_verifier does not match the code_challenge",
        );
    }

    mint(context, &issued.scope, issued.resource.as_deref())
}

fn refresh(context: &OAuthContext, parameters: &HashMap<String, String>) -> OAuthHttpResponse {
    let refresh_token = get(parameters, "refresh_token");
    if refresh_token.is_empty() {
        return oauth_error_response(400, ERROR_INVALID_REQUEST, "refresh_token is required");
    }

    let Some(verified) = context.signer.verify(refresh_token, TokenKind::Refresh) else {
        return oauth_error_response(
            400,
            ERROR_INVALID_GRANT,
            "The refresh token is invalid or expired",
        );
    };

    // The client is confidential — it proved itself with its secret above — so
    // the token is not rotated. The scope is carried over rather than re-read
    // from the request: a refresh may never widen what was granted.
    mint(context, &verified.scope, verified.audience.as_deref())
}

fn mint(context: &OAuthContext, scope: &str, audience: Option<&str>) -> OAuthHttpResponse {
    let minted = match context.mint_tokens(scope, audience) {
        Ok(minted) => minted,
        Err(err) => {
            return oauth_error_response(500, super::ERROR_SERVER_ERROR, &err);
        }
    };

    OAuthHttpResponse::json(200, token_response_body(minted))
}

fn token_response_body(minted: MintedTokens) -> Vec<u8> {
    let body = TokenResponseBody {
        access_token: minted.access_token,
        token_type: "Bearer",
        expires_in: minted.expires_in,
        refresh_token: minted.refresh_token,
        scope: minted.scope,
    };

    serde_json::to_vec(&body).unwrap_or_else(|_| {
        super::error_body(
            super::ERROR_SERVER_ERROR,
            "Can not build the token response",
        )
    })
}

/// How the client identified itself on this request.
struct PresentedClient {
    client_id: String,
    client_secret: String,
    /// Whether the credentials came from an `Authorization: Basic` header, which
    /// decides whether the 401 carries a `WWW-Authenticate` challenge.
    from_basic: bool,
}

/// Both authentication methods the metadata advertises: HTTP Basic, and the
/// credentials in the form body.
fn presented_client(
    authorization: Option<&str>,
    parameters: &HashMap<String, String>,
) -> Option<PresentedClient> {
    if let Some(authorization) = authorization {
        let trimmed = authorization.trim();

        if trimmed.len() > 6 && trimmed[..6].eq_ignore_ascii_case("Basic ") {
            let decoded = base64_standard_decode(trimmed[6..].trim()).ok()?;
            let decoded = String::from_utf8(decoded).ok()?;
            let (client_id, client_secret) = decoded.split_once(':')?;

            return Some(PresentedClient {
                client_id: client_id.to_string(),
                client_secret: client_secret.to_string(),
                from_basic: true,
            });
        }
    }

    let client_id = get(parameters, "client_id");

    if client_id.is_empty() {
        return None;
    }

    Some(PresentedClient {
        client_id: client_id.to_string(),
        client_secret: get(parameters, "client_secret").to_string(),
        from_basic: false,
    })
}

/// RFC 6749 §2.3.1 says Basic credentials are form-urlencoded before they are
/// base64'd, but plenty of clients base64 them raw. Both readings are tried, so
/// a secret containing `+` or `%` works either way.
fn matches_client_id(context: &OAuthContext, presented: &str) -> bool {
    context.check_client_id(presented) || context.check_client_id(&form_url_decode(presented))
}

fn matches_client_secret(context: &OAuthContext, presented: &str) -> bool {
    context.check_client_secret(presented)
        || context.check_client_secret(&form_url_decode(presented))
}

fn invalid_client(from_basic: bool) -> OAuthHttpResponse {
    let response = oauth_error_response(
        401,
        ERROR_INVALID_CLIENT,
        "The client_id or client_secret is wrong",
    );

    if from_basic {
        return response.add_header("WWW-Authenticate", "Basic realm=\"oauth\"".to_string());
    }

    response
}

fn get<'s>(parameters: &'s HashMap<String, String>, name: &str) -> &'s str {
    parameters.get(name).map(String::as_str).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::*;
    use crate::settings::OAuthSettings;

    const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    const CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
    const REDIRECT_URI: &str = "https://claude.ai/api/mcp/auth_callback";

    fn context() -> OAuthContext {
        OAuthContext::new(
            &OAuthSettings {
                client_id: "claude".to_string(),
                client_secret: "the-secret".to_string(),
                consent_password: "the-password".to_string(),
                public_url: None,
                signing_key: None,
                signing_key_file: None,
                access_token_ttl_sec: None,
                refresh_token_ttl_sec: None,
            },
            b"a-test-signing-key".to_vec(),
        )
    }

    fn post<'s>(body: &'s str, authorization: Option<&'s str>) -> OAuthRequest<'s> {
        OAuthRequest {
            method: OAuthMethod::Post,
            route: OAuthRoute::Token,
            base_url: "https://mcp-home.jetdev.eu",
            query: None,
            body: body.as_bytes(),
            content_type: Some(FORM_CONTENT_TYPE),
            authorization,
            known_resource_paths: &[],
        }
    }

    fn issue_code(context: &OAuthContext, resource: Option<&str>) -> String {
        let code = "the-code".to_string();

        context.codes.issue(
            code.clone(),
            IssuedAuthCode {
                redirect_uri: REDIRECT_URI.to_string(),
                code_challenge: CHALLENGE.to_string(),
                scope: "mcp offline_access".to_string(),
                resource: resource.map(|itm| itm.to_string()),
            },
        );

        code
    }

    fn body_of(response: &OAuthHttpResponse) -> serde_json::Value {
        serde_json::from_slice(&response.body).unwrap()
    }

    fn exchange_body(code: &str) -> String {
        format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fclaude.ai%2Fapi%2Fmcp%2Fauth_callback&code_verifier={}&client_id=claude&client_secret=the-secret",
            code, VERIFIER
        )
    }

    #[test]
    fn a_valid_exchange_returns_an_access_and_a_refresh_token() {
        let context = context();
        let code = issue_code(&context, None);

        let response = handle_token(&context, &post(&exchange_body(&code), None));

        assert_eq!(response.status_code, 200);

        let body = body_of(&response);
        assert_eq!(body["token_type"], "Bearer");
        assert_eq!(body["scope"], "mcp offline_access");
        assert_eq!(body["expires_in"], 3600);
        assert!(body["refresh_token"].is_string());

        let access_token = body["access_token"].as_str().unwrap();
        assert!(context
            .signer
            .verify(access_token, TokenKind::Access)
            .is_some());
    }

    #[test]
    fn the_response_is_never_cached() {
        let context = context();
        let code = issue_code(&context, None);

        let response = handle_token(&context, &post(&exchange_body(&code), None));

        assert!(response
            .headers
            .iter()
            .any(|header| header.name == "Cache-Control" && header.value == "no-store"));
    }

    #[test]
    fn the_requested_resource_becomes_the_token_audience() {
        let context = context();
        let code = issue_code(&context, Some("https://mcp-home.jetdev.eu/mt-risks"));

        let response = handle_token(&context, &post(&exchange_body(&code), None));

        let access_token = body_of(&response)["access_token"]
            .as_str()
            .unwrap()
            .to_string();

        assert_eq!(
            context
                .signer
                .verify(&access_token, TokenKind::Access)
                .unwrap()
                .audience,
            Some("https://mcp-home.jetdev.eu/mt-risks".to_string())
        );
    }

    #[test]
    fn a_code_can_not_be_replayed() {
        let context = context();
        let code = issue_code(&context, None);

        assert_eq!(
            handle_token(&context, &post(&exchange_body(&code), None)).status_code,
            200
        );

        let replayed = handle_token(&context, &post(&exchange_body(&code), None));

        assert_eq!(replayed.status_code, 400);
        assert_eq!(body_of(&replayed)["error"], "invalid_grant");
    }

    #[test]
    fn a_wrong_code_verifier_is_refused() {
        let context = context();
        let code = issue_code(&context, None);

        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fclaude.ai%2Fapi%2Fmcp%2Fauth_callback&code_verifier=not-the-verifier&client_id=claude&client_secret=the-secret",
            code
        );

        let response = handle_token(&context, &post(&body, None));

        assert_eq!(response.status_code, 400);
        assert_eq!(body_of(&response)["error"], "invalid_grant");
    }

    #[test]
    fn a_mismatched_redirect_uri_is_refused() {
        let context = context();
        let code = issue_code(&context, None);

        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=http%3A%2F%2F127.0.0.1%3A9000%2Fcallback&code_verifier={}&client_id=claude&client_secret=the-secret",
            code, VERIFIER
        );

        let response = handle_token(&context, &post(&body, None));

        assert_eq!(body_of(&response)["error"], "invalid_grant");
    }

    #[test]
    fn a_wrong_client_secret_is_an_invalid_client() {
        let context = context();
        let code = issue_code(&context, None);

        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fclaude.ai%2Fapi%2Fmcp%2Fauth_callback&code_verifier={}&client_id=claude&client_secret=wrong",
            code, VERIFIER
        );

        let response = handle_token(&context, &post(&body, None));

        assert_eq!(response.status_code, 401);
        assert_eq!(body_of(&response)["error"], "invalid_client");
        assert!(response.register_ip_failure);
    }

    #[test]
    fn http_basic_client_authentication_is_accepted() {
        let context = context();
        let code = issue_code(&context, None);

        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri=https%3A%2F%2Fclaude.ai%2Fapi%2Fmcp%2Fauth_callback&code_verifier={}",
            code, VERIFIER
        );

        // base64("claude:the-secret")
        let response = handle_token(
            &context,
            &post(&body, Some("Basic Y2xhdWRlOnRoZS1zZWNyZXQ=")),
        );

        assert_eq!(response.status_code, 200);
    }

    #[test]
    fn a_failed_basic_authentication_carries_a_challenge() {
        let context = context();

        // base64("claude:wrong")
        let response = handle_token(
            &context,
            &post("grant_type=refresh_token", Some("Basic Y2xhdWRlOndyb25n")),
        );

        assert_eq!(response.status_code, 401);
        assert!(response
            .headers
            .iter()
            .any(|header| header.name == "WWW-Authenticate"));
    }

    #[test]
    fn a_refresh_token_yields_a_fresh_pair_with_the_same_grant() {
        let context = context();
        let code = issue_code(&context, Some("https://mcp-home.jetdev.eu/mt-risks"));

        let first = body_of(&handle_token(&context, &post(&exchange_body(&code), None)));
        let refresh_token = first["refresh_token"].as_str().unwrap();

        let body = format!(
            "grant_type=refresh_token&refresh_token={}&client_id=claude&client_secret=the-secret",
            refresh_token
        );

        let response = handle_token(&context, &post(&body, None));

        assert_eq!(response.status_code, 200);

        let refreshed = body_of(&response);
        assert_eq!(refreshed["scope"], "mcp offline_access");

        let access_token = refreshed["access_token"].as_str().unwrap();
        assert_eq!(
            context
                .signer
                .verify(access_token, TokenKind::Access)
                .unwrap()
                .audience,
            Some("https://mcp-home.jetdev.eu/mt-risks".to_string()),
            "the audience must survive a refresh, or the token stops matching its resource"
        );
    }

    #[test]
    fn an_access_token_is_not_accepted_as_a_refresh_token() {
        let context = context();
        let code = issue_code(&context, None);

        let access_token = body_of(&handle_token(&context, &post(&exchange_body(&code), None)))
            ["access_token"]
            .as_str()
            .unwrap()
            .to_string();

        let body = format!(
            "grant_type=refresh_token&refresh_token={}&client_id=claude&client_secret=the-secret",
            access_token
        );

        assert_eq!(
            body_of(&handle_token(&context, &post(&body, None)))["error"],
            "invalid_grant"
        );
    }

    #[test]
    fn an_unknown_grant_type_is_reported_with_the_registered_code() {
        let context = context();

        let response = handle_token(
            &context,
            &post(
                "grant_type=password&client_id=claude&client_secret=the-secret",
                None,
            ),
        );

        assert_eq!(body_of(&response)["error"], "unsupported_grant_type");
    }

    #[test]
    fn a_non_post_is_refused_with_the_allowed_method() {
        let context = context();

        let mut request = post("", None);
        request.method = OAuthMethod::Get;

        let response = handle_token(&context, &request);

        assert_eq!(response.status_code, 405);
    }

    #[test]
    fn an_empty_body_says_what_content_type_is_expected() {
        let context = context();

        let response = handle_token(&context, &post("", None));

        assert_eq!(response.status_code, 400);
        assert!(body_of(&response)["error_description"]
            .as_str()
            .unwrap()
            .contains("application/x-www-form-urlencoded"));
    }
}
