# H1 Upstream Pool — Design

This document describes the design of the per-endpoint HTTP/1.1 upstream connection pool. One pool per `(scheme, host, port)`; `H1PoolRegistry` keeps them by `PoolKey`. Mirrors the [h2 pool](h2-pool.md) design with one h1-specific addition: each entry carries a `rented` flag because h1 is single-stream — only one in-flight request per connection.

## Goals

- Lazy growth: pool starts empty and fills on demand up to `target_size` (5).
- Single-request-per-connection: h1 has no multiplexing; the `rented` flag enforces exclusive use.
- Overflow: when all pool entries are rented, fall back to one-shot disposable connections up to a global cap (`MAX_DISPOSABLE = 100`).
- Self-healing: dead connections detected by passive `do_request` failures or active liveness pings; revival is asynchronous so user requests don't pay the connect latency.
- WebSocket: each WS session opens its own dedicated TCP — independent of the pool, no counter overhead.

## Data structures

```rust
pub struct H1Pool<TStream, TConnector> {
    clients:   ArcSwap<Vec<Arc<H1Entry<TStream, TConnector>>>>,
    grow_lock: parking_lot::Mutex<()>,    // brief, no await — only for Phase 0 push
    target:    u8,                         // 5 (hardcoded today)
    next:      AtomicUsize,                // round-robin scan start
    factory:   ConnectorFactory<TConnector>,
}

pub struct H1Entry<TStream, TConnector> {
    pub client:       ArcSwap<MyHttpClient<TStream, TConnector>>,  // atomic swap on revival
    pub dead:         AtomicBool,
    pub last_success: AtomicDateTimeAsMicroseconds,                // refreshed on every success
    pub rented:       AtomicBool,                                   // h1-specific: 1 in-flight max
    pub revive_lock:  tokio::sync::Mutex<()>,                       // serializes Path B + revive_task
}

pub enum H1ClientHandle<TStream, TConnector> {
    Reusable   { client: Arc<MyHttpClient>, entry: Arc<H1Entry> },
    Disposable { client: Arc<MyHttpClient> },
    Ws         { client: Arc<MyHttpClient> },
}

// Global, all h1 pools share these:
pub const  MAX_DISPOSABLE:     usize       = 100;
pub static DISPOSABLE_COUNTER: AtomicUsize = AtomicUsize::new(0);
```

- `clients` — the pool list. **Lock-free reads** via `ArcSwap::load()`.
- `grow_lock` — only for serializing Phase 0 pushes. Held briefly (no `await`); the connect happens before acquiring it.
- `revive_lock` (per entry) — `tokio::sync::Mutex<()>` held across the connect `await` during revival. Both foreground (Path B) and background (`revive_task`) lock it; re-check of `dead` after acquire prevents duplicate connects.
- `client` (per entry) — `ArcSwap<MyHttpClient>`, atomically replaced on successful revival.
- `dead`, `last_success`, `rented` — per-entry atomics; lock-free, visible to all readers immediately.
- `DISPOSABLE_COUNTER` — global back-pressure for overflow disposables. Inc on creation, dec on Drop.

## get_connection — three phases

`pool.get_connection().await` returns `Result<H1ClientHandle, MyHttpClientError>`. The whole body is wrapped in a `loop` so the overflow back-pressure can re-evaluate.

