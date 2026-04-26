# Gateway Protocol

Internal description of the wire protocol used between `my-reverse-proxy`
peers (gateway server and gateway clients). This document covers the
transport, handshake, frame types, batching/compression and limits.

## Goals

- **Forward secrecy.** Compromise of a long-term key must not decrypt past
  recorded sessions. Achieved via ephemeral X25519 ECDHE; the symmetric AES
  key is derived per connection and exists only in process memory.
- **SSH-style authentication.** The server keeps a list of authorized
  Ed25519 public keys. Each client signs the handshake with its private key.
  No CA, no host key — trust on first use is implicit.
- **No backwards compatibility.** Every gateway peer must run the same
  protocol version. Mismatched peers fail at handshake.

Out of scope: server authentication. Client does not verify server identity.
A network-level MITM that does not have any client's private key cannot read
or inject; an MITM that can mount an active proxy attack is also held back
by the signed transcript (server cannot present a different ephemeral key
to client without knowing a client's private key). Adding a server host key
is a future extension.

## Transport frame

Every byte on the wire after the TCP connection is established belongs to a
sequence of frames:

```
[u32 LEN little-endian] [AES-CBC-encrypt(u8 TYPE || PAYLOAD)]
```

- `LEN` is plaintext, 4 bytes, little-endian. Length of the encrypted body.
- The body, after AES-CBC decryption, is `u8 TYPE` followed by the
  type-specific `PAYLOAD`.
- The AES key/IV (48 bytes total: 32-byte key + 16-byte IV) is the
  per-connection session key derived during handshake. The same key is used
  by both directions for the lifetime of the TCP connection.

There is no other framing. Wire = `(LEN, encrypted(TYPE, PAYLOAD)) × N` with
no separators.

`MAX_PAYLOAD_SIZE = 5 MiB` (the encrypted body). Frames exceeding this
limit are rejected and the connection is closed.

## Handshake

The first two frames on the wire are the connection-level handshake. Their
bodies are **plaintext** (no AES yet — the session key does not exist until
both sides finish this exchange). They use the same `[u32 LEN]`
length prefix as regular frames, so the framing reader is uniform.

### `ClientHandshake` (Client → Server)

| Field            | Size  | Notes                                                          |
|------------------|-------|----------------------------------------------------------------|
| `protocol_version` | 1 B | currently `1`                                                  |
| `client_eph_pub`   | 32 B | X25519 ephemeral public key                                    |
| `client_id_pub`    | 32 B | Ed25519 long-term public key (used to look up authorized list) |
| `timestamp_us`     | 8 B | i64 LE microseconds since epoch (anti-replay)                 |
| `gateway_name_len` | 4 B | u32 LE                                                         |
| `gateway_name`     | var | UTF-8 bytes                                                    |
| `signature`        | 64 B | Ed25519 signature over `client_eph_pub‖timestamp_us‖gateway_name` |

### `ServerHandshake` (Server → Client)

| Field           | Size | Notes                          |
|-----------------|------|--------------------------------|
| `protocol_version` | 1 B | must match client's version |
| `server_eph_pub`   | 32 B | X25519 ephemeral public key |

### Server-side validation

1. Parse `ClientHandshake`. Reject (close connection) if:
   - `protocol_version` is not supported,
   - `client_id_pub` is not in the configured authorized list,
   - `signature` does not verify against `client_id_pub`,
   - `timestamp_us` is more than 60 seconds away from `now()`.
2. Generate a fresh X25519 ephemeral keypair, send `ServerHandshake`.

### Session key derivation

Both sides compute, independently:

```
shared      = X25519(my_eph_priv, their_eph_pub)
key_material = HKDF-SHA256(
    ikm  = shared,
    salt = client_eph_pub || server_eph_pub,
    info = "gateway-session-v1",
    out  = 48 bytes,
)
session_key = AesKey {
    key = key_material[0..32],
    iv  = key_material[32..48],
}
```

The ephemeral private keys are `zeroize`'d immediately after `shared` is
computed. From here on, every frame uses `session_key`.

## Application frame types

After the handshake, all frames carry one of the following `TYPE` codes:

| Code | Name                              | Direction      |
|------|-----------------------------------|----------------|
| `0`  | `PING`                            | both           |
| `1`  | `PONG`                            | both           |
| `3`  | `CONNECT`                         | both           |
| `4`  | `CONNECT_OK`                      | both           |
| `5`  | `CONNECTION_ERROR`                | both           |
| `6`  | `SEND_PAYLOAD` (forward)          | both           |
| `7`  | `RECEIVE_PAYLOAD` (backward)      | both           |
| `8`  | `UPDATE_PING_TIME`                | both           |
| `9`  | `GET_FILE_REQUEST`                | both           |
| `10` | `GET_FILE_RESPONSE`               | both           |
| `11` | `SYNC_SSL_CERTIFICATES`           | server → client |
| `12` | `SYNC_SSL_CERTIFICATES_REQUEST`   | client → server |
| `13` | `SYNC_SSL_CERTIFICATE_NOT_FOUND`  | server → client |
| `20` | `COMPRESSED_BATCH`                | both           |

The legacy `HANDSHAKE` (code `2`) packet has been removed — its job is now
covered by the connection-level handshake described above.

The byte-level layout of each `PAYLOAD` is identical to the previous
revision of the protocol, with the exception of `SEND_PAYLOAD` /
`RECEIVE_PAYLOAD` / `GET_FILE_RESPONSE`, which no longer carry an inner
per-payload gzip flag. Whole-batch zstd via `COMPRESSED_BATCH` replaces it.

## `COMPRESSED_BATCH`

Sender-driven batching with optional whole-batch compression. Active only
when the sender has compression enabled; the receiver always understands
this type.

### `COMPRESSED_BATCH` body

```
[u8 TYPE = 20][u8 ALGO][compressed_bytes]
```

- `ALGO`:
  - `0` — zstd, level 3 (default).
  - `1`, `2` — reserved for future algorithms. Receiving an unknown algo
    closes the connection.
- `compressed_bytes` — output of the algorithm applied to a stream of
  inner frames concatenated together: `(LEN, TYPE, PAYLOAD) × N`, exactly
  as they would appear on the wire **if they were not encrypted**.

### Send-side algorithm

The writer task already owns a `Vec<u8>` accumulator that grows while a
previous `write_all` await is pending. On every flush boundary (the moment
the previous write finishes and the writer is ready to send again):

1. Drain the accumulator. It contains plaintext serialized inner frames
   `[u32 len][u8 type][body]…`.
2. If the connection's `compress` flag is `false`:
   - Encrypt each inner frame individually, prepend its own outer `[u32 LEN]`,
     concatenate, write in one syscall.
3. If `compress` is `true` and the accumulator is non-empty:
   - Run `zstd::compress(buffer, level=3)`.
   - Build outer body: `[TYPE=COMPRESSED_BATCH][ALGO=0][compressed_bytes]`.
   - Encrypt the outer body, prepend `[u32 LEN]`, write in one syscall.

A single `write_all` to the socket is always emitted per flush — either
many independent frames concatenated (no compression) or a single
`COMPRESSED_BATCH` frame.

### Receive-side algorithm

The reader's main loop is unchanged for the outer step:

1. Read `[u32 LEN]`, read body, decrypt.
2. `TYPE = decrypted[0]`.
3. If `TYPE == COMPRESSED_BATCH`:
   - `algo = decrypted[1]`. If unknown, close connection.
   - `decompressed = decompress(decrypted[2..])`.
     Reject and close if `decompressed.len() > MAX_DECOMPRESSED_BATCH_SIZE`.
   - Iterate `[u32 inner_len][u8 inner_type][inner_payload]` over
     `decompressed`, dispatching each through the regular packet handler.
4. Otherwise: dispatch `decrypted` through the regular packet handler.

### Limits

- `MAX_PAYLOAD_SIZE = 5 MiB` — outer frame body.
- `MAX_DECOMPRESSED_BATCH_SIZE = 16 MiB` — defense against decompression
  bombs.

## Configuration shape

YAML — relies on the existing `ssh:` registry for client private keys.

```yaml
ssh:
  gateway-london:
    private_key_file: "~/keys/gateway_id_ed25519"
    passphrase: "secret"

gateway_server:
  port: 5125
  authorized_keys:
    - "~/keys/gateway_london.pub"
    - "~/keys/gateway_amsterdam.pub"

gateway_clients:
  to-main:
    remote_host: "1.2.3.4:5125"
    ssh_credentials: "gateway-london"
    compress: true
```

- The client's private key is loaded by reading `private_key_file` from the
  named `ssh:` entry, decrypted with `passphrase` if present, parsed via
  the `ssh-key` crate. Only Ed25519 keys are accepted.
- The server's `authorized_keys` is a list of paths to standard `*.pub`
  files (one Ed25519 public key per file, OpenSSH text format). Files are
  read at startup; failure to read or parse any of them fails fast.
- `compress: true` on a client enables sender-side `COMPRESSED_BATCH` for
  that client's outbound traffic. Receiver always handles both formats.

## Threat model and limitations

- **Passive observation:** Forward-secret. Recording the entire wire and
  later compromising any long-term key (server-side or client-side) does
  not reveal past traffic.
- **Active MITM without any client private key:** Cannot complete the
  server-side handshake (requires forging a signature). The client side
  may complete a handshake with the MITM but the MITM cannot relay it to
  the real server, so traffic does not flow.
- **Active MITM that controls a client's private key:** Can impersonate
  that client. There is no host pinning to detect this on the client
  side. Not in scope at this revision.
