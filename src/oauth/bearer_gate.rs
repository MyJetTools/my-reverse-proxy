use super::{
    build_resource_metadata_url, error_body, OAuthContext, OAuthHttpResponse, TokenKind,
    ERROR_INVALID_TOKEN,
};

/// What the gate needs to know about a request that is on its way to an
/// upstream.
pub struct BearerGateRequest<'s> {
    /// Raw `Authorization` header value, if the request carried one.
    pub authorization: Option<&'s str>,
    /// Scheme + host of this endpoint, no trailing slash.
    pub base_url: &'s str,
    /// Path of the location this request resolves to — what the challenge
    /// advertises as the protected resource, so Claude fetches the metadata
    /// document whose `resource` matches the URL the user typed.
    pub resource_path: &'s str,
}

pub enum BearerGateResult {
    /// The request may go on to the upstream.
    Allowed,
    /// A 401 carrying the `WWW-Authenticate` header that starts the OAuth flow.
    Challenge(OAuthHttpResponse),
}

/// The gate in front of every proxied request on an oauth-enabled endpoint.
///
/// The answer to a missing or bad token must be **401**, never 200 with a
/// challenge header attached: Claude only looks for `WWW-Authenticate` on a 401,
/// and a 200 is read as "this server needs no authorization", which ends the
/// discovery chain before it starts.
pub fn check_bearer(context: &OAuthContext, request: &BearerGateRequest) -> BearerGateResult {
    let Some(token) = bearer_token(request.authorization) else {
        // No credentials at all — RFC 6750 says the challenge carries no `error`
        // in this case, it is simply an invitation to authenticate.
        return BearerGateResult::Challenge(challenge(request, None));
    };

    let Some(verified) = context.signer.verify(token, TokenKind::Access) else {
        return BearerGateResult::Challenge(challenge(
            request,
            Some("The access token is invalid or has expired"),
        ));
    };

    if let Some(audience) = verified.audience.as_deref() {
        if !audience_allows(audience, request.base_url, request.resource_path) {
            return BearerGateResult::Challenge(challenge(
                request,
                Some("The access token was issued for a different resource"),
            ));
        }
    }

    BearerGateResult::Allowed
}

/// The token out of an `Authorization: Bearer …` header, if that is what it is.
pub fn bearer_token(authorization: Option<&str>) -> Option<&str> {
    let authorization = authorization?.trim();

    if authorization.len() <= 7 || !authorization[..7].eq_ignore_ascii_case("Bearer ") {
        return None;
    }

    let token = authorization[7..].trim();

    if token.is_empty() {
        return None;
    }

    Some(token)
}

/// Whether a token minted for `audience` may be used on `resource_path`.
///
/// A token whose audience is the endpoint root covers everything on it; one
/// minted for a specific path covers that path and what sits below it, and
/// nothing else. This is what stops a token handed out for one MCP server on the
/// host from being replayed against another.
fn audience_allows(audience: &str, base_url: &str, resource_path: &str) -> bool {
    let audience = audience.trim_end_matches('/');
    let base_url = base_url.trim_end_matches('/');

    let Some(audience_path) = audience.strip_prefix(base_url) else {
        return false;
    };

    if audience_path.is_empty() || audience_path == "/" {
        return true;
    }

    // The audience must be a path, not the rest of another host that happens to
    // start with ours (`https://host.evil` against a base of `https://host`).
    if !audience_path.starts_with('/') {
        return false;
    }

    let resource_path = if resource_path.is_empty() {
        "/"
    } else {
        resource_path
    };

    if resource_path.eq_ignore_ascii_case(audience_path) {
        return true;
    }

    match resource_path.get(..audience_path.len()) {
        Some(prefix) => {
            prefix.eq_ignore_ascii_case(audience_path)
                && resource_path.as_bytes().get(audience_path.len()) == Some(&b'/')
        }
        None => false,
    }
}