```mermaid
flowchart TD
    Start([get_connection]) --> Loop[loop]
    Loop --> Snap["snap = clients.load_full()"]
    Snap --> SizeCheck{snap.len ?}

    SizeCheck -->|< target| P0Connect["factory + connect (no lock)"]
    SizeCheck -->|== target| Phase1[round-robin scan]

    %% Phase 0
    P0Connect --> P0OK{ok ?}
    P0OK -->|err| Err1([return Err])
    P0OK -->|ok| P0Lock["lock grow_lock (sync)"]
    P0Lock --> P0Recheck{cur.len < target ?}
    P0Recheck -->|yes| P0Push["push pre-rented entry, store ArcSwap"]
    P0Recheck -->|no — race lost| P0Disp["DISPOSABLE_COUNTER += 1<br/>return Disposable (one-shot)"]
    P0Push --> Ok0([return Reusable])
    P0Disp --> OkD([return Disposable])

    %% Phase 1 + Phase 2
    Phase1 --> RR["start = next.fetch_add(1) % len"]
    RR --> ScanLoop[/for offset 0..len/]
    ScanLoop --> Pick["entry = snap[(start+offset) % len]"]
    Pick --> TryRent{try_rent ?}
    TryRent -->|false — занят| ScanLoop

    TryRent -->|true — rented by us| DeadCheck{entry.dead ?}
    DeadCheck -->|false| PathA([Path A: return Reusable])
    DeadCheck -->|true — Path B| Revive["revive_entry(entry).await<br/>(per-entry revive_lock)"]
    Revive --> ReviveOK{ok ?}
    ReviveOK -->|yes| PathB([Path B: return Reusable])
    ReviveOK -->|no| Unrent["entry.release_rent()<br/>return Err"]

    ScanLoop -->|loop done, none rented| Phase2[Phase 2 — overflow]
    Phase2 --> CounterInc["cur = DISPOSABLE_COUNTER.fetch_add(1)"]
    CounterInc --> CounterCheck{cur < MAX_DISPOSABLE ?}
    CounterCheck -->|yes| OverflowConnect["factory + connect"]
    CounterCheck -->|no — limit| CounterUndo["DISPOSABLE_COUNTER -= 1"]
    CounterUndo --> Sleep["tokio::sleep(10ms)"]
    Sleep --> Loop
    OverflowConnect --> OvOK{ok ?}
    OvOK -->|yes| OkOv([return Disposable])
    OvOK -->|no| CounterUndoErr["DISPOSABLE_COUNTER -= 1<br/>return Err"]
```

### Phase summary

| Phase | Trigger | Action | Outcome |
|------|---------|--------|---------|
| **0** | `len < target` | Connect; under `grow_lock` push pre-rented (or hand out as Disposable if race lost). | Lazy growth, no overshoot |
| **1A (Path A)** | `len == target`, scan rented an alive entry | Return Reusable | Hot path |
| **1B (Path B)** | `len == target`, scan rented a dead entry | Revive under `revive_lock`, return Reusable. On revive fail: release rent + Err | Foreground recovery |
| **2 (overflow)** | All entries rented | Up to `MAX_DISPOSABLE` Disposables; over limit → 10ms sleep + retry | Back-pressure |

The "race lost" branches (Phase 0, Phase 2 connect fail) keep the counter consistent: any inc has a matching dec on Drop or undo.

## do_request lifecycle

The handle wraps `MyHttpClient::do_request` and updates entry state:

```mermaid
sequenceDiagram
    participant CS as content_source
    participant Handle as H1ClientHandle
    participant Entry as H1Entry
    participant Apstream as Upstream

    CS->>Handle: do_request(req).await
    Handle->>Apstream: client.do_request(req).await
    alt do_request Ok
        Apstream-->>Handle: Response
        opt Reusable variant
            Handle->>Entry: last_success.update(now)
        end
        Handle-->>CS: Ok(MyHttpResponse)
    else do_request Err (timeout/network)
        Apstream--xHandle: Err
        opt Reusable variant
            Handle->>Entry: dead.store(true)
        end
        Handle-->>CS: Err
    end
```

Notes:
- 4xx/5xx HTTP responses are **not** treated as connection errors — the connection is healthy, the request is bad.
- For Disposable / Ws variants, neither `last_success` nor `dead` is touched.
- Drop releases the rent (Reusable), or decrements the counter (Disposable), or no-op (Ws).

## Supervisor tick

Driven by `MyTimer` (panic-safe). Runs every 10s.

