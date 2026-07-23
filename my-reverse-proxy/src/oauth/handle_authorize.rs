use std::collections::HashMap;

use super::{
    generate_consent_page, generate_error_page, is_allowed_redirect_uri, method_not_allowed,
    percent_encode, random_secret, AuthCodes, ConsentPageParams, IssuedAuthCode, OAuthContext,
    OAuthHttpResponse, OAuthMethod, OAuthRequest, ERROR_INVALID_REQUEST,
    ERROR_UNSUPPORTED_RESPONSE_TYPE,
};

/// How many bytes of entropy an authorization code carries.
const AUTH_CODE_BYTES: usize = 32;

/// `GET` renders the consent screen; `POST` is that screen coming back with the
/// consent password.
pub fn handle_authorize(context: &OAuthContext, request: &OAuthRequest) -> OAuthHttpResponse {
    match request.method {
        OAuthMethod::Get | OAuthMethod::Post => {}
        OAuthMethod::Other => return method_not_allowed("GET, POST"),
    }

    let parameters = request.parameters();
    let base_url = context.base_url(request.base_url);

    let validated = match validate(context, &parameters, base_url) {
        Validated::Rejected(response) => return response,
        Validated::Accepted(validated) => validated,
    };

    if request.method == OAuthMethod::Get {
        return consent_page(&validated, None);
    }

    let presented_password = parameters
        .get("consent_password")
        .map(String::as_str)
        .unwrap_or_default();

    if !context.check_consent_password(presented_password) {
        // 401 rather than 200 so the block-list sees a credential failure, and
        // the same screen comes back so the user can simply retype it.
        return consent_page(&validated, Some("Wrong consent password. Try again."))
            .into_credential_failure();
    }

    issue_code(&context.codes, &validated)
}

/// Everything `/authorize` needs once the request has been checked over, shared
/// by the GET that renders the form and the POST that acts on it — the two must
/// agree on every value or the second could widen what the first showed.
struct ValidatedAuthorizeRequest<'s> {
    client_id: &'s str,
    redirect_uri: &'s str,
    state: Option<&'s str>,
    code_challenge: &'s str,
    granted_scope: String,
    resource: Option<&'s str>,
}

enum Validated<'s> {
    Accepted(ValidatedAuthorizeRequest<'s>),
    Rejected(OAuthHttpResponse),
}

