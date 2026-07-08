# Upstream invisibility — roadmap

**Goal:** upstream connection churn (a dead connection, an upstream restart) is
invisible to endpoint clients; a genuine full outage fails **fast** (bounded
503), never as a timeout cascade.

## Invisibility contract (agreed with the user)

Two scenarios define "invisible":

1. **Upstream is genuinely down** → invisible is impossible; the client gets an
   honest 50x — but FAST (canary dial verdict + budget/cooldown fail-fast, one
   dial per window), never a timeout cascade or a connect storm.
2. **Upstream healthy, a connection randomly broke** → the liveness probe (or
   h2 keep-alive PING after Phase C) finds it and it is quietly recreated; at
   worst the request that discovers it triggers the recreate and still
   succeeds — the price is that ONE request is statistically slower, not an
   error.

### Known residual NON-invisible case (accepted for this release)

A **non-idempotent request (POST/PATCH) whose connection dies MID-FLIGHT** —
the request was already sent, the reply was lost. It cannot be silently
replayed (double-execution risk); the client gets one 502. This is a
fundamental ambiguity, not a defect. Everything else in scenario 2 is
invisible: a break discovered *before* send (`Disconnected`) is retried
silently for ANY method incl. POST; idempotent methods also fail over
mid-flight.

Shrunk by two planned items:
- **Phase C keep-alive PING** — broken connections get detected while idle,
  so a request rarely discovers the break itself (narrows the window to
  breaks happening exactly during the request);
- **M5 REFUSED_STREAM** (optional, Phase A) — h2 servers signal "stream not
  processed" on graceful restart (RFC 9113 §8.7); that subset of POSTs can be
  replayed safely → rolling restarts become invisible for POST too.

**Release decision (Jul 2026): ship the current state; revisit the residual
case by observed 502 rates in production.**

Two repos are involved:
- **proxy** — `my-reverse-proxy` (this repo). I edit it directly, incl. its
  `vendor/my-http-client` copy.
- **client** — standalone `my-http-client` (`~/RustProjects/my-jet-tools/my-http-client`).
  Changed **only via a prompt** the user runs in that project's own session.

---

## Status snapshot