```mermaid
flowchart TD
    Tick([tick — every 10s]) --> Snap["snap = clients.load_full()"]
    Snap --> Iter[/per entry in snap/]

    Iter --> EntryCheck{entry.dead ?}
    EntryCheck -->|true| SpawnRevive[["tokio::spawn(revive_task(entry))"]]
    EntryCheck -->|false| AgeCheck{now - last_success < 3s ?}

    AgeCheck -->|yes — hot| Skip([skip])
    AgeCheck -->|no — idle| PathConfigured{health_check_path set ?}
    PathConfigured -->|no| Skip
    PathConfigured -->|yes| Ping["GET health_check_path<br/>(timeout 1s)"]

    Ping --> PingResult{200..=205 ?}
    PingResult -->|yes| MarkSuccess["last_success.update(now)"]
    PingResult -->|no| MarkDead["dead.store(true)"]
    MarkDead --> SpawnRevive2[["tokio::spawn(revive_task(entry))"]]

    SpawnRevive --> NextEntry
    SpawnRevive2 --> NextEntry
    Skip --> NextEntry
    MarkSuccess --> NextEntry
    NextEntry[next entry...]
```

The supervisor never removes anything from the pool itself. Failed revives leave the dead entry in place; the next tick spawns another revive task for it.

### revive_task (tokio::spawn per dead entry)

```mermaid
sequenceDiagram
    participant Tick as Supervisor tick
    participant Task as revive_task (spawned)
    participant Lock as entry.revive_lock
    participant Apstream as Upstream
    participant Entry as H1Entry

    Tick->>Task: spawn(dead_entry_arc)
    Task->>Lock: lock().await
    Note over Lock: serializes vs Path B foreground revive
    Task->>Entry: re-check dead.load()
    alt already revived (!dead)
        Note over Task: race lost — return Ok, do nothing
    else still dead
        Task->>Apstream: factory + connect (timeout 5s)
        alt connect ok
            Apstream-->>Task: TCP/TLS established → MyHttpClient
            Task->>Entry: client.store(new), last_success.update(now), dead=false
            Note over Entry: dead → live in same entry
        else connect fail
            Apstream--xTask: Err
            Note over Task: no-op — dead stays<br/>next tick will spawn another revive
        end
    end
    Task->>Lock: drop guard
```

Concurrency:
- Multiple revive tasks for the same entry are possible (two ticks fired before the first completed). The `revive_lock` + `dead` re-check ensures only one wins; losers drop their fresh client.
- Path B (foreground) and revive_task (background) use the same `revive_lock`, so they don't double-revive.

## create_ws_connection — WebSocket fast path

WS upgrade is detected in content_source via `is_h1_websocket_upgrade(req)`. WS goes through `pool.create_ws_connection().await`, which **bypasses the pool entirely** — it just runs `factory + connect` and returns a fresh `Arc<MyHttpClient>` wrapped in `H1ClientHandle::Ws`.

`create_ws_connection` doesn't touch `clients` and doesn't increment `DISPOSABLE_COUNTER`. The h1 connection lives as long as the WS session, then is dropped. The WS-upgraded TCP stream is extracted into `WebSocketUpgradeStream`; `MyHttpClient::Drop` cleans up when the last Arc dies.

## Concurrency model

| Path | Operation | Synchronization |
|------|-----------|-----------------|
| Hot read (Path A) | Scan + `try_rent` for available entry | `ArcSwap::load()` + `AtomicBool::compare_exchange` — lock-free |
| Round-robin counter | Pick scan start | `AtomicUsize::fetch_add` — lock-free |
| Mark dead | `entry.dead.store(true)` | Atomic — no lock; idempotent |
| Update last_success | `entry.last_success.update(...)` | Atomic — no lock |
| Push (Phase 0) | Append entry under final size check | `grow_lock` (parking_lot) — short critical section, no await |
| Revive (Path B / revive_task) | Replace entry's client under final dead-check | `revive_lock` (tokio::sync::Mutex, per entry) — held across `connect.await` |
| Snapshot for tick | Iterate entries | `ArcSwap::load_full()` — lock-free |
| Disposable counter | Inc/dec | `AtomicUsize::fetch_add/sub` — lock-free |

`grow_lock` is **never held across `await`**. `revive_lock` **is** held across the connect `await` — that's the whole point: it serializes potential duplicate revives.

