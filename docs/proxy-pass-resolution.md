# ProxyPass → Location resolution

This document traces how an incoming connection is matched to a configured
location and what happens after the match for each `ProxyPassToConfig`
variant.

## The three resolution stages

Resolution happens in three sequential stages, each narrowing what we know
about the incoming traffic:

1. **Listening port → `ListenConfiguration`** — happens at TCP `accept`.
2. **`ListenConfiguration` → `HttpEndpointInfo`** — happens after TLS
   handshake (for TLS listeners) or after the first HTTP request's `Host`
   header is read (for plain HTTP listeners).
3. **`HttpEndpointInfo` → `ProxyPassLocationConfig`** — happens per
   request, by matching the request's path prefix.

---

## Stage 1: port → `ListenConfiguration`

The proxy binds one `tokio::net::TcpListener` per configured TCP port (and
one `tokio::net::UnixListener` per unix socket). Each port carries exactly
one `ListenConfiguration` enum from
`src/configurations/app_configuration_inner.rs`:

```rust
pub enum ListenConfiguration {
    Http(Arc<HttpListenPortConfiguration>),
    Tcp(Arc<TcpEndpointHostConfig>),
    Mcp(Arc<HttpListenPortConfiguration>),
}
```

When the listener accepts a connection, `handle_accepted_connection`
(`src/tcp_listener/listen_tcp_server.rs`) looks up the port in
`AppConfigurationInner::listen_tcp_endpoints` and dispatches:

- `ListenConfiguration::Http(_)` — match on `listen_endpoint_type`:
  - `Http1` → `h1_proxy_server::kick_h1_tcp_reverse_proxy_server_from_http`
    (plain HTTP/1, no TLS).
  - `Http2` → `tcp_listener::http2::handle_connection` (h2 over plain TCP,
    legacy hyper-based).
  - `Https1` → `tcp_listener::https::handle_connection`, which terminates
    TLS and then forks on `endpoint_info.listen_endpoint_type` again
    (`Https1` → h1 server, `Https2` → hyper h2 server).
  - `Https2` → same as `Https1`.
  - `Mcp` (in this branch) → `tcp_listener::https::handle_connection` →
    after TLS handshake, the inner branch for `Mcp` calls
    `mcp::run_mcp_connection` which is a raw byte tunnel (the legacy
    `endpoint.type = mcp` mode).
- `ListenConfiguration::Tcp(_)` — pure TCP forward, no HTTP parsing.
  Branches on `remote_host` (`Gateway` / `OverSsh` / `Direct`) and forwards
  bytes to the upstream.
- `ListenConfiguration::Mcp(_)` — TLS listener for `endpoint.type = mcp`.
  Goes through `tcp_listener::https::handle_connection`, terminates TLS,
  then `mcp::run_mcp_connection` pipes bytes to a single configured
  upstream `host:port`.

The legality of multiple endpoints on one port is governed by
`ListenHttpEndpointType::can_be_under_the_same_port`
(`src/configurations/http_type.rs`):

| New type | Same-port partners allowed |
|---|---|
| `Http1` | `Http1` |
| `Http2` | `Http2` |
| `Https1` | `Https1`, `Https2`, `Mcp` |
| `Https2` | `Https1`, `Https2` |
| `Mcp` | `Https1`, `Https2`, `Mcp` |

Any mismatch is rejected at config compile time
(`src/scripts/merge_http_configuration_with_existing_port.rs`).

---

## Stage 2: `ListenConfiguration` → `HttpEndpointInfo`

For TLS listeners (`Https1` / `Https2` / `Mcp`):

1. TLS accept reads SNI from the ClientHello.
2. `HttpListenPortConfiguration::get_http_endpoint_info(Some(sni))`
   (`src/configurations/http_listen_port_configuration.rs:63`) walks
   `endpoints: Vec<Arc<HttpEndpointInfo>>` and returns the first whose
   `host_endpoint.is_my_server_name(sni)` is true.
3. The chosen `HttpEndpointInfo` carries the SSL cert id, allowed user
   list, modify-headers config, and the per-endpoint `locations`.

For plain HTTP/1 listeners (`Http1`):

1. There is no SNI, so endpoint resolution is deferred until the first
   request's `Host` header is read.
2. `H1Reader::try_find_endpoint_info`
   (`src/h1_proxy_server/h1_read_part.rs:76`) extracts the `Host` header
   value and calls the same `get_http_endpoint_info`.