| # | Work | Where | Status |
|---|------|-------|--------|
| 1+4 | Pick-live routing + bounded all-dead recovery (cooldown, wait-budget, re-scan) | proxy `upstream_h2_pool` | ✅ done, reviewed |
| 2 | Idempotent failover retry + dedup 3 content sources into `execute_pooled_h2` | proxy `content_source` | ✅ done, reviewed |
| 5 | Background pool top-up in the supervisor | proxy `upstream_h2_pool` | ✅ done, reviewed |
| — | Bug: h1+h2 supervisor gauge not guarded by `shutdown` (drained pool resurrected its gauge) | proxy | ✅ fixed |
| 7 | Docs sync (`h2-pool.md` rewrite, `pool-lifecycle.md`, `h1-pool.md`, plan banner) | proxy `docs` | ✅ done, verified |
| R6 | `do_request(&req)` — request by reference; failover spare-clone removed | proxy + vendor | ✅ done (proxy/vendor); **standalone pending** |
| 3a | Client hardening: method-aware replay, bounded `is_canceled` loop, timeout≠disconnect, keep-alive PING, `is_alive` | client (standalone) | ✅ done by user; **verified — 2 majors found** |
| **A** | Client round 2: M1 (timeout rounds), M2 (connect_lock + full-dial timeout), R6 (`&req`), zombie fix (`read_loop_stopped` call), `set_read_from_stream_timeout` | client (standalone) | ✅ **done by user** — remote tag `0.1.2` → `8adc82b` (recreated) |
| **B** | **DE-VENDOR** (superseded vendor sync): proxy on git dep `tag = "0.1.2"`, `vendor/` deleted, Cargo.lock pins `8adc82b`. Library hook removed (`TaskMetricsHook` impl + `set_task_metrics_hook`); the proxy's own `spawn_named` + `tokio_tasks_spawned` gauge KEPT (39 named spawn sites) | proxy | ✅ done |
| **C** | Keep-alive integration: consts (10s/2s) + `PoolParams` fields + `set_keep_alive` in `connect_one` + `!is_alive()` → dead in pick-live AND supervisor tick (transport-level detection works even without `health_check_path`) | proxy | ✅ done |
| **D** | Docs: keep-alive/is_alive in `h2-pool.md`; stale replay-caveat dropped from `execute_pooled_h2` | proxy `docs` | ✅ caveat dropped; h2-pool.md updated |
| F1 | h1 idle liveness wired (rule R2): global path → h1 factories; ping rents (`try_rent`) via panic/cancel-safe `RentGuard`; sends `Host: authority` (new h1 `PoolDesc.authority`); drains the response body (dropped body kills the h1 read loop); invalid paths skipped (builder panics on them). h2 also skips invalid paths (would dead-churn). Reviewed (2 lenses), all findings fixed | proxy | ✅ done |
| F5b | `publish_alive_gauge` (h1+h2): set + post-set `shutdown` re-check-and-reset — closes the drain-vs-tick TOCTOU on the gauge; used by tick, revive task, top-up | proxy | ✅ done |
| — | Per-location liveness path / opt-out: all h1 pools now inherit the global `default_h2_livness_url`; an h1 upstream that 404s that path will churn (mark-dead + revive per tick). Per-location override is the fix | proxy config | ⏸ backlog (was already listed for h2) |
| F4 | ~~h2 rule-R5 analog~~ — **resolved by design** (user): "busy" is an h1-only concept (busy until response); h2 multiplexes, so R5 does not apply to h2. Physical note: the server still caps streams (`MAX_CONCURRENT_STREAMS` ≈100–200/conn); past 5×cap requests queue inside hyper until `request_timeout`. Acceptable; optional future observability: per-entry in-flight gauge | — | ✅ closed (by design) |
| F5 | Status-code mapping: `MyHttpClientError(RequestTimeout)` → **504** "Upstream timeout", other `MyHttpClientError` → **502** "Bad gateway" (was 500 catch-all); acquisition failures stay 503 | proxy `utils.rs` | ✅ done |
| F6 | h1 `spawn_revive`: `revive_pending` CAS + panic-safe `RevivePendingGuard` (no cooldown **by design** — h1 model requires foreground Path B to always dial) | proxy `upstream_h1_pool` | ✅ done |
| 6 | Hedged idempotent GETs / connection max-age rotation | proxy | ⏸ deferred |
| — | Safe non-idempotent failover at proxy layer (needs REFUSED_STREAM signal from client) | client + proxy | ⏸ deferred (see M5) |
| — | h1 failover retry — **BLOCKED on M7** (h1 client replays non-idempotent already-sent requests); do not add pool-side h1 failover until the h1 client gets 3a-style method-aware replay | client then proxy | ⏸ blocked |

### Pool model (user spec) and conformance — verdict

The intended model: **R1** pool of reused connections; **R2** liveness-probe only
for idle connections; **R3** dead (via liveness or a failed real request) → mark →
**recreated at the next pick** (the picking request pays the dial); **R4** client
gets 50x **only** when a create/recreate attempt failed → verdict "upstream dead";
**R5** all connections busy at pick → one-shot connection, never pooled.

- **h1 conforms to R1/R3/R4/R5 almost verbatim** (rent-first-free-then-revive;
  503 only from failed dials; Phase 2 disposables). Earlier G1/G2 "gaps"
  (foreground-revive block; 503-on-failed-recreate) are **retracted — they are
  the model, by design**. h1 deviations: R2 not wired (F1); overflow budget caps
  (100 global / 50 per-pool) + deadline give a 503 *without* a dial — a deliberate
  fd-protection extension of the model.
- **h2 deviates from R3/R4 deliberately** (approved invisibility work): dead
  entries are skipped (pick-live) and recreated in background; a request never
  pays the dial while live capacity exists; at all-dead one canary pays the dial
  and waiters are bounded by budget/cooldown (a waiter can get 503 with the
  verdict inherited from a ≤500ms-old failed dial, or — the one true R4 gap —
  while a slow dial is still in flight). **R5 is h1-only by design** (user):
  "busy until the response arrives" exists only in single-stream h1; h2
  multiplexes, so the all-busy category doesn't apply (F4 closed).