fn validate<'s>(
    context: &OAuthContext,
    parameters: &'s HashMap<String, String>,
    base_url: &str,
) -> Validated<'s> {
    let client_id = get(parameters, "client_id");
    let redirect_uri = get(parameters, "redirect_uri");
    let state = parameters
        .get("state")
        .map(String::as_str)
        .filter(|state| !state.is_empty());

    // RFC 6749 §4.1.2.1: when the client or its redirect URI can not be trusted,
    // the error must NOT be sent to that redirect URI — otherwise the endpoint
    // becomes an open redirector that also leaks the error to an attacker.
    if !context.check_client_id(client_id) {
        return Validated::Rejected(
            OAuthHttpResponse::html(
                400,
                generate_error_page(
                    "Unknown client",
                    "The client_id is not configured on this proxy.",
                ),
            )
            .into_credential_failure(),
        );
    }

    if !is_allowed_redirect_uri(redirect_uri) {
        return Validated::Rejected(OAuthHttpResponse::html(
            400,
            generate_error_page(
                "Invalid redirect_uri",
                "This proxy only redirects to the Claude callback or to a loopback address.",
            ),
        ));
    }

    // From here the redirect URI is trusted, so failures go back to the client
    // as OAuth errors, which is what lets Claude report them properly.
    let response_type = get(parameters, "response_type");
    if response_type != "code" {
        return Validated::Rejected(redirect_with_error(
            redirect_uri,
            state,
            ERROR_UNSUPPORTED_RESPONSE_TYPE,
            "Only the authorization code flow is supported",
        ));
    }

    let code_challenge = get(parameters, "code_challenge");
    if code_challenge.is_empty() {
        return Validated::Rejected(redirect_with_error(
            redirect_uri,
            state,
            ERROR_INVALID_REQUEST,
            "code_challenge is required — this server requires PKCE with S256",
        ));
    }

    // An absent method is treated as S256 rather than as the RFC 7636 default of
    // `plain`: `plain` is not accepted at the token endpoint either way, so a
    // client that genuinely meant it still fails there, and a client that meant
    // S256 but omitted the field is not turned away for a formality.
    let code_challenge_method = parameters
        .get("code_challenge_method")
        .map(String::as_str)
        .unwrap_or("S256");

    if !code_challenge_method.eq_ignore_ascii_case("S256") {
        return Validated::Rejected(redirect_with_error(
            redirect_uri,
            state,
            ERROR_INVALID_REQUEST,
            "Only the S256 code_challenge_method is supported",
        ));
    }

    let resource = parameters
        .get("resource")
        .map(String::as_str)
        .filter(|resource| !resource.is_empty());

    // RFC 8707: a resource that is not on this server can not be granted.
    if let Some(resource) = resource {
        if !resource.trim_end_matches('/').starts_with(base_url) {
            return Validated::Rejected(redirect_with_error(
                redirect_uri,
                state,
                "invalid_target",
                "The requested resource is not served by this authorization server",
            ));
        }
    }

    Validated::Accepted(ValidatedAuthorizeRequest {
        client_id,
        redirect_uri,
        state,
        code_challenge,
        granted_scope: context.granted_scope(parameters.get("scope").map(String::as_str)),
        resource,
    })
}

fn consent_page(validated: &ValidatedAuthorizeRequest, error: Option<&str>) -> OAuthHttpResponse {
    let status_code = if error.is_some() { 401 } else { 200 };

    OAuthHttpResponse::html(
        status_code,
        generate_consent_page(&ConsentPageParams {
            client_id: validated.client_id,
            redirect_uri: validated.redirect_uri,
            state: validated.state,
            scope: &validated.granted_scope,
            code_challenge: validated.code_challenge,
            resource: validated.resource,
            error,
        }),
    )
}

fn issue_code(codes: &AuthCodes, validated: &ValidatedAuthorizeRequest) -> OAuthHttpResponse {
    let code = random_secret(AUTH_CODE_BYTES);

    codes.issue(
        code.clone(),
        IssuedAuthCode {
            redirect_uri: validated.redirect_uri.to_string(),
            code_challenge: validated.code_challenge.to_string(),
            scope: validated.granted_scope.clone(),
            resource: validated.resource.map(|itm| itm.to_string()),
        },
    );

    let mut url = append_query(validated.redirect_uri);
    url.push_str(&format!("code={}", percent_encode(&code)));

    if let Some(state) = validated.state {
        url.push_str(&format!("&state={}", percent_encode(state)));
    }

    OAuthHttpResponse::redirect(url)
}

fn redirect_with_error(
    redirect_uri: &str,
    state: Option<&str>,
    error: &str,
    description: &str,
) -> OAuthHttpResponse {
    let mut url = append_query(redirect_uri);

    url.push_str(&format!(
        "error={}&error_description={}",
        percent_encode(error),
        percent_encode(description)
    ));

    if let Some(state) = state {
        url.push_str(&format!("&state={}", percent_encode(state)));
    }

    OAuthHttpResponse::redirect(url)
}

/// Opens the query part of a redirect URI, keeping any parameters the client
/// already put there — RFC 6749 requires them to survive.
fn append_query(redirect_uri: &str) -> String {
    let mut url = redirect_uri.to_string();
    url.push(if redirect_uri.contains('?') { '&' } else { '?' });
    url
}

fn get<'s>(parameters: &'s HashMap<String, String>, name: &str) -> &'s str {
    parameters.get(name).map(String::as_str).unwrap_or_default()
}