3. The result is cached on `HttpConnectionInfo.endpoint_info` for the
   lifetime of the client TCP — subsequent requests on the same TCP do
   not re-resolve.

### Special case: single-endpoint port without server name

If the port hosts exactly one endpoint and that endpoint's
`host_endpoint` was configured **without** a server name (e.g. just
`":443"`), resolution succeeds with `server_name = None` and returns that
single endpoint. This supports configs like "all HTTPS traffic on port
443 goes to the same backend regardless of SNI".

If multiple endpoints are configured on a port and SNI/Host is missing,
resolution returns `None` → request fails with
`ProxyServerError::HttpConfigurationIsNotFound`.

### Per-endpoint settings checked at this stage

Once the endpoint is resolved, the request stream is gated by:

- `whitelisted_ip_list_id` — client IP must be in the allowed list, or
  the connection is dropped immediately
  (`tcp_listener/https/handle_connection.rs:37`).
- `g_auth` (Google login) and/or `client_certificate_id` — checked at
  request time via `must_be_authorized` and the authorize step in
  `H1Reader::authorize`. Failures return 401/redirect responses, not
  proxy errors.
- `allowed_user_list_id` — checked after authentication;
  `HttpEndpointInfo::user_is_allowed` returns false → 401.

---

## Stage 3: `HttpEndpointInfo` → `ProxyPassLocationConfig`

Per request. The endpoint's `locations: Vec<Arc<ProxyPassLocationConfig>>`
is searched by `find_location` (`src/configurations/http_endpoint_info.rs:70`):

```rust
pub fn find_location(&self, path: &str) -> Option<&ProxyPassLocationConfig> {
    for location in self.locations.iter() {
        if location.path.len() > path.len() {
            continue;
        }
        let path_prefix = &path[..location.path.len()];
        if path_prefix.eq_ignore_ascii_case(&location.path) {
            return Some(&location);
        }
    }
    None
}
```

Important properties:

- **First match wins, in `Vec` order.** Not longest-prefix-first.
  Locations should be ordered with the most specific paths **first** in
  the config; otherwise a generic `/` match will shadow more specific
  ones.
- **Case-insensitive** prefix match.
- The path-and-query split is done before matching: only the path part
  is compared, query string is ignored
  (`H1HeadersFirstLine::get_path_and_query` returns the full
  path-and-query from the request line; matching is by prefix on this
  combined string, so a query never affects routing — `?foo=bar` cannot
  start with a configured path).
- A request on `/` matches a location with `path: "/"` only; an empty
  request path or no path is normalized upstream of this code.

If no location matches, the request fails with
`ProxyServerError::LocationIsNotFound` and the configured "location not
found" error template is returned to the client.

### Per-location settings

After match, the request is filtered/transformed by:

- `whitelisted_ip` — additional per-location IP filter (in addition to
  endpoint-level `whitelisted_ip_list_id`).
- `modify_request_headers` / `modify_response_headers` — add/remove/
  rewrite headers on request and response.
- `domain_name` — used for upstream TLS SNI when forwarding to an HTTPS
  upstream, overriding the request's `Host`.
- `compress` — enables gzip on responses (do not enable for
  `type: mcp` — see `docs/mcp.md`).
- `trace_payload` — debug logging.

---

## What happens after location resolution: per-variant

The resolved location's `proxy_pass_to: ProxyPassToConfig` decides the
upstream behavior. All variants are dispatched through `Upstream::connect`
(`src/h1_remote_connection/upstream.rs`), which builds the right
`UpstreamInner` and spawns the response read loop. The wrapping
`UpstreamState` (per client TCP) decides whether to reuse or open fresh
based on the variant — see "Upstream lifecycle per variant" below.

### `Http1(ProxyPassToModel)`

Plain HTTP/1 reverse proxy. Sub-cases by `remote_host`:

- `Direct { remote_host: http://...   }` →
  `UpstreamInner::Http1Direct` (raw `tokio::net::TcpStream`).
- `Direct { remote_host: https://... }` →
  `UpstreamInner::Https1Direct` (TLS-wrapped TCP). Uses
  `domain_name` for SNI if configured, else the URL host.
- `OverSsh { ssh_credentials, remote_host }` →
  `UpstreamInner::Http1OverSsh` (HTTP/1 over an SSH tunnel; see
  `docs/gateway-protocol.md` for unrelated gateway concept).
