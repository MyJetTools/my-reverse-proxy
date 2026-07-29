use crate::configurations::HttpEndpointInfo;
use crate::h1_utils::{Http1Headers, Http1ResponseBuilder};
use crate::network_stream::*;
use crate::oauth::*;

use super::super::{H1Reader, HttpConnectionInfo};
use super::{BodyCollectorSink, NullSink};

/// Largest request body the OAuth endpoints will read into memory. A token
/// request is a few hundred bytes; anything near this is not one.
const MAX_OAUTH_BODY: usize = 64 * 1024;

/// What the gate decided about a request on an oauth-enabled endpoint.
pub enum OAuthGateOutcome {
    /// Either the endpoint has no `oauth:` block, or the request carries a
    /// valid access token. The request body has NOT been touched, so the normal
    /// path can stream it to the upstream.
    Proceed,
    /// The proxy answered this one itself and the body has been consumed, so the
    /// connection stays byte-synced and usable for the next request.
    Answered(Vec<u8>),
    /// The body could not be read to the end — the connection is out of sync and
    /// the only safe move is to close it.
    Close,
}

/// The h1 side of the OAuth server.
///
/// Runs before `find_location` on purpose: `/.well-known/…` and `/oauth/…` match
/// no configured location, so anything later in the pipeline would answer them
/// with the 503 "location is not found" page instead.
pub async fn run_oauth_gate<TReadPart: NetworkStreamReadPart + Send + Sync + 'static>(
    h1_reader: &mut H1Reader<TReadPart>,
    endpoint_info: &HttpEndpointInfo,
    http_connection_info: &HttpConnectionInfo,
    request_headers: &Http1Headers,
) -> OAuthGateOutcome {
    let Some(oauth_id) = endpoint_info.oauth.as_deref() else {
        return OAuthGateOutcome::Proceed;
    };

    let context = crate::app::APP_CTX
        .current_configuration
        .get(|config| config.oauth_credentials.get(oauth_id))
        .await;

    let Some(context) = context else {
        // The endpoint asked to be gated and the gate is missing — fail closed
        // rather than proxy an unauthenticated request to the MCP server.
        return answer(
            h1_reader,
            http_connection_info,
            request_headers,
            oauth_error_response(
                503,
                ERROR_SERVER_ERROR,
                "The oauth configuration for this endpoint is not loaded",
            ),
        )
        .await;
    };

    // Everything read out of the request buffer is copied before the body is
    // touched: reading the body needs `&mut h1_reader`, which ends the borrow.
    let request = RequestFacts::read(endpoint_info, request_headers, h1_reader);

    let Some(route) = route_oauth_path(&request.path) else {
        return gate_proxied_request(
            h1_reader,
            http_connection_info,
            request_headers,
            &context,
            &request,
        )
        .await;
    };

    // An OAuth endpoint: read the body (the token endpoint's parameters live in
    // it) and answer from the core.
    let body = match read_body(h1_reader, request_headers, true).await {
        Some(body) => body,
        None => return OAuthGateOutcome::Close,
    };

    let Some(body) = body else {
        return OAuthGateOutcome::Answered(build_response_bytes(&oauth_error_response(
            413,
            ERROR_INVALID_REQUEST,
            "The request body is too large for an oauth endpoint",
        )));
    };

    let location_paths: Vec<&str> = endpoint_info
        .locations
        .iter()
        .map(|location| location.path.as_str())
        .collect();

    let response = handle_oauth_request(
        &context,
        &OAuthRequest {
            method: OAuthMethod::parse(&request.method),
            route,
            base_url: &request.base_url,
            query: request.query.as_deref(),
            body: &body,
            content_type: request.content_type.as_deref(),
            authorization: request.authorization.as_deref(),
            known_resource_paths: &location_paths,
        },
    );

    register_credential_failure(http_connection_info, &response);

    OAuthGateOutcome::Answered(build_response_bytes(&response))
}

/// The bearer gate in front of everything this endpoint proxies.
async fn gate_proxied_request<TReadPart: NetworkStreamReadPart + Send + Sync + 'static>(
    h1_reader: &mut H1Reader<TReadPart>,
    http_connection_info: &HttpConnectionInfo,
    request_headers: &Http1Headers,
    context: &OAuthContext,
    request: &RequestFacts,
) -> OAuthGateOutcome {
    let result = check_bearer(
        context,
        &BearerGateRequest {
            authorization: request.authorization.as_deref(),
            base_url: context.base_url(&request.base_url),
            resource_path: &request.resource_path,
        },
    );

    match result {
        // The body is deliberately left alone here — the normal path streams it
        // to the upstream worker.
        BearerGateResult::Allowed => OAuthGateOutcome::Proceed,
        BearerGateResult::Challenge(response) => {
            answer(h1_reader, http_connection_info, request_headers, response).await
        }
    }
}

