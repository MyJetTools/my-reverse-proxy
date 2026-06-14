# TODO — open follow-ups

Findings flagged during the CPU-leak / DDoS-defence review. Not yet implemented; capture here so they don't get lost.

---

## 1. Close TCP on parser-state errors (HIGH priority — security/CPU) — ✅ RESOLVED by the H1 pipeline rewrite (see §6)

`server_loop.rs` was deleted. The task-decomposed reader (`pipeline/reader.rs`) returns `ReaderStep::Close` on EVERY request error (parse / config / auth / upstream), so a garbage keep-alive stream is always closed instead of re-parsed in a loop. The storm is gone.

**Where (historical):** `src/h1_proxy_server/server_loop.rs`, the `Err(err)` branch of `serve_reverse_proxy`'s main `loop`.

**Problem.** Today the `close_after` flag only covers `LocationIsNotFound` and `HttpConfigurationIsNotFound`. After every other error we write an error template and **continue the loop**, waiting for the next request on the same keep-alive connection. For errors that mean *the TCP byte stream is no longer in a known state* this is unsafe:

- `HeadersParseError(_)` — we never found the `\r\n\r\n` boundary, so we don't know where the bad request ends and the next one (if any) starts.
- `ChunkHeaderParseError` — chunked body framing is desynchronized.
- `ParsingPayloadError(_)` — same reasoning, payload boundaries unknown.

A bot sending garbage on a keep-alive socket can keep us re-parsing random bytes forever, burning CPU on the very `Http1Headers::parse` path we already identified as the hotspot.

**Fix.** Extend the `close_after` predicate:

```rust
let close_after = matches!(
    &err,
    ProxyServerError::LocationIsNotFound
        | ProxyServerError::HttpConfigurationIsNotFound
        | ProxyServerError::HeadersParseError(_)
        | ProxyServerError::ChunkHeaderParseError
        | ProxyServerError::ParsingPayloadError(_)
);
```

Optionally make those three error templates also use `generate_layout_with_close` for consistency (a `Connection: close` header in the response).

---

## 2. Bound `domain_rps` cardinality (MEDIUM priority — observability)

**Where:** `src/app/rps_accumulator.rs`, `snapshot_and_reset`.

**Problem.** `RpsAccumulator::snapshot_and_reset` zeroes counts but **keeps every key** forever. For real product domains that's fine (a few dozen entries). But on a dev box exposed to the internet a scanner can send each request with a different `Host` (random subdomains, IP literals, fuzz strings) and the map will grow unbounded. Each label permanently lives in the prometheus registry → memory bloat + slower `/metrics` scrapes + label cardinality explosion in Grafana.

**Fix options (cheapest first).**

a. **Drop idle keys.** In `snapshot_and_reset`, after the snapshot, delete any key whose value was already zero (i.e. no activity this tick). Simple, no extra state. Loses keys that flap in and out, but is fine for the bot-spam case.

b. **Stricter:** require N consecutive idle ticks before drop (track `idle_ticks: u32` per key, reset on inc, increment on snapshot, drop when above threshold).

c. **Hard ceiling:** limit map to top-K entries by recent count (e.g. K=200). Beyond that, drop the smallest. Protects against pathological cardinality even from never-idle floods.

(a) is enough as a first cut.

---

## 3. h2 path — early reject on `Host` mismatch (LOW priority — defence-in-depth)

**Where:** `src/tcp_listener/http_request_handler/handle_requests.rs`.

**Problem.** On HTTPS with ALPN h2, `endpoint_info` is fixed at TLS handshake time (chosen via SNI). After that, hyper hands us each h2 stream with arbitrary `:authority` / `Host`. We don't currently check that the request's host matches the connection's pinned endpoint — an attacker who validly negotiates TLS can then spam streams targeting unrelated hostnames.

The CPU profiler did NOT show this as a hotspot (h2 framing in hyper is SIMD-cheap), so this is *defence-in-depth*, not a CPU-fix.

**Fix.** In `handle_requests`, before calling `proxy_pass.send_payload(...)`:

1. Extract host from `req.headers().get("host")` (fallback: `req.uri().host()`).
2. If it doesn't match `proxy_pass.endpoint_info.host_endpoint` (use the existing `is_my_endpoint` helper), return a `421 Misdirected Request` response with a body explaining the mismatch.

We can't force the TCP connection closed from inside a hyper `service_fn` (no access to the `Connection` handle), so the cap there is "reject every misdirected stream" rather than "drop the whole connection". That's still useful — well-behaved h2 clients (browsers, hyper-based clients) re-coalesce on a fresh socket after 421.