- `Gateway { id, remote_host }` →
  `UpstreamInner::Http1OverGateway` (HTTP/1 forwarded through a
  pre-existing MyJetTools gateway connection).

The request is forwarded as-is: first line, headers, body. Response is
streamed back via the per-upstream `response_read_loop` task.

### `McpHttp1(ProxyPassToModel)`

Same wire transports as `Http1` (Direct/OverSsh/Gateway, http or https),
but with one difference: on every request the first line of the HTTP
request is rewritten so the upstream sees the path-and-query of the
configured `proxy_pass_to`, not the path the client sent. Method, HTTP
version, and headers are preserved.

The path-and-query is captured once when the upstream is opened and
stored on `UpstreamState::Mcp { mcp_path, .. }`; it is read by
`H1Reader::compile_headers` via the `UpstreamAccess::mcp_path` field on
each request. See `docs/mcp.md` for the full rationale.

### `Http2(ProxyPassToModel)`

HTTP/2 upstream. **Currently not active** in the new h1 server path —
`Upstream::connect`'s `Http2 ⇒ todo!()` arm panics if reached. h2
upstream lives only in the legacy hyper-based pipeline
(`ProxyPassLocationConfig::create_data_source`), which is wired to the
HTTPS/2 listener via `tcp_listener::https::kick_off_https2`.

### `UnixHttp1(ProxyPassToModel)`

HTTP/1 over a unix domain socket. Only `Direct { remote_host }` is
implemented; `Gateway` and `OverSsh` panic in
`Upstream::connect` with `todo!()` because they are nonsensical
combinations on a local socket transport. Reaching those is a config
error, not a runtime expectation.

### `UnixHttp2(ProxyPassToModel)`

HTTP/2 over unix socket — currently `todo!("Not Implemented")`. Same
caveat as `Http2`: only the legacy hyper path uses this, via
`HttpProxyPassContentSource::UnixHttp2`.

### `FilesPath(ProxyPassFilesPathModel)`

Serves static files from a path. The path may itself be remote:

- `Direct { remote_host }` — path on the proxy's local filesystem;
  `LocalPathContent` is used directly.
- `OverSsh` — files are read over SSH (SFTP-like).
- `Gateway` — files are read through a MyJetTools gateway.

Per-request flow: `Upstream::connect` constructs an
`UpstreamInner::LocalFiles(Arc<LocalPathContent>)` (no upstream TCP),
and `read_http_response` spawns `execute_local_path` which reads the
file and writes the HTTP response synthesized from the file contents.

### `Static(Arc<StaticContentConfig>)`

Returns a configured static body (status code, content-type, body
bytes). No upstream connection at all. `Upstream::connect` builds
`UpstreamInner::StaticContent(...)`; `read_http_response` spawns
`execute_static_content` which writes the canned response.

---

## Upstream lifecycle per variant

How the upstream is held inside `UpstreamState`
(`src/h1_remote_connection/upstream_state.rs`):

| Variant | State holding the connection | Reuse behavior |
|---|---|---|
| `Http1` (any sub-variant) | `UpstreamState::Http(HashMap<location.id, Upstream>)` | Reused across requests on the same client TCP for the same location.id. New location.id → new entry. |
| `McpHttp1` | `UpstreamState::Mcp { location_id, upstream, mcp_path }` | Reused for same `location.id`. Different `location.id` (multi-mcp-service config) drops and replaces. |
| `UnixHttp1` | Same as `Http1`. | Same as `Http1`. |
| `Http2` / `UnixHttp2` | (Goes through legacy hyper path; this state machine is not used.) | n/a |
| `FilesPath` / `Static` | Same as `Http1` (held by `location.id` in `Http` variant). | Reused — but these have no real connection, so reuse is essentially free. |

State transitions when the request type doesn't match the current
`UpstreamState` variant are silently allowed (drop previous contents,
build fresh state) but in practice never trigger: HTTP/1 framing on
the client side plus normal client behavior keeps a client TCP on
exactly one protocol family for its lifetime.

### WebSocket upgrade

Only http (non-mcp) requests can perform a WebSocket upgrade. When the
request handshake completes, `serve_reverse_proxy` calls
`UpstreamState::take_http(location_id)` to extract the underlying
upstream out of the `Http` variant; the upstream is handed to
`H1ServerWritePart::add_web_socket_upgrade` along with the client read
half, and `serve_reverse_proxy` returns. From then on the WebSocket
frames flow as raw bytes between the two halves
(`response_read_loop` continues pumping bytes; `H1ServerWritePart`
no longer mediates).