- h1 is **ordering-safe**: exclusive rent (CAS) = one request per connection, and
  a timed-out connection is dead-marked → recreated, never reused with a pending
  response. Any future h1 change must preserve exclusive-rent; M7 gates failover.

---

## Ordered roadmap (with dependencies)

```
A (client round 2)  ──►  B (vendor sync)  ──►  C (keep-alive integration)  ──►  D (docs + verify)
     standalone              proxy/vendor            proxy                          proxy
```

**A must come before B.** B (vendor sync) overwrites the vendored client from the
standalone, so R6 + M1 + M2 must be in the standalone first, or B reverts them.

### Phase A — client round 2 (standalone `my-http-client`, via prompt)

Fix the two majors the verification found, plus land R6 so the sync preserves it.
Optional extras listed in "Open client issues" below. **Prompt is in the section
"Phase A prompt" at the bottom** — hand it to the my-http-client session.

Blockers resolved by A: M1 (concurrent-timeout false teardown), M2 (connect-stage
indefinite hang), R6-standalone.

### Phase B — DE-VENDOR (supersedes vendor sync; decided Jul 2026)

Instead of syncing the vendor copy, the vendor is **removed**: all vendor-only
deltas get merged INTO the standalone repo (Phase A prompt below covers the
merge + M1 + M2 + R6 in one visit), the standalone gets a version bump to
`0.2.0` + git tag (semver-breaking: `do_request(&req)`), and the proxy switches
to the MyJetTools-convention git dep:

```toml
my-http-client = { tag = "0.2.0", git = "https://github.com/MyJetTools/my-http-client.git" }
```

then `vendor/` is deleted. The old vendor-sync recipe below is kept only as the
authoritative list of vendor-only deltas that MUST survive the merge.

### Phase C — keep-alive integration (proxy, Opus)

Once the hardened client is vendored, wire up the opt-in keep-alive + liveness:

1. Consts in `src/consts.rs`:
   - `DEFAULT_H2_KEEP_ALIVE_INTERVAL = 10s`
   - `DEFAULT_H2_KEEP_ALIVE_TIMEOUT = 2s`
2. `H2Pool::connect_one` — call `client.set_keep_alive(interval, timeout)` next to
   `set_connect_timeout` (pull the two values from `PoolParams`; add fields with
   the consts as defaults, mirroring the other tunables).
3. Pick-live (`get_connection` Path A loop) — treat `!entry.client.load().is_alive()`
   as dead: `entry.dead.store(true)` + `spawn_revive(entry)` + continue scanning.
   A keep-alive-PING-detected dead socket then heals **before** user traffic
   touches it.
4. `cargo check`.

### Phase D — docs + verify (proxy, Opus)

- `h2-pool.md`: add the keep-alive PING + `is_alive`-in-pick-live behavior to the
  "Design goal: invisibility" and Layer-2/3 sections; move item 3 from "future"
  to shipped.
- Drop the "(Caveat until the my-http-client hardening lands…)" note in
  `h2_dispatch.rs` `execute_pooled_h2`.
- Run the doc-vs-code verification pass.

---

## Phase B — vendor-sync recipe (verified gotchas — DO NOT naive-copy)

A plain `cp -r src` over `vendor/my-http-client/src` breaks the proxy in **4
places**. Preserve these vendor-only deltas after copying:

- **`mod task_metrics` (`lib.rs`)** — vendor-only `TaskMetricsHook`,
  `set_task_metrics_hook`, `spawn_named`. Proxy uses them (`app_ctx.rs`
  `set_task_metrics_hook`; `prometheus.rs` `impl TaskMetricsHook`). Keep the module
  **and** the matching `Cargo.toml` deps (vendor uses `tokio-stream` +
  `futures-util`, not `futures = "*"`).
- **`MyHttpClient::set_read_from_stream_timeout` (h1 `my_http_client.rs`)** —
  vendor-only setter the proxy calls (`h1_pool.rs:212`). Standalone has the field,
  no setter. Re-apply after sync (or upstream it first).
- **h1 read-loop supervisor + `inner.read_loop_stopped(id).await` call** —
  standalone defines `read_loop_stopped` but has **zero call sites**; the vendor
  calls it on clean read-loop exit (+ `catch_unwind` supervision) to avoid zombie
  h1 connections. Re-apply the supervisor block.
