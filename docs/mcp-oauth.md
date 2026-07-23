# OAuth 2.1 authorization server

This document describes the `oauth:` settings block: what it turns the proxy
into, how a request flows through it, and what it does and does not protect.

Written for the "Add custom connector" dialog on claude.ai, but nothing in it is
Claude-specific — it is a plain OAuth 2.1 authorization server with PKCE.

## Purpose

An MCP server behind the proxy typically does no authentication at all: it
listens on loopback and trusts whatever reaches it. Exposing it publicly means
the proxy has to be the gate.

`auth_header` (see [header-auth.md](header-auth.md)) already does that with a
static bearer token, and it is the simpler option whenever the client can be
configured with one. The connector dialog on claude.ai cannot: it offers an
**OAuth Client ID** and **Client Secret** and nothing else. There is no field for
a bearer token, and putting a token in the URL is forbidden by the MCP
specification. So for that client the proxy has to speak OAuth.

`oauth:` makes an endpoint do two things at once:

- serve the OAuth 2.1 authorization-server and discovery endpoints itself, and
- require an access token it minted on every request it proxies.

## Configuration

```yaml
oauth:
  claude:
    client_id: "${env:MCP_OAUTH_CLIENT_ID}"
    client_secret: "${env:MCP_OAUTH_CLIENT_SECRET}"
    consent_password: "${env:MCP_CONSENT_PASSWORD}"

hosts:
  "mcp-home.jetdev.eu:443":
    endpoint:
      type: https           # NOT `type: mcp` — see "Endpoint type" below
      ssl_certificate: my_ssl_cert
      oauth: claude
    locations:
    - path: /mt-risks
      type: mcp
      proxy_pass_to: http://127.0.0.1:8123/mt-risks
```

In the connector dialog the user then enters:

| Field         | Value                                  |
| ------------- | -------------------------------------- |
| URL           | `https://mcp-home.jetdev.eu/mt-risks`  |
| Client ID     | the `client_id` above                  |
| Client Secret | the `client_secret` above              |

and, once redirected to the proxy's consent screen, the `consent_password`.

### Fields

| Field                   | Required | Meaning                                                                                                        |
| ----------------------- | -------- | -------------------------------------------------------------------------------------------------------------- |
| `client_id`             | yes      | The pre-registered client. Checked on `/oauth/authorize` and `/oauth/token`.                                     |
| `client_secret`         | yes      | Checked on `/oauth/token`, via the form body or HTTP Basic.                                                      |
| `consent_password`      | yes      | What a human types on the consent screen to approve the connector.                                               |
| `public_url`            | no       | Issuer override. Defaults to the request's own scheme + `Host`. Set it when something in front rewrites either.  |
| `signing_key`           | no       | Pins the token signing key. Any sufficiently random string; used as raw bytes.                                   |
| `signing_key_file`      | no       | Where a generated key is kept. Default `~/.my-reverse-proxy-oauth/{block_id}.json`.                              |
| `access_token_ttl_sec`  | no       | Default 3600.                                                                                                    |
| `refresh_token_ttl_sec` | no       | Default 2592000 (30 days).                                                                                       |

All string fields go through `${variable}` substitution, so the secrets can live
in the environment rather than in the settings file.

`oauth:` can also be set on an `endpoint_templates:` entry, like `google_auth`.

### Endpoint type

**Use `type: https` on the endpoint and `type: mcp` on the location.**

The proxy has three independent request paths, and only two of them parse HTTP:

| Endpoint type       | Path                                                   | OAuth |
| ------------------- | ------------------------------------------------------ | ----- |
| `http` / `https`    | `src/h1_proxy_server/pipeline/reader.rs`               | yes   |
| `http2` / `https2`  | hyper, `src/http_proxy_pass/http_proxy_pass.rs`        | yes   |
| `mcp`               | `src/tcp_listener/mcp/run_mcp_connection.rs`, raw bytes | no    |

`type: mcp` is a byte-for-byte TCP bridge that never looks at the request, so
there is nothing for an authorization server to hook into. Setting `oauth:` on
one is refused when the configuration is compiled, with a message pointing at
the combination above — the proxy will not start rather than serve the MCP
server unauthenticated.

