# Plan: per-endpoint h2 upstream pool with N connections and active health-check

## Context

In the high-load scenario (tens of thousands of concurrent WebSocket connections to several upstream services), the current outgoing-side h2 architecture does not scale, for two reasons:

1. **TCP/TLS h2 pool** ([src/http2_client_pool/http2_client_pool_inner.rs](../src/http2_client_pool/http2_client_pool_inner.rs)) keeps **one** `MyHttp2Client` per endpoint string. All incoming traffic goes through a single TCP connection, hits the hyper-default `SETTINGS_MAX_CONCURRENT_STREAMS = 200` limit on the upstream server, and contends on a single `Mutex<state>` inside the client.
2. **UDS h2 pool** ([src/http_clients/http2_clients.rs](../src/http_clients/http2_clients.rs)) is keyed by `connection_id` (per-incoming-connection). Each browser opens its OWN UDS connection to the upstream. This kills h2 multiplexing entirely and produces N×M file descriptors on the upstream side.

**Goal:** replace both with a single model — "named per-endpoint pool of N=5 h2 connections with a background supervisor and active health-check", independent of the request flow. Requests pick a connection via round-robin; the supervisor reconnects dead slots based on a background ping.

**Scope (per user decision):** h2 only (`http2`, `https2`, `unix+http2`). h1 paths stay as is — they have one-to-one "request per connection" semantics and a fundamentally different logic.

**Hardcoded parameters:**
- success status range: `200..=205`
- ping timeout: `1s`
- fail threshold: `3` consecutive failed pings → slot is recreated
- canonicalization of endpoint key: scheme lowercase, host lowercase, port explicit; path/query NOT part of the key

**Per-location config (new YAML fields):**
- `pool_size: u8` — default `5`
- `health_check_path: Option<String>` — if absent, active health-check is **disabled**, reconnect happens reactively only (on actual `send_request` error)
- `health_check_interval_ms: u64` — default `10_000`

## Implementation

### 1. New module `src/upstream_h2_pool/`

```
src/upstream_h2_pool/
  mod.rs
  pool_key.rs           # canonicalize endpoint string into pool key
  h2_slot.rs            # slot: ArcSwapOption<MyHttp2Client> + fail_count: AtomicU8
  h2_pool.rs            # Vec<Arc<H2Slot>> + AtomicUsize round-robin counter
  pool_supervisor.rs    # background task: connect-if-empty, ping, fail counter, kill-recreate
  pool_registry.rs      # HashMap<PoolKey, Arc<H2Pool<...>>>; lazy create + drain unused
```

**`H2Slot`:**
- `client: ArcSwapOption<MyHttp2Client<TStream, TConnector>>`
- `failure_count: AtomicU8`
- `last_health_check: AtomicI64` (timestamp ms)

**`H2Pool<TStream, TConnector>`:**
- `slots: Vec<Arc<H2Slot<TStream, TConnector>>>` — sized by configured `pool_size`
- `next: AtomicUsize` for round-robin
- `health_check_path: Option<String>`, `health_check_interval: Duration`
- `acquire(&self) -> Option<Arc<MyHttp2Client>>` — round-robin, skip empty slots; returns None if all slots are empty → caller returns 503

**`PoolSupervisor`** — spawned when the pool is created, runs an infinite loop:
```
loop {
    for slot in slots {
        if slot.empty() { try_connect_into(slot) }
        else if health_check_path.is_some() {
            ping(slot, timeout=1s)
            if status in 200..=205 { reset fail_count }
            else { fail_count += 1 }
            if fail_count >= 3 { slot.client.store(None) }  // → recreated next tick
        }
    }
    sleep(interval)
}
```

If `health_check_path` is None, the supervisor only fills empty slots when accessed (no active ping).

### 2. Pool key + canonicalization

`PoolKey { scheme: H2Scheme, host: String, port: u16, tls_marker: Option<TlsConfigHash> }`

`H2Scheme = { Http2 | Https2 | UnixHttp2 }` — for the TLS variant, include a hash of the TLS config (ssl_certificate id + client cert) so that the same `https2://x:443` with different credentials maps to a different pool.

### 3. Pool registry in AppContext

