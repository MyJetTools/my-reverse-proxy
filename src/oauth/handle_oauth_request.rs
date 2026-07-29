use super::{
    build_authorization_server_metadata, build_protected_resource_metadata, handle_authorize,
    handle_token, method_not_allowed, normalize_resource_path, oauth_error_response, OAuthContext,
    OAuthHttpResponse, OAuthMethod, OAuthRequest, OAuthRoute, ERROR_INVALID_REQUEST,
};

/// The whole of the OAuth server, as one pure function: request in, response
/// out, no transport and no I/O. Both the h1 byte pipeline and the hyper-based
/// h2 path call this, which is what keeps their behaviour identical.
pub fn handle_oauth_request(context: &OAuthContext, request: &OAuthRequest) -> OAuthHttpResponse {
    match &request.route {
        OAuthRoute::AuthorizationServerMetadata => {
            if request.method != OAuthMethod::Get {
                return method_not_allowed("GET");
            }

            OAuthHttpResponse::metadata(build_authorization_server_metadata(
                context.base_url(request.base_url),
            ))
        }

        OAuthRoute::ProtectedResourceMetadata { resource_path } => {
            if request.method != OAuthMethod::Get {
                return method_not_allowed("GET");
            }

            protected_resource_metadata(context, request, resource_path)
        }

        OAuthRoute::Authorize => handle_authorize(context, request),

        OAuthRoute::Token => handle_token(context, request),
    }
}

fn protected_resource_metadata(
    context: &OAuthContext,
    request: &OAuthRequest,
    resource_path: &str,
) -> OAuthHttpResponse {
    // Only paths this endpoint actually serves get a document. Echoing back
    // whatever was asked for would advertise resources that do not exist and
    // make a typo in the connector dialog look like a working setup.
    if !resource_path_is_served(resource_path, request.known_resource_paths) {
        return oauth_error_response(
            404,
            ERROR_INVALID_REQUEST,
            "No such protected resource on this endpoint",
        );
    }

    OAuthHttpResponse::metadata(build_protected_resource_metadata(
        context.base_url(request.base_url),
        resource_path,
    ))
}

