# MCP (Model Context Protocol) support

How the reverse proxy handles Model Context Protocol traffic, what an MCP
location actually changes, and the limitations that remain.

## Two unrelated `mcp` concepts

The keyword `mcp` appears in two scopes with completely different semantics:

1. **`endpoint.type = mcp`** — a TLS-wrapped raw TCP tunnel. Implemented in
   `src/tcp_listener/mcp/run_mcp_connection.rs`. The listener terminates TLS and
   then pipes bytes to one configured upstream `host:port`. It does **not**
   parse HTTP at all — so it also cannot route by path: it always uses the
   endpoint's FIRST location. `https://` upstreams are rejected (no TLS-to-TLS
   bridging), unix sockets are not supported, and `oauth` is refused on it at
   config time (there is no HTTP layer to hook into).

2. **`location.type = mcp` / `location.type = mcp-h2`** — an ordinary HTTP
   proxy-pass with a URI rewrite. This is what the rest of this document is
   about, and what every real deployment uses: many MCP servers behind one
   domain, each on its own path.

These two share only a name; they do not share code.

## Why the rewrite is correct for Streamable HTTP MCP

The Streamable HTTP MCP transport puts the entire protocol on a **single URL**
per server (e.g. `/mcp`). Tool calls, notifications, the long-lived listening
SSE stream and session teardown all travel as `POST` / `GET` / `DELETE` to that
one URL; method and routing information live in the JSON-RPC body, and the
session identifier is the `Mcp-Session-Id` header.

So a path-rewriting reverse proxy is a natural fit:

```
endpoint: mcp.domain.com (https)          # http/1 endpoint -> `mcp` locations
locations:
  - path: /service-a   type: mcp      proxy_pass_to: http://service-a:8000/mcp
  - path: /service-b   type: mcp      proxy_pass_to: http://service-b:8000/mcp

endpoint: mcp2.domain.com (https2)        # http/2 endpoint -> `mcp-h2` locations
locations:
  - path: /service-c   type: mcp-h2   proxy_pass_to: http://service-c:8000/mcp
```