In [src/app/app_ctx.rs](../src/app/app_ctx.rs), three new fields (one per stream type — generics don't allow a single HashMap covering all three):
- `h2_tcp_pools: H2PoolRegistry<TcpStream, HttpConnector>`
- `h2_tls_pools: H2PoolRegistry<TlsStream<TcpStream>, HttpTlsConnector>`
- `h2_uds_pools: H2PoolRegistry<UnixStream, UnixSocketHttpConnector>`

To remove:
- `unix_socket_h2_socket_per_connection: Http2Clients<...>` — replaced by `h2_uds_pools`.
- The legacy [src/http_clients/http2_clients.rs](../src/http_clients/http2_clients.rs) is removed entirely.
- [src/http2_client_pool/](../src/http2_client_pool/) — review for removal (its role is taken over by the new `H2PoolRegistry` + `H2Pool`).

### 4. Lazy pool creation at config compile time

In [src/scripts/compile_location_proxy_pass_to.rs](../src/scripts/compile_location_proxy_pass_to.rs), when processing locations of type `Http2`/`Https2`/`UnixHttp2`:
- Extract the endpoint string + pool params from the location
- Call `registry.ensure_pool(pool_key, params)` — lazily creates the pool if missing; returns the existing one otherwise
- On hot-reload: `registry.drain_unused(active_keys)` removes pools that no location references anymore

Conflict resolution (two locations referencing the same endpoint string but with different `pool_size`): first one wins; warning logged. Simple semantics, no startup failure.

### 5. Refactor content sources

[src/http_proxy_pass/content_source/http2.rs](../src/http_proxy_pass/content_source/http2.rs), [https2.rs](../src/http_proxy_pass/content_source/https2.rs), [unix_http2.rs](../src/http_proxy_pass/content_source/unix_http2.rs):

```rust
pub async fn execute(&self, req: ...) -> Result<HttpResponse, ProxyPassError> {
    let pool = APP_CTX.h2_xxx_pools.get(&self.pool_key)
        .ok_or(ProxyPassError::UpstreamUnavailable)?;
    let Some(client) = pool.acquire() else {
        return Err(ProxyPassError::UpstreamUnavailable);  // → 503 to the client
    };
    super::execute_h2(&client, req, self.request_timeout).await
}
```

The `H2Sender` trait already implements support for `Arc<MyHttp2Client>` — reuse it as is. [src/http_proxy_pass/content_source/h2_dispatch.rs](../src/http_proxy_pass/content_source/h2_dispatch.rs) does not change.

### 6. Settings + parsing

In [src/settings/location_settings.rs](../src/settings/location_settings.rs), add to `LocationSettings`:
```rust
pub pool_size: Option<u8>,
pub health_check_path: Option<String>,
pub health_check_interval_ms: Option<u64>,
```

At compile time, propagate them into a `PoolParams` object passed to `registry.ensure_pool`.

### 7. ProxyPassError

In [src/http_proxy_pass/error.rs](../src/http_proxy_pass/error.rs), add a variant `UpstreamUnavailable` (or reuse `Disconnected` — depending on semantics; a new variant is cleaner). Convert to HTTP 503 in [src/tcp_listener/http_request_handler/](../src/tcp_listener/http_request_handler/).

## Files to create / modify

**Create:**
- `src/upstream_h2_pool/mod.rs`
- `src/upstream_h2_pool/pool_key.rs`
- `src/upstream_h2_pool/h2_slot.rs`
- `src/upstream_h2_pool/h2_pool.rs`
- `src/upstream_h2_pool/pool_supervisor.rs`
- `src/upstream_h2_pool/pool_registry.rs`

**Modify:**
- [src/app/app_ctx.rs](../src/app/app_ctx.rs) — replace pools
- [src/http_proxy_pass/content_source/http2.rs](../src/http_proxy_pass/content_source/http2.rs)
- [src/http_proxy_pass/content_source/https2.rs](../src/http_proxy_pass/content_source/https2.rs)
- [src/http_proxy_pass/content_source/unix_http2.rs](../src/http_proxy_pass/content_source/unix_http2.rs)
- [src/settings/location_settings.rs](../src/settings/location_settings.rs) — new fields
- [src/scripts/compile_location_proxy_pass_to.rs](../src/scripts/compile_location_proxy_pass_to.rs) — pass params into the registry
- [src/http_proxy_pass/error.rs](../src/http_proxy_pass/error.rs) — `UpstreamUnavailable` variant

**Delete:**
- `src/http_clients/http2_clients.rs` (per-incoming-connection UDS pool)
- The `unix_socket_h2_socket_per_connection` field in AppContext

## Reused components

- `MyHttp2Client::do_request` / `do_extended_connect` — wrapped as before.
- [src/http_proxy_pass/content_source/h2_dispatch.rs](../src/http_proxy_pass/content_source/h2_dispatch.rs) — `H2Sender` trait + `execute_h2` function stay.
- `arc_swap::ArcSwapOption` — for slot client (lock-free swap on kill/recreate).

## Verification

1. **Build:** `cargo build` clean, no warnings.
2. **Basic functionality:**
   - Run mt-rest-api locally on h2 UDS.
   - Proxy config: one location `unix+http2` with `pool_size: 5`, `health_check_path: /health`, `health_check_interval_ms: 5000`.
   - `lsof -p <proxy-pid>` after startup — exactly 5 UDS fds to the upstream socket.
3. **Liveness:**
   - `kill` upstream → within 3 cycles (≤15s) the supervisor must clear all 5 slots; requests start returning 503.
   - Restart upstream → on the next tick slots recover and requests flow again.
4. **Scale:**
   - 10K concurrent WebSocket sessions to one endpoint → `lsof` still shows 5 UDS connections to upstream (multiplexing works); proxy memory does not grow linearly with the number of WS sessions.
5. **Hot-reload:**
   - Add a second location with the same endpoint string → still 5 connections (one shared pool).
   - Remove all locations referring to that endpoint, wait → pool drains and is removed.
6. **Health-check off:**
   - Location without `health_check_path` → supervisor doesn't ping, but on `send_request` error a slot is reactively recreated on the next acquire.
7. **No regressions:**
   - h1, https1, unix+http1 paths work as before (untouched).
   - Existing h2 → h1 WebSocket path continues to work.