fn challenge(request: &BearerGateRequest, error_description: Option<&str>) -> OAuthHttpResponse {
    let metadata_url = build_resource_metadata_url(request.base_url, request.resource_path);

    let mut header_value = format!("Bearer resource_metadata=\"{}\"", metadata_url);

    let body = match error_description {
        Some(description) => {
            header_value.push_str(&format!(
                ", error=\"{}\", error_description=\"{}\"",
                ERROR_INVALID_TOKEN,
                description.replace('"', "'")
            ));
            error_body(ERROR_INVALID_TOKEN, description)
        }
        None => error_body(ERROR_INVALID_TOKEN, "Authorization is required"),
    };

    OAuthHttpResponse::json(401, body).add_header("WWW-Authenticate", header_value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::*;
    use crate::settings::OAuthSettings;

    const BASE_URL: &str = "https://mcp-home.jetdev.eu";

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

    fn gate_request<'s>(
        authorization: Option<&'s str>,
        resource_path: &'s str,
    ) -> BearerGateRequest<'s> {
        BearerGateRequest {
            authorization,
            base_url: BASE_URL,
            resource_path,
        }
    }

    fn challenge_of(result: BearerGateResult) -> OAuthHttpResponse {
        match result {
            BearerGateResult::Allowed => panic!("expected a challenge"),
            BearerGateResult::Challenge(response) => response,
        }
    }

    fn is_allowed(result: BearerGateResult) -> bool {
        matches!(result, BearerGateResult::Allowed)
    }

    fn www_authenticate(response: &OAuthHttpResponse) -> String {
        response
            .headers
            .iter()
            .find(|header| header.name == "WWW-Authenticate")
            .map(|header| header.value.clone())
            .expect("the challenge must carry WWW-Authenticate")
    }

    #[test]
    fn a_request_without_a_token_is_challenged_with_401_and_the_metadata_url() {
        let context = context();

        let response = challenge_of(check_bearer(&context, &gate_request(None, "/mt-risks")));

        // 200 with the header would end Claude's discovery before it starts.
        assert_eq!(response.status_code, 401);
        assert_eq!(
            www_authenticate(&response),
            "Bearer resource_metadata=\"https://mcp-home.jetdev.eu/.well-known/oauth-protected-resource/mt-risks\""
        );
    }

    #[test]
    fn a_first_challenge_carries_no_error_parameter() {
        let context = context();

        let response = challenge_of(check_bearer(&context, &gate_request(None, "/mt-risks")));

        assert!(!www_authenticate(&response).contains("error="));
    }

    #[test]
    fn a_bad_token_is_challenged_as_an_invalid_token() {
        let context = context();

        let response = challenge_of(check_bearer(
            &context,
            &gate_request(Some("Bearer not-a-real-token"), "/mt-risks"),
        ));

        assert_eq!(response.status_code, 401);
        assert!(www_authenticate(&response).contains("error=\"invalid_token\""));
    }

    #[test]
    fn a_valid_token_passes() {
        let context = context();
        let token = context
            .signer
            .mint(TokenKind::Access, "mcp", None, 3600)
            .unwrap();

        assert!(is_allowed(check_bearer(
            &context,
            &gate_request(Some(&format!("Bearer {}", token)), "/mt-risks")
        )));
    }

    #[test]
    fn the_bearer_scheme_is_matched_case_insensitively() {
        let context = context();
        let token = context
            .signer
            .mint(TokenKind::Access, "mcp", None, 3600)
            .unwrap();

        assert!(is_allowed(check_bearer(
            &context,
            &gate_request(Some(&format!("bearer {}", token)), "/mt-risks")
        )));
    }

    #[test]
    fn a_refresh_token_does_not_open_the_gate() {
        let context = context();
        let token = context
            .signer
            .mint(TokenKind::Refresh, "mcp", None, 3600)
            .unwrap();

        assert!(!is_allowed(check_bearer(
            &context,
            &gate_request(Some(&format!("Bearer {}", token)), "/mt-risks")
        )));
    }

    #[test]
    fn a_token_bound_to_one_resource_does_not_open_another() {
        let context = context();
        let token = context
            .signer
            .mint(
                TokenKind::Access,
                "mcp",
                Some("https://mcp-home.jetdev.eu/mt-risks"),
                3600,
            )
            .unwrap();

        let header = format!("Bearer {}", token);

        assert!(is_allowed(check_bearer(
            &context,
            &gate_request(Some(&header), "/mt-risks")
        )));
        // A sub-path of what was granted is still covered.
        assert!(is_allowed(check_bearer(
            &context,
            &gate_request(Some(&header), "/mt-risks/messages")
        )));
        // A sibling MCP server on the same host is not.
        assert!(!is_allowed(check_bearer(
            &context,
            &gate_request(Some(&header), "/other-mcp")
        )));
        // Nor is a path that merely starts with the same characters.
        assert!(!is_allowed(check_bearer(
            &context,
            &gate_request(Some(&header), "/mt-risks-admin")
        )));
    }

    #[test]
    fn a_token_bound_to_another_host_is_refused() {
        let context = context();
        let token = context
            .signer
            .mint(
                TokenKind::Access,
                "mcp",
                Some("https://elsewhere.example/mt-risks"),
                3600,
            )
            .unwrap();

        assert!(!is_allowed(check_bearer(
            &context,
            &gate_request(Some(&format!("Bearer {}", token)), "/mt-risks")
        )));
    }

    #[test]
    fn an_endpoint_wide_token_covers_every_location() {
        let context = context();
        let token = context
            .signer
            .mint(
                TokenKind::Access,
                "mcp",
                Some("https://mcp-home.jetdev.eu"),
                3600,
            )
            .unwrap();

        assert!(is_allowed(check_bearer(
            &context,
            &gate_request(Some(&format!("Bearer {}", token)), "/anything")
        )));
    }

    #[test]
    fn the_challenge_for_the_root_resource_has_no_path_suffix() {
        let context = context();

        let response = challenge_of(check_bearer(&context, &gate_request(None, "/")));

        assert_eq!(
            www_authenticate(&response),
            "Bearer resource_metadata=\"https://mcp-home.jetdev.eu/.well-known/oauth-protected-resource\""
        );
    }

    #[test]
    fn other_authorization_schemes_are_not_bearer_tokens() {
        assert!(bearer_token(Some("Basic Y2xhdWRlOng=")).is_none());
        assert!(bearer_token(Some("Bearer")).is_none());
        assert!(bearer_token(Some("Bearer   ")).is_none());
        assert!(bearer_token(None).is_none());
        assert_eq!(bearer_token(Some("Bearer  the-token ")), Some("the-token"));
    }
}