## The flow

```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant C as claude.ai
    participant B as Browser
    participant P as my-reverse-proxy<br/>(AS + gate)
    participant M as Upstream MCP

    Note over C,P: 1. Discovery
    C->>P: POST /mt-risks (no token)
    P-->>C: 401 + WWW-Authenticate:<br/>Bearer resource_metadata="…/mt-risks"
    C->>P: GET /.well-known/oauth-protected-resource/mt-risks
    P-->>C: { resource, authorization_servers:[…] }
    C->>P: GET /.well-known/oauth-authorization-server
    P-->>C: { authorize, token, S256, offline_access }

    Note over C,B,P: 2. Authorization code + PKCE
    C->>C: generate verifier + S256 challenge
    C->>B: open /oauth/authorize?…
    B->>P: GET /oauth/authorize
    P-->>B: consent screen
    U->>B: types the consent password
    B->>P: POST /oauth/authorize
    P-->>B: 302 → https://claude.ai/api/mcp/auth_callback?code&state
    C->>P: POST /oauth/token (code + verifier + client secret)
    P->>P: redeem code, verify PKCE
    P-->>C: { access_token, refresh_token, expires_in }

    Note over C,M: 3. Normal traffic
    C->>P: POST /mt-risks + Bearer …
    P->>P: verify signature, expiry, audience
    P->>M: proxied, Authorization stripped
    M-->>P: response
    P-->>C: response

    Note over C,P: 4. Refresh, no user involved
    C->>P: POST /oauth/token grant_type=refresh_token
    P-->>C: new access + refresh token
```

## Endpoints the proxy answers itself

These are matched **before** `find_location`, so they need no location entry —
which is also why they have to be intercepted early: they match no location and
would otherwise get the 503 "location is not found" page.

| Path                                                | Method     | What it is                                             |
| --------------------------------------------------- | ---------- | ------------------------------------------------------ |
| `/.well-known/oauth-authorization-server[/<path>]`  | GET        | RFC 8414 authorization server metadata                 |
| `/.well-known/openid-configuration[/<path>]`        | GET        | The same document; the MCP discovery chain probes it    |
| `/.well-known/oauth-protected-resource[/<path>]`    | GET        | RFC 9728 protected resource metadata, one per location |
| `/oauth/authorize`                                  | GET / POST | Consent screen, and the consent coming back            |
| `/oauth/token`                                      | POST       | `authorization_code` and `refresh_token` grants        |

A location whose path starts with `/oauth` or `/.well-known` is shadowed by
these on an oauth-enabled endpoint.

### What the metadata says, and why

- **`code_challenge_methods_supported: ["S256"]`** — not optional in practice.
  Claude reads its absence as "this server does not support PKCE" and refuses to
  start the flow. `plain` is never accepted; OAuth 2.1 removed it.
- **`scopes_supported` includes `offline_access`** — Claude only asks for a
  refresh token when the server advertises it. Without it the connector
  re-prompts the user every time the access token expires.
- **`resource` in the protected resource metadata** must be byte-for-byte the URL
  the user typed, path included. Claude compares the two and aborts on a
  mismatch — this is why the metadata is served per location path and why the
  default port is stripped from the `Host` when the issuer is built.
- **`authorization_servers`** — Claude reads only the first entry.

A protected resource document is only served for a path some configured location
covers; anything else is a 404, so a typo in the connector dialog fails loudly
instead of looking like a working setup.

## The gate

Every request on an oauth-enabled endpoint that is not one of the paths above
must carry `Authorization: Bearer <token>` minted by this proxy. Otherwise:

```http
HTTP/1.1 401 Unauthorized
WWW-Authenticate: Bearer resource_metadata="https://mcp-home.jetdev.eu/.well-known/oauth-protected-resource/mt-risks"
```

**401, not 200.** Claude only looks for `WWW-Authenticate` on a 401; a 200 with
the header attached is read as "this server needs no authorization" and ends
discovery before it starts.