## Edge cases

### Cold start

```mermaid
sequenceDiagram
    participant C1 as Client req 1
    participant C2 as Client req 2
    participant C3 as Client req N≤target
    participant Pool

    Note over Pool: clients = []
    par
        C1->>Pool: get_connection
        Pool-->>C1: snap.len=0 < target → Phase 0 connect
    and
        C2->>Pool: get_connection
        Pool-->>C2: snap.len=0 < target → Phase 0 connect
    and
        C3->>Pool: get_connection
        Pool-->>C3: snap.len=0 < target → Phase 0 connect
    end
    Note over C1,C3: all paying connect_timeout in parallel
    C1->>Pool: lock + push pre-rented (len < target)
    C2->>Pool: lock + push pre-rented (len < target)
    C3->>Pool: lock + push pre-rented (len < target)
    Note over Pool: clients = [c1, c2, c3] all rented
```

The first `target` parallel requests each pay one `connect`. Subsequent gets after caller drops handles will find rented=false on existing entries via Path A.

### Race overshoot prevention

```mermaid
sequenceDiagram
    participant G1 as Get 1
    participant G2 as Get 2
    participant Pool

    Note over Pool: clients = [a, b, c, d] (len=4, target=5)
    G1->>Pool: snap.len < target
    G2->>Pool: snap.len < target
    par
        G1->>G1: factory + connect → x
    and
        G2->>G2: factory + connect → y
    end
    G1->>Pool: lock(grow_lock)
    G1->>Pool: cur.len=4 < 5 → push x (rented=true) → [a,b,c,d,x]
    G1->>Pool: unlock
    G2->>Pool: lock(grow_lock)
    G2->>Pool: cur.len=5 < 5 ? NO → DISPOSABLE_COUNTER += 1
    G2->>Pool: return Disposable y
    G2->>Pool: unlock
    Note over G2: y returned to caller as one-shot;<br/>after caller's Drop: DISPOSABLE_COUNTER -= 1, TCP closes
```

Pool size after both: `[a,b,c,d,x]` — exactly target. `y` served Get 2's request and went away.

### Overflow back-pressure

```mermaid
sequenceDiagram
    participant Caller
    participant Pool
    participant Counter as DISPOSABLE_COUNTER (global)

    Note over Pool: all 5 entries rented
    Caller->>Pool: get_connection
    Pool->>Pool: scan — try_rent fails for all
    Pool->>Counter: fetch_add(1) → 99
    Note over Counter: 99 < MAX_DISPOSABLE (100)
    Pool->>Caller: Disposable (counter=100)

    Note over Caller: ... another concurrent caller
    Caller->>Pool: get_connection
    Pool->>Pool: scan — try_rent fails for all
    Pool->>Counter: fetch_add(1) → 100
    Note over Counter: 100 >= 100 — at limit
    Pool->>Counter: fetch_sub(1) → 100
    Pool->>Pool: tokio::sleep(10ms).await
    Note over Pool: loop top — re-snap, re-check
    Pool->>Pool: maybe someone released a rent → Path A
```

The retry loop handles transient overload. If the upstream is permanently slow and 100 disposables are stuck, the proxy's `request_timeout` (15s default) bails callers out.

### Upstream went down (single endpoint goes flaky)

```mermaid
sequenceDiagram
    participant CS as caller
    participant Handle
    participant Entry as H1Entry
    participant Tick as Supervisor
    participant Revive as revive_task
    participant Apstream

    CS->>Handle: do_request via Reusable handle
    Handle->>Apstream: client.do_request
    Apstream--xHandle: timeout
    Handle->>Entry: dead.store(true)
    Handle-->>CS: Err
    CS-->>CS: 5xx to client
    Note over Handle,Entry: Drop releases rent (rented=false)

    Note over Tick: 10s later
    Tick->>Tick: snap, iterate
    Tick->>Revive: spawn(revive(entry))
    Revive->>Apstream: factory + connect
    alt apstream still down
        Apstream--xRevive: Err
        Note over Revive: no-op, entry.dead stays
    else apstream recovered
        Apstream-->>Revive: TCP/TLS ok
        Revive->>Entry: client.store(new), last_success=now, dead=false
        Note over Entry: pool entry healthy again
    end
```