(`mcp-h2` under an http/1 endpoint is refused at config time — see "The two
request paths" below.)

The client is configured with `https://mcp.domain.com/service-a`; the upstream
always sees its own real path. Sessions, streaming and method dispatch keep
working because they live in body and headers, not in the URL.

The legacy "HTTP+SSE" two-endpoint transport (`/messages?sessionId=…` + `/sse`)
is **not** supported: it encodes session state in the query string, which the
rewrite drops.

## What an MCP location actually changes

Exactly three things, relative to a plain `http` / `http2` location:

1. **Path rewrite** — the request's path-and-query is replaced with the one from
   `proxy_pass_to` (`h1_remote_connection::mcp_path`). On the h1 byte pipeline
   this is applied by `H1Reader::compile_headers`
   (`Http1Headers::push_first_line_with_other_path`); on the hyper path by
   `rewrite_mcp_path` in `http_request_builder.rs`. Idempotent when listen path
   and upstream path are equal.
2. **Read timeout** — `DEFAULT_MCP_READ_TIMEOUT` (1 hour) instead of the
   endpoint's read timeout, because the listening SSE stream idles with no
   keepalive. On the byte pipeline this is set per request in the worker; on the
   hyper path it is `PoolParams::read_stream_timeout`, applied to every
   connection the location's pool creates.
3. **Failure reaction** — when the upstream cannot be reached the client
   connection is dropped (`ResponseEvent::Abort`) instead of an HTML error page
   being substituted. A JSON-RPC client cannot parse "Bad gateway"; the one
   signal it acts on is the transport dropping.

Everything else is the ordinary upstream machinery, deliberately: **upstream
selection and pooling for `mcp` are identical to `http1`, and for `mcp-h2`
identical to `http2`.** There is no MCP-specific connection holder, no
dedicated-connection carve-out, no separate GC.

## The pool key: for MCP the path is part of the identity

`connection_key` (`src/h1_remote_connection/upstream_state.rs`) keys an upstream
by protocol + remote host — **plus the upstream path for `mcp` / `mcp-h2`**.

That asymmetry is not an accident. An ordinary location forwards the client's
own path, so `host:port` fully identifies what a connection talks to. An MCP
location rewrites every request onto its configured path, so two MCP servers
published on one `host:port` under different paths are *different upstreams*;
sharing a connection between them is meaningless, and the key says so.

On the hyper path the equivalent identity is `PoolDesc.location_id`, which is
derived from `id_string` = `listen_host|path->type|upstream` — already
per-location.

### History — the bug this replaced

MCP used to bypass the pool entirely: each client TCP held ONE `McpUpstream`
slot, on the theory that "a client connection talks to exactly one MCP
endpoint". That is false. An MCP client (Claude Code, for one) reuses a single
keep-alive connection across several MCP servers on the same host. The slot was
not keyed, so a request for `/service-b` could be written onto the parked
connection to `/service-a` — and get service-a's answer. The slot, its
registry, its idle-timeout GC and `DEFAULT_MCP_IDLE_TIMEOUT` are gone; a keyed
pool has no such state to get wrong.

## The two request paths

Which machinery serves a location depends on the ENDPOINT type, not the
location type:

| endpoint type | request path | h1 upstream | h2 upstream |
|---|---|---|---|
| `http`, `https` | h1 byte pipeline (`h1_proxy_server::pipeline`) | `H1PoolHolder`, per client TCP, keyed | **none** |
| `http2`, `https2` | hyper path (`http_proxy_pass`) | `APP_CTX.h1_*_pools` (`upstream_h1_pool`) | `APP_CTX.h2_*_pools` (`upstream_h2_pool`) |

Consequence: **`mcp-h2` requires an `http2` / `https2` endpoint.** The byte
pipeline has no h2 upstream at all, so the combination is refused when the
configuration is compiled (`compile_http_configuration`) rather than failing
every request.

## Known limitations

### An SSE stream occupies an h1 upstream connection for its whole life

h1 is one request per connection, and the listening GET never ends. On the hyper
path the rented pool entry is held until the body ends, so N concurrent
listening streams to one location pin N entries out of `pool_size` (default 5);
short JSON-RPC POSTs beyond that are served by on-demand overflow connections
(`max_disposables`, default 50 per pool) and, past that, wait up to
`connect_timeout` and then fail. Raise `pool_size` on heavy MCP locations, or
use `mcp-h2` — multiplexing removes the problem entirely.

On the byte pipeline the same fact is harmless: connections are handed out by
value, an SSE response is not self-delimiting so it is simply never returned to
the pool, and the next request dials a fresh one.

### HTTP/1 head-of-line on the client side

Under an `https` (h1) endpoint the proxy serializes responses in request-arrival
order. A client that sends request B on the same TCP while request A's SSE
response is still open blocks B behind A. Compliant Streamable-HTTP clients open
separate connections for the listening stream and for calls, so this is
defensive; the real fix is an h2 listener, which removes head-of-line blocking
at the protocol level.

### `mcp-h2` gets an error page, not a dropped connection

Difference (3) above — drop the client connection instead of substituting an
error page — exists only on the h1 byte pipeline, which owns the client socket
and can emit `ResponseEvent::Abort`. The hyper path answers an unreachable
upstream with the ordinary 502/503 error page, so an `mcp-h2` location (and an
`mcp` location under an `http2` / `https2` endpoint) hands the MCP client an HTML
body it cannot parse. Not a regression — the hyper path always did this — but it
means only two of the three MCP differences hold there.

### Upstream URL must include the path

`get_path_and_query()` falls back to `/` when the upstream URL has no path, so
`proxy_pass_to: http://upstream-host` rewrites every request to `/` — which most
MCP servers 404. Write the path explicitly.

### Compression

`compress: true` gzips the response, which buffers SSE events until the window
flushes. Do not enable it on MCP locations.

### Auth

`google_auth` redirects to a browser login — a programmatic MCP client just
fails on the 302. Use the endpoint's `oauth` block (the proxy's own OAuth 2.1
server; it strips the `authorization` header before the upstream, as the MCP
authorization spec requires), per-location `auth_header`, or no proxy-level auth
at all. See [mcp-oauth.md](mcp-oauth.md).

## Files involved

- `src/configurations/proxy_pass_to_config.rs` — `McpHttp1` / `McpHttp2`
  variants and `is_mcp()`
- `src/settings/location_settings.rs`, `src/scripts/compile_location_proxy_pass_to.rs`
  — `mcp` / `mcp-h2` → those variants
- `src/scripts/compile_http_configuration.rs` — refuses `mcp-h2` under an h1
  endpoint
- `src/h1_remote_connection/upstream_state.rs` — `mcp_path`, `connection_key`
- `src/h1_proxy_server/pipeline/worker.rs` — read-timeout carve-out, `Abort`
  instead of an error page
- `src/http_proxy_pass/http_request_builder.rs` — `rewrite_mcp_path` (hyper path)
- `src/configurations/proxy_pass_location_config.rs` — content source + pool
  factory per location
- `src/timers/gc_pools_timer.rs` — mcp locations counted in the desired pool set
- `src/tcp_listener/mcp/run_mcp_connection.rs` — the unrelated
  `endpoint.type = mcp` raw TLS tunnel