The challenge names the *location*, not the exact request path, so an MCP client
posting to `/mt-risks/messages` is still pointed at the metadata document whose
`resource` is the URL the user typed.

The gate runs after the endpoint's mTLS and IP allow-list checks and before
`find_location`, so `whitelisted_ip` and `client_certificate_ca` still apply, and
they apply to the OAuth endpoints too.

## Tokens

Access and refresh tokens are **stateless**: `v1.<base64url(claims)>.<HMAC-SHA256>`.
Nothing is stored server-side, so validation is a signature check, there is no
token table to grow without bound, and a token minted before a restart still
works after one.

That last property depends on the signing key surviving the restart. It does:
the key is generated on first use and written to `signing_key_file`
(`0600`, replaced atomically), or pinned with `signing_key`. If the file is
deleted, every issued token stops verifying and the connector has to be
authorized again — which is why a file that exists but does not parse is a
startup error rather than a silent regeneration.

Claims are the kind (access or refresh — one is never accepted where the other is
expected), the expiry, the granted scope, and the audience.

**Audience binding.** When the client sends the RFC 8707 `resource` parameter —
Claude does — it is carried into the token. A token minted for
`https://host/mt-risks` then opens `/mt-risks` and everything under it, and
nothing else: a second MCP server at `/other-mcp` on the same host will not
accept it. A token with no audience covers the whole endpoint.

Authorization codes are the one piece of state kept in memory: single use, five
minute TTL, bounded store. Losing them to a restart costs one retry of the
consent screen.

## Security properties

- **PKCE S256 is mandatory.** A request without `code_challenge` is refused. An
  explicit `code_challenge_method` other than `S256` is refused. A missing one is
  treated as S256 — a client that actually meant `plain` still fails at the token
  endpoint, where the verifier is hashed and compared.
- **Redirect URIs are fixed, not registered.** Only
  `https://claude.ai/api/mcp/auth_callback` and RFC 8252 loopback
  (`127.0.0.1`, `localhost`, `[::1]`, any port, which is what Claude Code needs)
  are accepted. Lookalikes (`localhost.evil.example`) and embedded credentials
  (`http://localhost@evil.example/`) are not.
- **An untrusted client is never redirected.** A bad `client_id` or a
  `redirect_uri` this server will not honour produces an error page, not a
  redirect — otherwise `/oauth/authorize` would be an open redirector. Errors
  after those two checks do go back to the client, as OAuth errors, so Claude can
  report them.
- **Errors use the RFC 6749 codes** (`invalid_grant`, `invalid_client`,
  `unsupported_grant_type`, …). A custom string reads to Claude as an unknown
  failure and it gives up instead of retrying.
- **The token never reaches the upstream.** `Authorization` is added to the
  endpoint's request header remove-list whenever `oauth:` is set, which the MCP
  authorization specification requires. (Endpoint level, because location-level
  `modify_http_headers` is ignored on the h1 path.) If the upstream needs its own
  credential, add it with endpoint-level `modify_http_headers.add`.
- **Credential guessing feeds the IP block-list.** A wrong consent password or
  client secret is registered as a hard failure, so the existing fail2ban-style
  block-list picks it up after a handful of attempts.
- **Constant-time comparison** for the client id, client secret, consent
  password, token signature and PKCE challenge.
- **Bounded bodies.** The OAuth endpoints read at most 64 KiB, and always drain
  the request body to the end so a keep-alive connection stays byte-synced.
- **Codes are single use**, taken out of the store on redemption, and bound to
  the `redirect_uri` they were issued for.

## Limitations

- **No Dynamic Client Registration.** The client is pre-registered — the user
  types the same id and secret into the connector dialog. `/oauth/register` does
  not exist.
- **One client per block.** Configure a second `oauth:` block for a second
  client; blocks have independent signing keys.
- **No per-user identity.** The consent password is a single shared secret, like
  `auth_header`. Tokens carry no user, so `allowed_users` is not driven by OAuth.
  For user identity use `google_auth` or client certificates.
