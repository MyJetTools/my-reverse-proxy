# Endpoint Protection — Design

This document describes how `my-reverse-proxy` defends listening endpoints from noisy or malicious clients with the **minimum** amount of work per dropped connection. Defenses are layered: a per-IP blocklist short-circuits known-bad sources at accept-time, and silent-drop semantics make the proxy indistinguishable from a closed port for unknown SNI / Host.

## Goals

- Spend zero CPU on TLS handshake / HTTP parsing for IPs that have already misbehaved.
- Reveal nothing about which endpoints are configured: unknown SNI / Host → just close TCP, no TLS Alert, no HTTP response.
- Never block legitimate clients: a successful TLS handshake or successful endpoint resolution clears the IP's history.
- Bound memory: blocklist self-prunes via a periodic GC tick.

## Layers

```
                  TCP accept
                      │
                      ▼
        ┌───────────────────────────┐
        │   ip_blocklist.is_blocked │ ── yes ──► drop tcp_stream, continue
        └───────────────────────────┘
                      │ no
                      ▼
       ┌──────────────────────────────┐
       │  TLS lazy_accept (10s timeout)│
       └──────────────────────────────┘
              │ ok                │ err
              ▼                   ▼
   register_success(ip)    register_failure(ip)
              │                   │
              ▼                   ▼
       endpoint_info        drop tcp_stream
              │
              ▼
   ┌──────────────────────────┐
   │  H1: read_headers + Host │
   └──────────────────────────┘
        │ ok          │ err / unknown Host / garbage
        ▼             ▼
register_success(ip)  register_failure(ip), drop
```

## Per-IP blocklist — `IpBlocklist`

Implementation: [src/app/ip_blocklist.rs](../src/app/ip_blocklist.rs). Backed by `parking_lot::Mutex<AHashMap<IpAddr, IpEntry>>`.

Parameters (constants in the same file):

| Constant | Value | Purpose |
|---|---|---|
| `WINDOW_SECS` | 60 | Sliding window for fail counting |
| `FAIL_THRESHOLD` | 10 | Fails within window that trigger a block |
| `BLOCK_SECS` | 300 (5 min) | Duration of an active block |

`IpEntry` is `~26 bytes`: `fail_count: u16`, `window_start: DateTimeAsMicroseconds`, `blocked_until: Option<DateTimeAsMicroseconds>`.

### API

- `is_blocked(&IpAddr) -> bool` — fast path called from accept-loop.
- `register_failure(IpAddr)` — increments counter; sets `blocked_until = now + 5m` once threshold is hit.
- `register_success(&IpAddr)` — removes the entry entirely. The next failure starts from zero.
- `unblock(&IpAddr) -> bool` — admin override (Swagger endpoint).
- `cleanup() -> usize` — drops expired blocks and stale window entries; returns the count of currently active blocks (used to push the prometheus gauge).

## What counts as a failure

| Source | When | Where |
|---|---|---|
| TLS handshake | `lazy_accept_tcp_stream` errors out (bad ClientHello, no matching SNI, garbage bytes, or 10 s timeout) | [src/tcp_listener/https/handle_connection.rs](../src/tcp_listener/https/handle_connection.rs) |
| H1 unknown Host | `find_endpoint_info` returns nothing for the request's `Host` header → `HttpConfigurationIsNotFound` | [src/h1_proxy_server/server_loop.rs](../src/h1_proxy_server/server_loop.rs) |
| H1 garbage payload | `HeadersParseError`, `ChunkHeaderParseError`, `ParsingPayloadError` from the client | [src/h1_proxy_server/server_loop.rs](../src/h1_proxy_server/server_loop.rs) |

`BufferAllocationFail` is **not** a failure — it's a server-side OOM, not the client's fault.

H2 traffic that survived the TLS handshake is currently not counted at the framing layer; the TLS handshake itself remains protected.

## What clears the counter

| Trigger | Where |
|---|---|
| TLS handshake completes successfully | [src/tcp_listener/https/handle_connection.rs](../src/tcp_listener/https/handle_connection.rs) |
| H1 endpoint info resolves on the first request of a connection | [src/h1_proxy_server/server_loop.rs](../src/h1_proxy_server/server_loop.rs) |
| Admin calls `POST /api/IpBlocklist/Unblock` | [src/http_server/controllers/ip_blocklist/unblock_action.rs](../src/http_server/controllers/ip_blocklist/unblock_action.rs) |

`register_success` deletes the entry from the map, so subsequent activity starts fresh — keep-alive sessions don't keep paying the lookup cost.

## Silent-drop semantics

Unknown SNI or Host **never** triggers a TLS Alert or HTTP response. The TCP socket is just closed. From an attacker's perspective the port is indistinguishable from a closed/firewalled port. This:

- Prevents endpoint enumeration (no `404 Configuration is missing` page).
- Avoids handing the attacker timing/protocol oracles.
- Keeps work per dropped connection minimal: no template render, no socket write.

## Garbage Collection

`IpBlocklistGcTimer` ([src/timers/ip_blocklist_gc_timer.rs](../src/timers/ip_blocklist_gc_timer.rs)) ticks every 60 s as part of the existing `gc_connections_time` timer schedule. Each tick:

1. `cleanup()` drops entries whose block has expired and whose fail-window is older than 60 s.
2. The remaining count of currently-blocked IPs is pushed to the `ip_blocklist_size` Prometheus gauge.

Memory is therefore bounded by `(active blocks + IPs that failed within last 60 s) × 26 bytes`.

## Swagger / Admin endpoints

Both registered in [src/http_server/builder.rs](../src/http_server/builder.rs) under controller name `IpBlocklist`:

- `GET /api/IpBlocklist/Check?ip=1.2.3.4` → `{ ip, blocked }`
- `POST /api/IpBlocklist/Unblock` (form `ip=1.2.3.4`) → `{ ip, removed }`

Use these for verification ("am I banning myself?") and for unblocking after a false positive.

## Metrics

| Metric | Type | Labels | Meaning |
|---|---|---|---|
| `ip_blocklist_size` | gauge | — | IPs with `blocked_until > now` at last GC tick |
| `domain_rps` | gauge | `domain` | RPS by Host. Only counted **after** endpoint resolution succeeded — unknown Host headers do not pollute the label set. |
| `http1_server_connections` / `http2_server_connections` | gauge | `endpoint` | Currently-open inbound connections **after** they passed routing. |

`tcp_pending_handshake` (TCP coverage between accept and routing) is not tracked yet — see [TODO.md](TODO.md).

## What this does NOT cover

- **Distributed botnets** with many source IPs each below threshold. Defense at this scale belongs to a CDN / fingerprinting layer.
- **SYN flood** — kernel/iptables territory.
- **Volumetric DDoS** — provider/CDN territory.
- **Slowloris-style** held-open TCP sessions: bounded by `RESOLVE_TLS_TIMEOUT = 10 s` for TLS and the H1 read timeout for plain HTTP, but there is no global concurrent-connection cap (limited only by `ulimit -n` and RAM).

The whole point is to cheaply absorb **single-source noise** so the rest of the proxy doesn't have to.

## Defaults & tuning

The thresholds (10 fails / 60 s, 5 min block) are compiled-in constants. Move them to runtime config if a deployment needs to tune them per environment. The map and timestamps already support hot reconfiguration; no schema migration needed.