/// Whether some configured location covers this resource path, using the same
/// prefix rule `find_location` routes with.
fn resource_path_is_served(resource_path: &str, location_paths: &[&str]) -> bool {
    location_paths.iter().any(|location_path| {
        let location_path = normalize_resource_path(location_path);

        if location_path == "/" {
            return true;
        }

        if resource_path.eq_ignore_ascii_case(&location_path) {
            return true;
        }

        match resource_path.get(..location_path.len()) {
            Some(prefix) => {
                prefix.eq_ignore_ascii_case(&location_path)
                    && resource_path.as_bytes().get(location_path.len()) == Some(&b'/')
            }
            None => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::*;
    use crate::settings::OAuthSettings;

    const BASE_URL: &str = "https://mcp-home.jetdev.eu";
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

    fn request<'s>(
        method: OAuthMethod,
        path: &str,
        query: Option<&'s str>,
        body: &'s str,
        locations: &'s [&'s str],
    ) -> OAuthRequest<'s> {
        OAuthRequest {
            method,
            route: route_oauth_path(path).expect("the path must belong to the oauth server"),
            base_url: BASE_URL,
            query,
            body: body.as_bytes(),
            content_type: None,
            authorization: None,
            known_resource_paths: locations,
        }
    }

    fn header(response: &OAuthHttpResponse, name: &str) -> Option<String> {
        response
            .headers
            .iter()
            .find(|header| header.name == name)
            .map(|header| header.value.clone())
    }

    fn json(response: &OAuthHttpResponse) -> serde_json::Value {
        serde_json::from_slice(&response.body).unwrap()
    }

    #[test]
    fn the_authorization_server_metadata_is_served() {
        let context = context();

        let response = handle_oauth_request(
            &context,
            &request(
                OAuthMethod::Get,
                "/.well-known/oauth-authorization-server",
                None,
                "",
                &[],
            ),
        );

        assert_eq!(response.status_code, 200);
        assert_eq!(json(&response)["issuer"], BASE_URL);
    }

    #[test]
    fn the_openid_configuration_path_answers_with_the_same_document() {
        let context = context();

        let response = handle_oauth_request(
            &context,
            &request(
                OAuthMethod::Get,
                "/.well-known/openid-configuration",
                None,
                "",
                &[],
            ),
        );

        assert_eq!(json(&response)["issuer"], BASE_URL);
    }

    #[test]
    fn protected_resource_metadata_is_served_for_a_configured_location() {
        let context = context();

        let response = handle_oauth_request(
            &context,
            &request(
                OAuthMethod::Get,
                "/.well-known/oauth-protected-resource/mt-risks",
                None,
                "",
                &["/mt-risks"],
            ),
        );

        assert_eq!(response.status_code, 200);
        assert_eq!(
            json(&response)["resource"],
            "https://mcp-home.jetdev.eu/mt-risks",
            "the resource must be exactly the URL the user typed into the connector dialog"
        );
    }

    #[test]
    fn protected_resource_metadata_is_404_for_a_path_no_location_serves() {
        let context = context();

        let response = handle_oauth_request(
            &context,
            &request(
                OAuthMethod::Get,
                "/.well-known/oauth-protected-resource/typo",
                None,
                "",
                &["/mt-risks"],
            ),
        );

        assert_eq!(response.status_code, 404);
    }

    #[test]
    fn a_catch_all_location_serves_every_resource_path() {
        let context = context();

        let response = handle_oauth_request(
            &context,
            &request(
                OAuthMethod::Get,
                "/.well-known/oauth-protected-resource/anything",
                None,
                "",
                &["/"],
            ),
        );

        assert_eq!(response.status_code, 200);
    }

    #[test]
    fn metadata_documents_reject_a_post() {
        let context = context();

        let response = handle_oauth_request(
            &context,
            &request(
                OAuthMethod::Post,
                "/.well-known/oauth-authorization-server",
                None,
                "",
                &[],
            ),
        );

        assert_eq!(response.status_code, 405);
    }

    fn authorize_query() -> String {
        format!(
            "response_type=code&client_id=claude&redirect_uri={}&state=the-state&code_challenge={}&code_challenge_method=S256&scope=mcp+offline_access&resource={}",
            percent_encode(REDIRECT_URI),
            CHALLENGE,
            percent_encode("https://mcp-home.jetdev.eu/mt-risks"),
        )
    }

    #[test]
    fn a_valid_authorize_request_renders_the_consent_screen() {
        let context = context();
        let query = authorize_query();

        let response = handle_oauth_request(
            &context,
            &request(OAuthMethod::Get, "/oauth/authorize", Some(&query), "", &[]),
        );

        assert_eq!(response.status_code, 200);
        assert_eq!(response.content_type, CONTENT_TYPE_HTML);
        assert!(String::from_utf8_lossy(&response.body).contains("consent_password"));
    }

    #[test]
    fn an_unknown_client_is_not_redirected_anywhere() {
        let context = context();
        let query = authorize_query().replace("client_id=claude", "client_id=impostor");

        let response = handle_oauth_request(
            &context,
            &request(OAuthMethod::Get, "/oauth/authorize", Some(&query), "", &[]),
        );

        assert_eq!(response.status_code, 400);
        assert!(
            header(&response, "Location").is_none(),
            "redirecting an untrusted client turns /authorize into an open redirector"
        );
        assert!(response.register_ip_failure);
    }

    #[test]
    fn a_redirect_uri_this_server_does_not_know_is_refused_without_redirecting() {
        let context = context();
        let query = authorize_query().replace(
            &percent_encode(REDIRECT_URI),
            &percent_encode("https://evil.example/steal"),
        );

        let response = handle_oauth_request(
            &context,
            &request(OAuthMethod::Get, "/oauth/authorize", Some(&query), "", &[]),
        );

        assert_eq!(response.status_code, 400);
        assert!(header(&response, "Location").is_none());
    }

    #[test]
    fn a_request_without_pkce_is_sent_back_as_an_oauth_error() {
        let context = context();
        let query = authorize_query().replace(&format!("code_challenge={}", CHALLENGE), "");

        let response = handle_oauth_request(
            &context,
            &request(OAuthMethod::Get, "/oauth/authorize", Some(&query), "", &[]),
        );

        let location = header(&response, "Location").unwrap();

        assert_eq!(response.status_code, 302);
        assert!(location.starts_with(REDIRECT_URI));
        assert!(location.contains("error=invalid_request"));
        assert!(location.contains("state=the-state"));
    }

    #[test]
    fn a_plain_code_challenge_method_is_refused() {
        let context = context();
        let query =
            authorize_query().replace("code_challenge_method=S256", "code_challenge_method=plain");

        let response = handle_oauth_request(
            &context,
            &request(OAuthMethod::Get, "/oauth/authorize", Some(&query), "", &[]),
        );

        assert!(header(&response, "Location")
            .unwrap()
            .contains("error=invalid_request"));
    }

    #[test]
    fn a_resource_on_another_server_is_an_invalid_target() {
        let context = context();
        let query = authorize_query().replace(
            &percent_encode("https://mcp-home.jetdev.eu/mt-risks"),
            &percent_encode("https://elsewhere.example/mt-risks"),
        );

        let response = handle_oauth_request(
            &context,
            &request(OAuthMethod::Get, "/oauth/authorize", Some(&query), "", &[]),
        );

        assert!(header(&response, "Location")
            .unwrap()
            .contains("error=invalid_target"));
    }

    #[test]
    fn a_wrong_consent_password_re_renders_the_form_as_a_credential_failure() {
        let context = context();
        let body = format!("{}&consent_password=wrong", authorize_query());

        let response = handle_oauth_request(
            &context,
            &request(OAuthMethod::Post, "/oauth/authorize", None, &body, &[]),
        );

        assert_eq!(response.status_code, 401);
        assert!(response.register_ip_failure);
        assert!(String::from_utf8_lossy(&response.body).contains("Wrong consent password"));
    }

    /// The whole flow the connector walks: consent, code, token, and then the
    /// gate opening for the minted token.
    #[test]
    fn consent_produces_a_code_that_exchanges_for_a_working_token() {
        let context = context();

        let consent_body = format!("{}&consent_password=the-password", authorize_query());

        let consent = handle_oauth_request(
            &context,
            &request(
                OAuthMethod::Post,
                "/oauth/authorize",
                None,
                &consent_body,
                &[],
            ),
        );

        assert_eq!(consent.status_code, 302);

        let location = header(&consent, "Location").unwrap();
        assert!(location.contains("state=the-state"));

        let code = location
            .split("code=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap()
            .to_string();

        let token_body = format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&code_verifier={}&client_id=claude&client_secret=the-secret",
            code,
            percent_encode(REDIRECT_URI),
            VERIFIER
        );

        let token = handle_oauth_request(
            &context,
            &request(OAuthMethod::Post, "/oauth/token", None, &token_body, &[]),
        );

        assert_eq!(token.status_code, 200);

        let access_token = json(&token)["access_token"].as_str().unwrap().to_string();

        let allowed = check_bearer(
            &context,
            &BearerGateRequest {
                authorization: Some(&format!("Bearer {}", access_token)),
                base_url: BASE_URL,
                resource_path: "/mt-risks",
            },
        );

        assert!(matches!(allowed, BearerGateResult::Allowed));
    }
}
