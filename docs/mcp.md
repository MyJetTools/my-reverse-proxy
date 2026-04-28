# MCP (Model Context Protocol) support

This document describes how the reverse proxy handles Model Context Protocol
traffic, the design decisions behind the implementation, and known
limitations.

## Two unrelated `mcp` concepts

The keyword `mcp` appears in two different scopes with completely different
semantics:

1. **`endpoint.type = mcp`** — a TLS-wrapped raw TCP tunnel.
   Implemented in `src/tcp_listener/mcp/run_mcp_connection.rs`.
   The listener terminates TLS and then bidirectionally pipes bytes to a
   single configured upstream `host:port`. It does **not** parse HTTP at all.
   `https://` upstreams are rejected (the tunnel does not do TLS-to-TLS
   bridging). Unix sockets are not supported.

2. **`location.type = mcp`** — an HTTP/1 proxy-pass with URI rewrite.
   This is the focus of the rest of this document. It lives inside an
   ordinary HTTP(S) endpoint and uses the regular `h1_proxy_server` request
   pipeline. On every request the first line of the HTTP request is
   rewritten so the upstream sees a fixed, configured path.

These two share only a name; they do not share code.

## Why the rewrite is correct for Streamable HTTP MCP

The Streamable HTTP MCP transport (spec 2025-03-26) puts the entire protocol
on a **single URL** per server (e.g. `/mcp`). Every operation — tool calls,
notifications, the long-lived "listening" SSE stream, session teardown —
travels as `POST` / `GET` / `DELETE` to that one URL. The protocol carries
all method and routing information inside the JSON-RPC body, and the session
identifier is sent as the `Mcp-Session-Id` header.

Because of that, a path-rewriting reverse proxy is a natural fit:

- The user exposes one MCP server under any path they like
  (e.g. `https://mcp.domain.com/service-a`).
- The proxy rewrites every request's first line so the upstream always
  receives requests on its real endpoint path (e.g. `/mcp` or `/`).
- Method, body, and headers (including `Mcp-Session-Id`) are forwarded
  unchanged.

This is just standard reverse-proxy path rewriting; the only thing
"MCP-specific" about it is that we do **not** try to preserve the original
client path or query string — there is no useful information in them for an
MCP upstream.

The legacy "HTTP+SSE" MCP transport (deprecated, two-endpoint with
`/messages?sessionId=…` and `/sse`) is **not** supported by this rewrite,
because the legacy transport encodes session state in the query string and
the rewrite drops the query. We only target Streamable HTTP.

## Target use case

The motivating scenario is hosting many MCP servers behind one domain:

```
endpoint: mcp.domain.com (https)
locations:
  - path: /service-a   type: mcp   proxy_pass_to: http://service-a-host/
  - path: /service-b   type: mcp   proxy_pass_to: http://service-b-host/
  - path: /service-c   type: mcp   proxy_pass_to: http://service-c-host/mcp
```

Clients configure their MCP client as `https://mcp.domain.com/service-a`,
the proxy translates this into a request to `http://service-a-host/`, the
upstream sees `/` (or `/mcp` for service-c) regardless of what path the
client sent. Sessions, streaming, and method dispatch all continue to work
because they live in body and headers, not in the URL.

## Implementation

### Type-level invariants

`mcp` is HTTP/1 only. The encoding makes this an invariant on the type
system, not a runtime flag:

- `ProxyPassToConfig` has a dedicated variant `McpHttp1(ProxyPassToModel)`.
- `LocationType::Mcp` always compiles to that variant in
  `compile_location_proxy_pass_to.rs`.
- There is no `is_mcp` boolean anywhere — the variant *is* the marker.

This makes it impossible to construct an MCP location over HTTP/2 or over
unix sockets by mistake.

### Listener-side invariants

`ListenHttpEndpointType::Mcp` (used only by `endpoint.type = mcp`) advertises
`http/1.1` over ALPN and lives in `can_be_under_the_same_port` next to
`Https1` only. It cannot share a port with HTTP/2-only listeners.

### Path rewrite

The path-and-query string from the configured `proxy_pass_to` is captured
once when an MCP upstream is opened and stored on the `Mcp` variant of
`UpstreamState` (see "State machine" below). On every request, the
server loop reads it from `UpstreamAccess::mcp_path` and passes it to
`H1Reader::compile_headers`, which replaces the request's first-line
path-and-query via `Http1Headers::push_first_line_with_other_path` before
forwarding to the upstream.

The method (POST/GET/DELETE), HTTP version, and all headers are preserved.

The rewrite is idempotent: if the listen path equals the upstream path the
result is identical to the input.

### State machine: `UpstreamState`

For each client TCP, `serve_reverse_proxy` keeps a single `UpstreamState`
that captures everything the proxy knows about upstream connections for
that client. It is a three-state enum:

```rust
pub enum UpstreamState {
    Unknown,                                                // no requests yet
    Http(HashMap<i64, Upstream>),                           // key: location.id
    Mcp { location_id: i64, upstream: Upstream, mcp_path: String },
}
```