---

## 4. Pre-existing bug: error templates always emit `HTTP/1.1 200 OK` (LOW priority — cosmetic)

**Where:** `src/error_templates/generate_layout.rs`, `build_layout`.

`headers.push_response_first_line(200)` is hard-coded. The `status_code` parameter is interpolated into the HTML body but never used in the response status line. Clients that look at status code see `200 OK` even on `Server Error` pages.

**Fix.** Add a `Http1HeadersBuilder::push_response_first_line(status_code)` overload that accepts the status code, then thread the parameter through.

---

## 5. h2 cleanup metric — confirm it stays at zero (LOW priority — observability)

`tokio_tasks_spawned{spawn_name="tcp_gateway_connection_cleanup"}` and friends spawned only from `Drop` paths should normally show 0 / very small numbers. If after a few weeks they grow monotonically — that's a separate leak signal worth investigating.

Capture this as a Grafana alert: `tokio_tasks_spawned > 50` for any non-loop spawn name (loops are expected to scale with workload — cleanup spawns aren't).

---

## 6. H1 byte-pipe rewrite — follow-ups (close after prod verification)

The H1 byte-pipe path was rewritten into a task-decomposed pipeline (`src/h1_proxy_server/pipeline/`: client-reader + per-request worker + client-writer, glued by mpsc; per-connection or global `H1PoolHolder`). The old path (`server_loop`, `response_read_loop`, `h1_server_write_part`, `h1_current_request`, `reconnect_policy`, old `Upstream::connect`/`UpstreamState`) is DELETED. cargo build clean, 34 tests pass. **First step: deploy + verify in prod, then close the holes below.** Rollback is git-revert (old path no longer in tree).

Feature parity with the old byte path is complete (http1/mcp/unix+http1 upstreams direct/ssh/gateway, static, files, drop, dynamic, WS upgrade, header modify, auth, hsts, keep_alive). Remaining holes — none were a regression; they were hyper-only / `todo!()` / dead before too:

- **`compress: true` on h1.** Currently applied ONLY in the hyper path (`http_proxy_pass/http_request_builder.rs`, http2/https2) + gateway. The byte path never compressed. Port the compress logic into the worker (compress the proxied body, esp. for ssh upstreams) if we want it on h1.
- **`trace_payload` config field.** Dead everywhere (`never read`) — never implemented in any path. Decide what it should do (e.g. write request/response payloads to `proxy_logs` per location) and wire it.
- **http2 / unix+http2 UPSTREAM from a byte-path endpoint.** `connect_owned` → `todo!("Http2 temporary is disabled")` / `todo!("Not Implemented")` (same as old). h2 upstreams normally go via the hyper path; only needed if an http1 listen endpoint must point at an h2 upstream.
- **unix+http1 over ssh/gateway.** `connect_owned` → `todo!("UnixHttp1 over gateway/ssh not implemented")` (same as old).
- **WS tunnel: cross-cancel the two `copy_streams` pumps.** On a half-close, the surviving direction lingers up to `read_timeout` (3 min) before tearing down (same as the old WS path — not a regression). Couple the two pumps (select on both JoinHandles + abort the loser, or a shared CancellationToken).
- **Detached worker cleanup on connection close.** Workers are `spawn_named` fire-and-forget; when the reader hits `Close`/`Upgrade`, in-flight workers self-terminate only up to `read_timeout`. Track them in a `JoinSet` / per-connection `CancellationToken` and abort on connection end (slow cleanup, not a leak).
- **Dead-code cleanup (5 warnings):** `trace_payload` field; `error_templates::ENDPOINT_CAN_NOT_BE_UPGRADED_TO_WEB_SOCKET`; `inner.rs` `flush_it`/`write_to_socket`/`shutdown_socket` (orphaned by removing Upstream's `NetworkStreamWritePart` impl); `H1PoolHolder::new_global` (kept for the future global pool).

**Replay semantics (by design, confirm in prod):** a WRITE failure to the upstream (head or body — request did not get through) is retried on a fresh connection (head+body buffered, `MAX_DELIVERY_ATTEMPTS=2`). A READ failure (response timeout/disconnect/garbage — may have been processed) is NEVER replayed: socket marked dead + classified 5xx (timeout→ERROR_TIMEOUT, disconnect→REMOTE_RESOURCE_IS_NOT_AVAILABLE, unparseable→UPSTREAM_IS_NOT_HTTP); next request's `acquire` reconnects. MCP upstreams are never pooled (fresh per request).
