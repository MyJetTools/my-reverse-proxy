use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Full, Limited};

use crate::{configurations::HttpEndpointInfo, oauth::*, types::ConnectionIp};

/// Largest request body the OAuth endpoints will read into memory — the same
/// bound the h1 gate applies.
const MAX_OAUTH_BODY: usize = 64 * 1024;

/// What the gate decided about a request on an oauth-enabled h2 endpoint.
pub enum OAuthGateH2Outcome {
    /// Either the endpoint has no `oauth:` block, or the request carries a valid
    /// access token. The request is handed back untouched, body included.
    Proceed(hyper::Request<hyper::body::Incoming>),
    /// The proxy answered this one itself.
    Answered(hyper::Response<BoxBody<Bytes, String>>),
}

/// The h2 side of the OAuth server.
///
/// Mirrors the h1 gate (`h1_proxy_server::pipeline::run_oauth_gate`) and calls
/// the same core, so the two pipelines answer identically. It runs before
/// `find_location_index` for the same reason: the OAuth server's own paths match
/// no configured location.
pub async fn run_oauth_gate_h2(
    endpoint_info: &HttpEndpointInfo,
    connection_ip: &ConnectionIp,
    request: hyper::Request<hyper::body::Incoming>,
) -> OAuthGateH2Outcome {
    let Some(oauth_id) = endpoint_info.oauth.as_deref() else {
        return OAuthGateH2Outcome::Proceed(request);
    };

    let context = crate::app::APP_CTX
        .current_configuration
        .get(|config| config.oauth_credentials.get(oauth_id))
        .await;

    let Some(context) = context else {
        // The endpoint asked to be gated and the gate is missing — fail closed
        // rather than proxy an unauthenticated request to the MCP server.
        return answered(
            connection_ip,
            oauth_error_response(
                503,
                ERROR_SERVER_ERROR,
                "The oauth configuration for this endpoint is not loaded",
            ),
        );
    };

    let path = request.uri().path().to_string();
    let query = request.uri().query().map(|query| query.to_string());
    let method = OAuthMethod::parse(request.method().as_str());

    let base_url = build_base_url(
        endpoint_info.listen_endpoint_type.is_https_or_mcp(),
        &host_of(endpoint_info, &request),
    );

    let authorization = header_of(&request, "authorization");
    let content_type = header_of(&request, "content-type");

    let Some(route) = route_oauth_path(&path) else {
        // The challenge names the location, not the exact request path, so an
        // MCP client posting to `/mt-risks/messages` is still pointed at the
        // metadata document whose `resource` is the URL the user typed.
        let resource_path = match endpoint_info.find_location(&path) {
            Some(location) => normalize_resource_path(&location.path),
            None => normalize_resource_path(&path),
        };

        let result = check_bearer(
            &context,
            &BearerGateRequest {
                authorization: authorization.as_deref(),
                base_url: context.base_url(&base_url),
                resource_path: &resource_path,
            },
        );

        return match result {
            BearerGateResult::Allowed => OAuthGateH2Outcome::Proceed(request),
            BearerGateResult::Challenge(response) => answered(connection_ip, response),
        };
    };

    let body = match Limited::new(request.into_body(), MAX_OAUTH_BODY)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return answered(
                connection_ip,
                oauth_error_response(
                    413,
                    ERROR_INVALID_REQUEST,
                    "The request body is too large for an oauth endpoint",
                ),
            )
        }
    };

    let location_paths: Vec<&str> = endpoint_info
        .locations
        .iter()
        .map(|location| location.path.as_str())
        .collect();

    let response = handle_oauth_request(
        &context,
        &OAuthRequest {
            method,
            route,
            base_url: &base_url,
            query: query.as_deref(),
            body: &body,
            content_type: content_type.as_deref(),
            authorization: authorization.as_deref(),
            known_resource_paths: &location_paths,
        },
    );

    answered(connection_ip, response)
}

fn answered(connection_ip: &ConnectionIp, response: OAuthHttpResponse) -> OAuthGateH2Outcome {
    // A wrong consent password or client secret is credential guessing, so it
    // goes to the existing block-list.
    if response.register_ip_failure {
        if let Some(ip) = connection_ip.get_ip_addr() {
            crate::app::APP_CTX
                .ip_blocklist
                .register_failure(ip, crate::app::FailureSeverity::Hard);
        }
    }

    OAuthGateH2Outcome::Answered(to_hyper_response(&response))
}

fn to_hyper_response(response: &OAuthHttpResponse) -> hyper::Response<BoxBody<Bytes, String>> {
    let mut builder = hyper::Response::builder()
        .status(response.status_code)
        .header("content-type", response.content_type);

    for header in response.headers.iter() {
        builder = builder.header(header.name.as_str(), header.value.as_str());
    }

    let body = Full::from(Bytes::from(response.body.clone()))
        .map_err(crate::to_hyper_error)
        .boxed();

    match builder.body(body) {
        Ok(response) => response,
        // A header value the http crate refuses would otherwise panic here. The
        // values are all built by this crate, so this is a guard rather than an
        // expected path — but a panic in a request handler is not an option.
        Err(err) => hyper::Response::builder()
            .status(500)
            .header("content-type", CONTENT_TYPE_JSON)
            .body(
                Full::from(Bytes::from(error_body(
                    ERROR_SERVER_ERROR,
                    &format!("Can not build the oauth response. Err: {}", err),
                )))
                .map_err(crate::to_hyper_error)
                .boxed(),
            )
            .expect("a status and one static header always build"),
    }
}

/// The authority this request was addressed to: the h2 `:authority`
/// pseudo-header, or the `Host` header on the h1-over-hyper path.
fn host_of(
    endpoint_info: &HttpEndpointInfo,
    request: &hyper::Request<hyper::body::Incoming>,
) -> String {
    if let Some(authority) = request.uri().authority() {
        return authority.as_str().to_string();
    }

    if let Some(host) = header_of(request, "host") {
        return host;
    }

    endpoint_info.host_endpoint.as_str().to_string()
}

fn header_of(request: &hyper::Request<hyper::body::Incoming>, name: &str) -> Option<String> {
    request
        .headers()
        .get(name)?
        .to_str()
        .ok()
        .map(|value| value.trim().to_string())
}