After the first request, the client TCP is "marked": the path resolves to
either a regular http/https location (transition to `Http`) or to an MCP
location (transition to `Mcp`). HTTP/1 framing on the client plus normal
client behavior (an MCP client never emits non-MCP requests on the same TCP
and vice versa) keeps the state stable for the rest of the connection.

The state machine is permissive: cross-protocol transitions are allowed
silently (drop previous state's contents, build a fresh state of the new
kind). This is safe because by HTTP/1 framing the previous response was
already delivered before the next request arrived. In practice transitions
do not trigger on real MCP/HTTP traffic.

For MCP, only **one** upstream connection lives at a time. Multiple MCP
requests on the same client TCP reuse it as long as they target the same
`location.id`; a request to a different MCP location replaces the upstream
(typical real-world deployments target one MCP server per client TCP).

For HTTP, the variant holds a per-`location.id` `HashMap` so several
different non-MCP locations on the same endpoint can each keep their own
upstream alive across requests on a keep-alive client connection.

The decision logic lives in `UpstreamState::get_or_connect`
(`src/h1_remote_connection/upstream_state.rs`), exposed via the small
`UpstreamAccess<'a> { upstream, mcp_path }` borrow. The server loop is
mcp-agnostic — it never matches on the variant or branches on `is_mcp`.

### Diagnostics

`ProxyPassToConfig::get_type_as_str()` returns `"mcp"` for `McpHttp1` so the
admin `/configuration` endpoint reflects the real configured type, not
`"http1"`.

## Known limitations

### HTTP/1 head-of-line on the client side

The proxy server side speaks HTTP/1 only. `H1ServerWritePart` serializes
responses in request-arrival order: a response may only be flushed to the
client when its request is at position 0 of the `current_requests` queue,
and is buffered otherwise (`src/h1_proxy_server/h1_server_write_part.rs`).

Consequence: if a single client TCP connection issues request A (returns
SSE — never ends) and then issues request B over the same TCP via keep-alive,
B's response will be buffered indefinitely behind A. By HTTP/1 framing this
should not happen for compliant clients (request B cannot be sent before
response A is fully read), so this is mostly a defensive limitation against
misbehaving clients.

In practice, modern Streamable HTTP clients open separate TCP connections
for the long-lived listening stream and for short request/response calls,
so they do not hit this limit. Clients that pipeline or reuse a single TCP
across SSE-returning requests will block.

There is no plan to fix this on HTTP/1: the proper solution is HTTP/2
multiplexing on the listener side, which removes the head-of-line block at
the protocol level. That work is part of the longer-term HTTP/2 migration.

### MCP location under an HTTP/2 listener

`location.type = mcp` is only meaningful under an HTTP/1 or HTTPS/1
listener, where requests flow through `h1_proxy_server` and the rewrite
runs. Under an HTTPS/2 listener the request flows through the legacy
hyper-based pipeline (`create_data_source` in `proxy_pass_location_config.rs`),
which does **not** apply the MCP rewrite. `McpHttp1` is currently merged
with `Http1` in that path, so the request would silently be forwarded
without the path rewrite — MCP semantics would not work.

This is a silent misconfiguration risk; if you place `type: mcp` locations,
keep the surrounding endpoint at `type: http` / `https` (HTTP/1).

### Upstream URL must include the path

`MyReverseProxyRemoteEndpoint::get_path_and_query()` falls back to `/` when
the upstream URL has no path. If your MCP upstream listens on a non-root
path (e.g. `/mcp`), make sure to write it explicitly:

```
proxy_pass_to: http://upstream-host/mcp   # explicit path
```

A bare `http://upstream-host` will rewrite to `GET / HTTP/1.1`, which most
MCP servers will return 404 for.

### Compression on MCP locations

`compress: true` on a location wraps the response in gzip. For SSE streams
this would buffer events until the gzip window flushes, breaking real-time
event delivery. Don't enable `compress` on MCP locations.

### Auth on MCP locations

If the endpoint has `g_auth` (Google login redirect), an MCP client that
isn't a browser will receive a 302 redirect and fail. MCP clients are
typically programmatic; place MCP endpoints behind plain bearer auth (via
upstream) or no proxy-level auth, not behind redirect-based login flows.

## Files involved

- `src/configurations/proxy_pass_to_config.rs` — `ProxyPassToConfig::McpHttp1`
- `src/scripts/compile_location_proxy_pass_to.rs` — `LocationType::Mcp` →
  `McpHttp1`
- `src/h1_remote_connection/upstream.rs` — `Upstream` (the upstream
  connection type, mcp-agnostic)
- `src/h1_remote_connection/upstream_state.rs` — `UpstreamState` enum and
  `UpstreamAccess`
- `src/h1_proxy_server/server_loop.rs` — owns `UpstreamState` per client TCP
- `src/h1_proxy_server/h1_read_part.rs` — applies `mcp_path` in
  `compile_headers`
- `src/h1_utils/http_headers.rs` — `push_first_line_with_other_path`
- `src/configurations/http_type.rs` — `ListenHttpEndpointType::Mcp` ALPN
  rules
- `src/tcp_listener/mcp/run_mcp_connection.rs` — the unrelated
  `endpoint.type = mcp` raw TLS tunnel