In the meantime, foreground gets that round-robin and try_rent past the dead entry hit Path B (also tries to revive — succeeds the moment upstream is back).

### Hot pool — no idle pings

If RPS to an endpoint is high enough that every pool entry sees `last_success` updated within 3s, the supervisor tick **does no pings at all** — every entry is "hot" and skipped. Active probing only kicks in for genuinely idle endpoints, which avoids hammering upstreams that are already known-good.

### WebSocket sessions

WS sessions don't share the pool. Each WS goes through `create_ws_connection` → fresh TCP → returned as `H1ClientHandle::Ws`. The handle's Drop is a no-op; the WS-upgraded TCP stream is extracted into `WebSocketUpgradeStream`. When the WS session closes, the underlying `Arc<MyHttpClient>` drops and TCP closes via `MyHttpClient::Drop`.

WS doesn't count toward `DISPOSABLE_COUNTER` — long-lived WS sessions would otherwise exhaust the back-pressure limit.

## Metrics

Exposed on `/metrics` (Prometheus):

- `h1_pool_size{endpoint="h1://host:port"}` — configured `target` (5).
- `h1_pool_alive{endpoint="..."}` — current `len(clients)` minus `dead` count, set by tick after the pass.

Endpoint label format mirrors the `/configuration` snapshot: `h1://host:port`, `h1s://host:port`, `uds-h1://path`.

`DISPOSABLE_COUNTER` is global; a `h1_disposable_active` gauge is not exposed today (potential add for visibility).

## Hardcoded parameters (today)

- `target_size = 5`
- `MAX_DISPOSABLE = 100` (global, all h1 pools combined)
- `health_check_interval = 10s` (the MyTimer cadence)
- `ping_timeout = 1s`
- `connect_timeout = 5s` (per `PoolParams`, default)
- "Hot threshold" `last_success` window = `3s`
- Overflow retry sleep = `10ms`
- Success status range for ping = `200..=205`

All of these are tracked as tech debt for YAML configuration.

## H1Entry Drop — what happens when its pool is already gone

`H1Entry` doesn't hold a back-reference to `H1Pool`. It has no "find my pool" step in Drop. The default Drop just lets each field clean itself up. So whether the pool still exists or has been drained makes **no difference to H1Entry's own Drop logic** — only to the chain of who decrements which Arc when.

### Setup

Entry is referenced from at most three places:
1. Pool's `clients: ArcSwap<Vec<Arc<H1Entry>>>` — keeps an Arc per slot.
2. Live `H1ClientHandle::Reusable { entry: Arc<H1Entry>, .. }` — one Arc per outstanding rent.
3. Background `revive_task` — captured `Arc<H1Entry>` for the duration of one revival attempt.

`H1Entry` drops when **all three** Arc references are gone.

### Scenario: GcPoolsTimer drains the pool while a request is in-flight

Order of events:

```mermaid
sequenceDiagram
    participant Caller as caller (in-flight do_request)
    participant Handle as H1ClientHandle::Reusable
    participant Entry as Arc<H1Entry>
    participant Pool as Arc<H1Pool>
    participant Registry as H1PoolRegistry
    participant GC as GcPoolsTimer

    Note over Pool,Entry: pool.clients holds Arc<H1Entry>, Vec slot active
    Caller->>Handle: holds handle (Arc<MyHttpClient> + Arc<H1Entry>)
    GC->>Registry: drain_unused — endpoint no longer in config
    Registry->>Registry: lock(write_lock)
    Registry->>Pool: pool.shutdown.store(true)
    Registry->>Registry: build new AHashMap WITHOUT this pool's key
    Registry->>Registry: ArcSwap::store(new map)
    Note over Registry,Pool: Registry's Arc<H1Pool> dropped
    Note over Pool: Refcount of Arc<H1Pool> > 0 only if revive_task still holds it
    Note over Pool: When last Arc<H1Pool> dies → H1Pool drops
    Note over Pool: H1Pool::Drop default: clients (ArcSwap<Vec>) drops → Vec drops →<br/>each Arc<H1Entry> in Vec decremented
    Note over Entry: Entry now referenced ONLY by handle (count=1)
```