- **No revocation endpoint.** Stateless tokens cannot be revoked individually;
  rotating `signing_key` (or deleting `signing_key_file`) invalidates all of
  them at once.
- **Refresh tokens are not rotated.** The client is confidential — it proves
  itself with its secret on every token request — so rotation is not required.
- **`type: mcp` endpoints are not supported**, see "Endpoint type".

## Troubleshooting

| Symptom                                                     | Likely cause                                                                                     |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Claude never opens the consent screen                       | Discovery failed. Check `GET /.well-known/oauth-protected-resource/<path>` returns 200, not 404 — the path must match a configured location. |
| "Could not connect" right after entering the URL            | The first request got something other than 401. Check `oauth:` is set on the **endpoint**, not a location. |
| Consent screen appears, then the callback errors            | `resource` mismatch. The `resource` in the metadata must equal the URL typed in the dialog; set `public_url` if something in front rewrites the host or scheme. |
| Token request fails with `invalid_request`                  | The body was not `application/x-www-form-urlencoded`. The error description says what was received. |
| The connector asks to re-authorize after every proxy restart | The signing key is not persisting. Check `signing_key_file` is writable, or pin `signing_key`.     |
| The connector re-prompts every hour                         | No refresh token was issued, i.e. `offline_access` was not granted. It is advertised by default; check the client requested it. |
| 401 with `error="invalid_token"` on a fresh token           | Audience mismatch — the token was minted for a different location on the same host.               |

Timeouts on the Claude side are 10 s for discovery and token requests and 30 s
for refresh; its traffic comes from `160.79.104.0/21`.

## Files involved

- `src/oauth/` — the whole protocol, transport-independent. `handle_oauth_request`
  takes a method, route, query, body and headers and returns a status, headers
  and a body; it touches no socket, which is what lets both pipelines share it.
  - `oauth_route.rs` — path → `OAuthRoute`; what the proxy answers itself.
  - `handle_oauth_request.rs` — the dispatcher, plus the protected-resource
    metadata check against the endpoint's locations.
  - `handle_authorize.rs`, `consent_page.rs` — the consent screen and the code.
  - `handle_token.rs` — both grants and client authentication.
  - `bearer_gate.rs` — the 401 challenge and audience binding.
  - `metadata.rs`, `base_url.rs` — the two discovery documents and the issuer.
  - `token_signer.rs`, `hmac_sha256.rs`, `pkce.rs`, `secrets.rs` — the crypto.
  - `auth_codes.rs`, `auth_codes_inner.rs` — the in-memory code store.
  - `signing_key_storage.rs` — the key that has to outlive a restart.
  - `form.rs`, `oauth_request.rs`, `oauth_response.rs`, `oauth_error.rs` —
    form-urlencoded parsing and the request/response shapes.
- `src/settings/oauth_settings.rs` — the YAML block.
- `src/settings/end_point_settings.rs`, `src/settings/endpoint_template_settings.rs`
  — the `oauth:` reference on an endpoint or template.
- `src/settings_compiled/populate_settings.rs` — `${variable}` substitution.
- `src/configurations/oauth_credentials.rs` — the loaded blocks; a reload that
  changes nothing keeps the context and its in-flight codes.
- `src/scripts/get_oauth_credentials.rs` — resolves the block, validates it, and
  loads or generates the signing key.
- `src/scripts/compile_http_configuration.rs` — wires it onto the endpoint and
  refuses `type: mcp`.
- `src/configurations/http_endpoint_info.rs` — carries `oauth` and adds
  `Authorization` to the request header remove-list.
- `src/h1_proxy_server/pipeline/oauth_gate.rs` — the h1 side.
- `src/h1_proxy_server/pipeline/body_collector_sink.rs` — bounded body collection.
- `src/h1_proxy_server/pipeline/reader.rs` — resolves the endpoint before the
  location so the endpoint-wide checks cover the OAuth paths, then calls the gate.
- `src/http_proxy_pass/handle_oauth.rs` — the h2 side, calling the same core.
- `src/http_proxy_pass/http_proxy_pass.rs` — calls it before the location is
  resolved.