/// Consumes the request body (so the connection survives) and returns the
/// rendered response.
async fn answer<TReadPart: NetworkStreamReadPart + Send + Sync + 'static>(
    h1_reader: &mut H1Reader<TReadPart>,
    http_connection_info: &HttpConnectionInfo,
    request_headers: &Http1Headers,
    response: OAuthHttpResponse,
) -> OAuthGateOutcome {
    if read_body(h1_reader, request_headers, false).await.is_none() {
        return OAuthGateOutcome::Close;
    }

    register_credential_failure(http_connection_info, &response);

    OAuthGateOutcome::Answered(build_response_bytes(&response))
}

/// `None` when the body could not be read to the end; `Some(None)` when it was
/// read but exceeded the collection limit.
async fn read_body<TReadPart: NetworkStreamReadPart + Send + Sync + 'static>(
    h1_reader: &mut H1Reader<TReadPart>,
    request_headers: &Http1Headers,
    collect: bool,
) -> Option<Option<Vec<u8>>> {
    // The head is still sitting in the loop buffer — the normal path drops it in
    // `compile_headers`, which this one never reaches. Without this the body
    // transfer would start by replaying the request head.
    h1_reader.loop_buffer.commit_read(request_headers.end);

    let content_length = request_headers.content_length;

    if !collect {
        let mut sink = NullSink;
        h1_reader
            .transfer_body(0, &mut sink, content_length)
            .await
            .ok()?;
        return Some(Some(Vec::new()));
    }

    let mut sink = BodyCollectorSink::new(MAX_OAUTH_BODY);
    h1_reader
        .transfer_body(0, &mut sink, content_length)
        .await
        .ok()?;

    Some(sink.into_body())
}

/// The parts of the request the OAuth core needs, copied out of the read buffer.
struct RequestFacts {
    method: String,
    path: String,
    query: Option<String>,
    /// Scheme + `Host`, no trailing slash — this endpoint's own origin.
    base_url: String,
    /// Path of the location the request resolves to, or the request path when it
    /// resolves to none. What the 401 challenge points the client's metadata
    /// lookup at.
    resource_path: String,
    authorization: Option<String>,
    content_type: Option<String>,
}

impl RequestFacts {
    fn read<TReadPart: NetworkStreamReadPart + Send + Sync + 'static>(
        endpoint_info: &HttpEndpointInfo,
        request_headers: &Http1Headers,
        h1_reader: &H1Reader<TReadPart>,
    ) -> Self {
        let buffer = h1_reader.loop_buffer.get_data();

        let first_line = request_headers.get_first_line(buffer);
        let (verb, path_and_query) = first_line.get_verb_and_path();
        let method = verb.to_string();

        let (path, query) = match path_and_query.split_once('?') {
            Some((path, query)) => (path.to_string(), Some(query.to_string())),
            None => (path_and_query.to_string(), None),
        };

        // The endpoint was resolved from this same `Host`, so it names a vhost
        // this proxy serves — which is what makes it safe to build the issuer
        // out of it.
        let host = request_headers
            .find_header_value_str(buffer, b"host")
            .unwrap_or_else(|| endpoint_info.host_endpoint.as_str());

        // The challenge has to name the location, not the exact request path: an
        // MCP client posting to `/mt-risks/messages` must still be sent to the
        // metadata document for `/mt-risks`, whose `resource` is the URL the
        // user typed.
        let resource_path = match endpoint_info.find_location(&path) {
            Some(location) => normalize_resource_path(&location.path),
            None => normalize_resource_path(&path),
        };

        Self {
            method,
            path,
            query,
            base_url: build_base_url(endpoint_info.listen_endpoint_type.is_https_or_mcp(), host),
            resource_path,
            authorization: request_headers
                .find_header_value_str(buffer, b"authorization")
                .map(|value| value.to_string()),
            content_type: request_headers
                .find_header_value_str(buffer, b"content-type")
                .map(|value| value.to_string()),
        }
    }
}

fn build_response_bytes(response: &OAuthHttpResponse) -> Vec<u8> {
    let mut builder =
        Http1ResponseBuilder::new(response.status_code).add_content_type(response.content_type);

    for header in response.headers.iter() {
        builder = builder.add_header(header.name.as_str(), header.value.as_str());
    }

    builder.build_with_body(&response.body)
}

/// A wrong consent password or client secret is credential guessing, so it goes
/// to the existing block-list at the severity that blocks after a handful of
/// attempts.
fn register_credential_failure(
    http_connection_info: &HttpConnectionInfo,
    response: &OAuthHttpResponse,
) {
    if !response.register_ip_failure {
        return;
    }

    if let Some(ip) = http_connection_info.connection_ip.get_ip_addr() {
        crate::app::APP_CTX
            .ip_blocklist
            .register_failure(ip, crate::app::FailureSeverity::Hard);
    }
}