At this point the pool is gone from the registry. The `Arc<H1Pool>` itself may already have dropped (if no revive_task held it), which dropped the pool's `Vec`, which decremented each entry's Arc count by 1.

### Then the request completes

```mermaid
sequenceDiagram
    participant Caller
    participant Handle as H1ClientHandle::Reusable
    participant Entry as H1Entry
    participant Client as MyHttpClient
    participant TCP as TCP socket

    Caller->>Handle: do_request returns → handle goes out of scope
    Handle->>Handle: Drop runs
    Handle->>Entry: entry.release_rent() → rented.store(false, Release)
    Note over Entry: harmless — no one will look at this entry again
    Handle->>Handle: drop handle.client (Arc<MyHttpClient>) — refcount -1
    Handle->>Entry: drop handle.entry (Arc<H1Entry>) — refcount -1
    Note over Entry: Arc<H1Entry> count → 0 → Entry drops
    Entry->>Entry: H1Entry default Drop:
    Entry->>Client: drop entry.client (ArcSwap<MyHttpClient>) → inner Arc<MyHttpClient> -1
    Note over Client: refcount → 0 → MyHttpClient::Drop
    Client->>TCP: close TCP / TLS stream (h1 client cleanup)
    Note over Entry: dead, last_success, rented (atomic POD) — no-op drop
    Note over Entry: revive_lock (tokio::sync::Mutex<()>) — drops, no waiters
```

### What does NOT happen

- **No "return to pool" attempt.** `H1ClientHandle::Reusable::drop` does NOT try to look up the pool or re-insert anything. It just calls `entry.release_rent()`, which is one atomic store on a flag inside the entry. There's no `registry.return(...)` call anywhere in the codebase.
- **No `release_rent` failure.** It can't fail — the rent flag is on the entry itself, accessed via the still-alive `Arc<H1Entry>` we hold.
- **No double-close of TCP.** TCP closes exactly once when the last `Arc<MyHttpClient>` dies — which is whenever the last holder (handle, or entry's ArcSwap) drops it.
- **No use-after-free.** Rust's `Arc` guarantees the entry stays alive as long as we hold our reference. Pool dropping its Vec doesn't invalidate our entry; it just decrements the count.

### Edge case: revive_task captured the entry just before drain

Two-step Drop:

1. revive_task's captured `Arc<H1Pool>` ensures the pool is alive while it runs. After drain, the registry no longer has the pool, but revive_task does.
2. revive_task checks `pool.shutdown.load() == true` early and returns. Its captured Arcs (`Arc<H1Pool>`, `Arc<H1Entry>`) drop.
3. Now if no in-flight handle holds the entry either, the entry drops as described above. If a handle still holds it, entry survives until the handle drops.

Same end state, slightly delayed by the time it took the revive_task to notice `shutdown=true`.

### Race with parallel ensure_pool

If, while a request is in-flight on entry from "old" pool, the same endpoint is requested again and a *new* pool is created via `ensure_pool` (because the old pool was drained):

- The new pool has its own fresh `Vec<Arc<H1Entry>>` with brand-new entries.
- The old in-flight request's handle holds the OLD entry, drops it normally, OLD entry's MyHttpClient closes.
- The new pool's entries are independent — they will get connected on their own first `get_connection`.

Two separate "generations" of pool-for-the-same-endpoint can coexist briefly. They don't share state. The OLD generation dies when its last in-flight handle drops; the NEW generation lives on.

## Out of scope

- `http_over_ssh` — still uses the legacy `HttpClientPool` from `src/http_client_pool/`. SSH-tunneled h1 is a different stream type and not migrated.
- See [pool-lifecycle.md](pool-lifecycle.md) for how pools are created on demand and removed by `GcPoolsTimer`.