- **`tokio::spawn` → `crate::spawn_named` at every spawn point**, merged with the
  new `Handle::try_current()` guard:
  - conn-driver `wrap_http2_endpoint.rs` → `"myhttp_h2_hyper_conn_driver"`;
    `wrap_http1_endpoint.rs` → `"myhttp_h1_hyper_conn_driver"`.
  - Drop impls: `if Handle::try_current().is_ok() { crate::spawn_named("myhttp_h2_client_drop_dispose", async move { inner.dispose().await }) }`
    (h1 variant `"myhttp_h1_client_drop_dispose"`).
  - h1 inner disconnect tasks: `"myhttp_h1_disconnect"` / `"myhttp_h1_websocket_disconnect"`.
  - No NEW task names — the hardening added zero new spawned tasks (keep-alive
    runs inside the existing conn driver; `register_request_timeout` is inline).

---

## Open client issues (from verification of 3a)

Severity as found. **M1, M2** are the two that matter for invisibility → Phase A.
The rest are optional / deferred.

| id | sev | file | issue |
|----|-----|------|-------|
| M1 | major | h2 `my_http2_client_inner.rs:145` | `consecutive_timeouts` counts **concurrent** timeouts → 3 parallel slow requests tear down a healthy shared connection (re-breaks R3 under load) |
| M2 | major | h2 `my_http2_client.rs:292` | `connect_timeout` covers only `connector.connect()`, not the handshake, and `state` lock is held across the whole dial → a wedged upstream hangs **all** `do_request` forever |
| M7 | major | h1 `my_http_client.rs:220` | h1 client replays non-idempotent already-sent requests with **no retry cap** → double-execution + possible infinite reconnect loop (rename `is_retirable`→`is_retryable` only; semantics unchanged) |
| M3 | minor | h2 `my_http2_client.rs:246`, `inner.rs:115` | a single stream-level RST_STREAM tears down the whole shared connection (predates 3a; contradicts R3) |
| M4 | minor | h2 `inner.rs:111` | `consecutive_timeouts` reset not connection-id-gated → stale success delays dead-conn detection (bounded) |
| M5 | minor | h2 `my_http2_client.rs:145` | `REFUSED_STREAM` not distinguished → non-idempotent provably-unsent requests fail instead of safe retry (the deferred "safe POST failover" signal) |
| M6 | minor | h2 `my_http2_client.rs:344` | Drop off-runtime skips `dispose()` → leak; with keep-alive the orphan connection is immortal (not reachable in proxy today) |
| — | minor | h1_hyper `my_http_hyper_client.rs:261` | stale-guard `disconnect()` can tear down a newer connection (no proxy caller today) |

---

## DECISION (Jul 2026): task-count metrics dropped

The `task_metrics`/`spawn_named` machinery (Prometheus gauges counting live
tasks) was hand-debugging tooling — the user decided to drop it entirely:
- **standalone my-http-client**: no task_metrics port at all; plain
  `tokio::spawn` stays; no Cargo dep changes; chunked body reader stays on
  `futures` (its tokio rewrite was only motivated by dropping the futures dep).
- **proxy**: keeps its OWN `crate::app::spawn_named` + `tokio_tasks_spawned`
  gauge for the proxy's tasks (final user decision: named spawns stay wherever
  we can have them). Only the LIBRARY-side hook went away: no
  `impl TaskMetricsHook`, no `set_task_metrics_hook` — the client's ~6 internal
  tasks are simply not counted.

This shrinks the de-vendor merge to: `set_read_from_stream_timeout` setter,
the one-line `read_loop_stopped` zombie fix, R6 `&req`, M1, M2.

## Phase A prompt v2 — CONSOLIDATED (de-vendor merge + M1 + M2 + R6)

The single prompt handed to the my-http-client session (Jul 2026). Supersedes
the v1 prompt below (kept for history). Covers: porting all vendor-only deltas
(task_metrics/spawn_named, set_read_from_stream_timeout, h1 read-loop
supervisor, chunked-body rewrite, Cargo deps), R6 `&req`, and the M1/M2
hardening — in dependency order, ending with a `0.2.0` version bump + tag.
See the chat transcript / the message that delivered it for the full text; the
per-file merge spec it was built from is reproduced in essence by the
"vendor-sync recipe" section above plus the M1/M2 items in the v1 prompt below.