`take_http` returns `None` if the state is `Mcp` or `Unknown`, which
matches reality — MCP upstreams never WebSocket-upgrade.

---

## Worked examples

### Example 1: `https://api.example.com/v1/users`

1. Port 443 → `ListenConfiguration::Http(Https1Config)`.
2. TLS handshake reads SNI `api.example.com` →
   `get_http_endpoint_info(Some("api.example.com"))` returns the matching
   `HttpEndpointInfo`.
3. After TLS termination, h1 reader reads the request `GET /v1/users HTTP/1.1`.
4. `find_location("/v1/users")` walks the endpoint's locations:
   - location 0: `path: "/v1/users"` → match, returns it.
   - (The next location, e.g. `path: "/"`, is skipped because the search
     stopped.)
5. Location's `proxy_pass_to: Http1(http://users-svc:8080/)`.
6. `UpstreamState::get_or_connect` is called; state is `Unknown` → opens
   a fresh upstream, transitions to
   `Http(HashMap { location.id → upstream })`.
7. Request first line and headers are forwarded as-is. Response streams
   back through `response_read_loop` → `H1ServerWritePart` → client.

### Example 2: `https://mcp.example.com/service-a` (POST, MCP)

1. Port 443 → `ListenConfiguration::Http(Https1Config)`.
2. TLS handshake reads SNI `mcp.example.com` → endpoint resolved.
3. h1 reader reads `POST /service-a HTTP/1.1`.
4. `find_location("/service-a")` matches the location with
   `path: "/service-a"`, `proxy_pass_to: McpHttp1(http://upstream-a/mcp)`.
5. `UpstreamState::get_or_connect` opens a fresh upstream, transitions to
   `Mcp { location_id, upstream, mcp_path: "/mcp" }`.
6. `H1Reader::compile_headers` sees `mcp_path: Some("/mcp")` — rewrites
   the first line to `POST /mcp HTTP/1.1`. Headers (including
   `Mcp-Session-Id`) and body are forwarded unchanged.
7. Response streams back. If it's an SSE stream, the `response_read_loop`
   keeps pumping until upstream closes or client disconnects.

### Example 3: TLS-only endpoint without SNI

Config: port 443 has one endpoint with `host_endpoint: ":443"` (no
server name).

1. Client connects without SNI.
2. `get_http_endpoint_info(None)` succeeds because `endpoints.len() == 1`
   and the single endpoint has no server name — returns it directly.
3. Resolution proceeds normally.

If the same port had two endpoints (one with SNI, one without), a
no-SNI client would get `None` and the connection would be dropped after
TLS handshake.

### Example 4: Plain HTTP with `Host` header missing

Config: port 80 with multiple HTTP/1 endpoints.

1. `get_http_endpoint_info(None)` is called because the request had no
   `Host` header.
2. Multiple endpoints exist → returns `None`.
3. Request fails with `HttpConfigurationIsNotFound` → "configuration not
   found" error template returned.

If the port has exactly one endpoint without a server name, the
fallback applies and the request resolves to that single endpoint.

---

## Files involved

- `src/tcp_listener/listen_tcp_server.rs` — port-level dispatch
  (Stage 1).
- `src/tcp_listener/listen_unix_server.rs` — same for unix sockets.
- `src/tcp_listener/https/handle_connection.rs` — TLS termination and
  inner type-based dispatch.
- `src/configurations/http_listen_port_configuration.rs` —
  `get_http_endpoint_info` (Stage 2).
- `src/configurations/host_str.rs` — `EndpointHttpHostString::is_my_server_name`.
- `src/configurations/http_endpoint_info.rs` — `find_location` (Stage 3).
- `src/h1_proxy_server/h1_read_part.rs` — Stage 2/3 plumbing for h1
  (`try_find_endpoint_info`, `find_location`).
- `src/h1_proxy_server/server_loop.rs` — request loop, owns
  `UpstreamState`.
- `src/h1_remote_connection/upstream.rs` — `Upstream::connect`,
  per-variant connect logic.
- `src/h1_remote_connection/upstream_state.rs` — per-client-TCP state
  machine.
- `src/configurations/proxy_pass_to_config.rs` — the `ProxyPassToConfig`
  enum itself.
- `src/scripts/merge_http_configuration_with_existing_port.rs` —
  port-sharing validation.