## Phase A prompt v1 (superseded — kept for the M1/M2/R6 wording)

```
Три правки в my-http-client по результатам ревью. established-connection
поведение уже корректно — трогаем только эти пункты.

1) M1 — consecutive_timeouts ложно срабатывает на ПАРАЛЛЕЛЬНЫХ таймаутах.
register_request_timeout (src/http2/my_http2_client_inner.rs) инкрементит один
per-connection счётчик; 3 одновременных медленных запроса на живом соединении
дают counter=3 → соединение рвётся, хотя исправно (медленный роут, не мёртвый
сокет), и рушит остальные мультиплексированные стримы. Нужно рвать только реально
«немой» connection (таймауты И отсутствие успехов), а не конкурентную медленную
нагрузку. Варианты (на выбор):
 (а) держать per-connection last_success timestamp; рвать только если
     now - last_success > window при N таймаутах;
 (б) считать «раунды»: инкремент только если с прошлого инкремента был успех
     ИЛИ прошло > request_timeout без успеха;
 (в) если set_keep_alive задан — не рвать по счётчику вообще (PING поймает
     мёртвый сокет), счётчик оставить фолбэком только для keep_alive=None.
Инвариант: соединение, отдающее хоть какие-то успешные ответы, НИКОГДА не рвётся
из-за одновременных медленных запросов.

2) M2 — connect_timeout не покрывает handshake, и state-lock держится через дозвон.
connect() (src/http2/my_http2_client.rs) оборачивает таймаутом только
connector.connect(), но не wrap_http2_endpoint (builder.handshake + sender.ready()),
и держит self.inner.state.lock() на всю функцию. Зависший апстрим (TCP accept, нет
SETTINGS-преамбулы) → connect() висит вечно с залоченным state → все do_request
виснут на state.lock() до своего request_timeout. Нужно:
 - обернуть таймаутом ВЕСЬ дозвон, включая wrap_http2_endpoint;
 - желательно не держать state.lock() через await дозвона: dial+handshake без
   лока, затем краткий лок только чтобы записать результат (double-check: если
   кто-то уже подключился — выбросить свой). Большой рефактор не обязателен —
   минимум обернуть всё таймаутом, чтобы не висло вечно.
Тот же паттерн есть в http1_hyper connect() — поправить симметрично, если недорого.

3) R6 — сменить сигнатуру MyHttp2Client::do_request (src/http2/my_http2_client.rs):
   req: hyper::Request<Full<Bytes>>  →  req: &hyper::Request<Full<Bytes>>
   и внутри send_payload(req, ...) вместо send_payload(&req, ...). request_is_idempotent
   и остальное работают на &req. Больше req по значению в do_request не нужен.
   (Уже сделано в вендорной копии прокси; без правки в standalone следующий
   вендор-синк откатит R6.)

Проверка: cargo check + cargo build чисто.

ОПЦИОНАЛЬНО (скажи, если брать — не блокирует прокси):
 - M7 (h1): src/http1/my_http_client.rs — is_retryable-ветка реплеит
   не-идемпотентные УЖЕ отправленные запросы без лимита (double-execution +
   возможный бесконечный цикл). Ограничить: не реплеить не-идемпотентные, у
   которых запрос уже ушёл в сокет; добавить retry-cap.
 - M5: отличать REFUSED_STREAM (RFC 9113 8.7 «не обработан») от прочих h2-ошибок
   через err.source() → h2::Error::reason() == REFUSED_STREAM, и разрешать retry
   не-идемпотентных в этом случае (открывает безопасный POST-failover в проксе).
 - M3: одиночный RST_STREAM не должен рвать всё соединение (send_payload:115 и
   do_extended_connect_inner:246 делают disconnect на stream-level ошибке).
```

---

## Reference — original design docs

- Living per-pool design: [h2-pool.md](h2-pool.md), [h1-pool.md](h1-pool.md).
- Lifecycle/GC: [pool-lifecycle.md](pool-lifecycle.md).
- Original (historical) plan: [h2-upstream-pool-plan.md](h2-upstream-pool-plan.md)
  — superseded by this roadmap for the request path.
