# Cross-Platform Interop Test Matrix

This document defines the test matrix for ClipSync server/client interoperability
across supported platforms.

## Platform Matrix

| Server \ Client | macOS (arm64) | macOS (x86_64) | Linux (x86_64) | Linux (aarch64) | Windows (x86_64) |
|-----------------|:---:|:---:|:---:|:---:|:---:|
| macOS (arm64)   | A   | A   | A   | M   | M   |
| macOS (x86_64)  | A   | A   | A   | M   | M   |
| Linux (x86_64)  | A   | A   | A   | M   | M   |
| Linux (aarch64) | M   | M   | M   | M   | M   |
| Windows (x86_64)| M   | M   | M   | M   | M   |

**Legend:** A = Automated (CI), M = Manual

## Automated Tests (CI)

The CI matrix (`rust-ci.yml`) covers macOS, Linux (x86_64), and can cross-validate
server/client on the same runner. True cross-platform pairing (e.g., Linux server +
macOS client) requires two machines on the same LAN or Tailscale network.

### What CI validates automatically

1. **Protocol conformance** (all platforms)
   - Golden file deserialization (ClipPayload text, image, pair_response, health)
   - HMAC sign/verify round-trip against golden vector
   - Wire format field completeness (6 fields, correct types)
   - Cross-platform compat vectors (`rust/tests/compat/`):
     - `payload_v1.json` / `payload_v1_ms.json` — ClipPayload v1 with ms `ts`
     - `pairing_error_invalid.json` — `/pair` 401 body shape (`{"error":"invalid"}`)
     - `inject_400_decode.json` — `/inject` 400 body shape (`{"error":"decode_error","message":"..."}`)

2. **Server endpoint behaviour** (all platforms)
   - `/health` returns `{"ok":true,"version":"0.1.0","platform":"<os>"}`
   - `/pair` with valid code returns base64 token/sig/secret
   - `/inject` with valid Bearer + HMAC returns `{"ok":true,"nonce":"..."}`
   - `/inject` without auth returns 401
   - `/inject` with expired HMAC returns 401

3. **Edge cases** (all platforms)
   - Oversized payload (>20 MB) rejected
   - Token revocation blocks subsequent requests
   - Clock skew >60 s rejected (past and future)
   - Malformed HMAC header returns 401
   - Invalid JSON body returns error
   - Empty body returns error
   - Concurrent inject requests all succeed

## Manual Test Checklist

Use this checklist when testing a new platform pair. Replace `S` with the server
platform and `C` with the client platform.

### Prerequisites

- [ ] Server binary built for platform S
- [ ] Client binary built for platform C
- [ ] Both machines on same LAN (or Tailscale)
- [ ] No firewall blocking port 7010

### Discovery

- [ ] Server starts and advertises `_clipsync._tcp.` via mDNS
- [ ] Client discovers server within 5 seconds
- [ ] Server TLS fingerprint displayed in tray/log

### Pairing

- [ ] Server generates 6-digit pairing code
- [ ] Client sends GET /pair?code=XXXXXX with X-ClipSync-Device header
- [ ] Client receives token, sig, and secret (all base64, 32 bytes decoded)
- [ ] Token stored as SHA-256 hex on server side
- [ ] Pairing code consumed (second use rejected with 401)

### Text Sync

- [ ] Copy text on S machine, appears on C within 2 seconds
- [ ] Copy text on C machine, appears on S within 2 seconds
- [ ] Unicode text (emoji, CJK, RTL) syncs correctly
- [ ] Empty string handled gracefully (no crash)
- [ ] Very long text (1 MB) syncs without timeout

### Image Sync

- [ ] Copy PNG on S, received as PNG on C
- [ ] Copy TIFF on macOS S, received as PNG on C (conversion)
- [ ] Copy BMP on Windows S, received as PNG on C (conversion)
- [ ] Large image (10 MB) syncs successfully
- [ ] Notification shows image preview or size

### File Sync

- [ ] Copy file reference on S, file saved to ~/Downloads on C
- [ ] Duplicate file name gets conflict suffix (_1, _2, etc.)
- [ ] Notification shows file name

### Network Resilience

- [ ] Disconnect Wi-Fi on C, reconnect: auto-reconnects within 30 s
- [ ] Disconnect Wi-Fi on S, reconnect: clients reconnect
- [ ] Switch from Wi-Fi to Ethernet: sync continues
- [ ] Tailscale-only connection: manual IP pairing works

### Security

- [ ] Expired HMAC (>60 s clock skew) rejected
- [ ] Revoked token rejected on next request
- [ ] Invalid pairing code rejected (401)
- [ ] TLS fingerprint mismatch: client refuses connection

### Tray Integration

- [ ] Server tray icon shows "running" status
- [ ] Client tray icon shows "connected" status
- [ ] Pause/resume from tray works
- [ ] Quit from tray cleanly shuts down

## Test Results Template

```
Date:       YYYY-MM-DD
Tester:     <name>
Server:     <platform> (<arch>)  build: <commit>
Client:     <platform> (<arch>)  build: <commit>
Network:    LAN / Tailscale / Mixed

| Test                        | Pass | Notes |
|-----------------------------|------|-------|
| mDNS discovery              |      |       |
| Pairing                     |      |       |
| Text sync (S→C)             |      |       |
| Text sync (C→S)             |      |       |
| Unicode text                |      |       |
| Image sync                  |      |       |
| File sync                   |      |       |
| Large payload               |      |       |
| Network reconnect           |      |       |
| Expired HMAC rejected       |      |       |
| Revoked token rejected      |      |       |
| TLS fingerprint validation  |      |       |
| Tray status                 |      |       |
```

## Cross-Platform Compat Vectors

Golden vectors live in `rust/tests/compat/` and are byte-equivalent to what the
Mac Swift implementation emits on the wire. The Mac implementation is the
canonical source of truth for the wire format; Rust must parse and round-trip
these vectors unchanged.

### Vector index

| File                                                  | Wire shape                                                                              | Source of truth (Mac/Android)                            |
|-------------------------------------------------------|------------------------------------------------------------------------------------------|----------------------------------------------------------|
| `compat/payload_v1.json`                              | `ClipPayload` v1 with `ts` in **ms** (legacy name; kept for back-compat)                | Mac `ClipPayload.swift` post-fix `f615624e`              |
| `compat/payload_v1_ms.json`                           | Alias of `payload_v1.json` — explicit name introduced in Phase 1.8                       | Mac `ClipPayload.swift` post-fix `f615624e`              |
| `compat/pairing_error_invalid.json`                   | `/pair` 401 body: `{"error":"invalid"}` (no `message`)                                  | Mac pairing route, Phase 1.5 commits `62ad7bc6` / `b3cc3159` |
| `compat/inject_400_decode.json`                       | `/inject` 400 body: `{"error":"decode_error","message":"<text>"}`                       | Mac `ClipServer.swift`, Phase 1.3 commit `4f8ab8de`      |

### Wire-shape contracts asserted

- **`ClipPayload.ts`** — Unix milliseconds (`Int64`), never seconds. See
  CLAUDE.md §"Wire Protocol Invariants".
- **`/pair` 401** — single-key body `{"error":"<code>"}`; `<code>` is one of
  `invalid`, `expired`, `consumed`, `notStarted`. No `message` field.
- **`/inject` 4xx** — two-key body `{"error":"<code>","message":"<text>"}`;
  `<code>` is one of `decode_error`, `unsupported_kind`,
  `timestamp_out_of_range`, `payload_too_large`. The `message` text is
  diagnostic and not part of the wire contract — only the `error` code is.

Each vector ships with an adjacent `.md` sidecar that captures the source
commit and per-vector invariants. Tests live in
`rust/crates/clipsync-core/tests/protocol_compat.rs`.

## Enterprise Backward Compatibility (Phase 2.14)

Integration tests in `rust/apps/enterprise-server/tests/enterprise_compat.rs`
prove that the enterprise server handles legacy and enterprise clients correctly.

### Enterprise Compat Matrix

| Scenario | Client Type | Hello Frame | Default Policy | Push | Receive | Test |
|----------|-------------|:-----------:|:--------------:|:----:|:-------:|------|
| Legacy Android v0.1.1 | Legacy | No | ReadWrite | Yes | Yes | `legacy_android_compat` |
| Legacy Mac Swift | Legacy | No | ReadWrite | Yes | Yes | `legacy_mac_swift_compat` |
| Enterprise Client | Enterprise | Yes (v2) | Per-device | Per-policy | Per-policy | `enterprise_client_full_path` |

### Policy Enforcement Across Reconnects

All 5 policies are tested with a connect-disconnect-reconnect cycle. The
policy is pre-set in the runtime before the first connection and must
survive both connections unchanged.

| Policy | Can Push | Can Receive | Receive From Leader Only | Test |
|--------|:--------:|:-----------:|:------------------------:|------|
| ReadWrite | Yes | Yes | N/A | `policy_enforcement_read_write_across_reconnect` |
| ReadOnly | No | Yes | N/A | `policy_enforcement_read_only_across_reconnect` |
| WriteOnly | Yes | No | N/A | `policy_enforcement_write_only_across_reconnect` |
| Muted | No | No | N/A | `policy_enforcement_muted_across_reconnect` |
| FollowLeader | No | Yes (leader only) | Yes | `policy_enforcement_follow_leader_across_reconnect` |

### Broadcast Multicast

| Scenario | Clients | Payload Size | Verified | Test |
|----------|:-------:|:------------:|:--------:|------|
| Multicast broadcast | 3 receivers + 1 sender | 1 MB | Byte-identical delivery | `broadcast_multicast_identical_bytes` |

### Handshake Protocol Contract

- Legacy clients that send raw `ClipPayload` as first frame are accepted
  with default `ReadWrite` policy. The first payload is not dropped.
- Enterprise clients sending `Hello` with `protocol_version <= 2` receive
  `Welcome` with `server_capabilities`, `your_policy`, and agreed version.
- Enterprise clients with `protocol_version > CURRENT_PROTOCOL_VERSION`
  receive `HandshakeError` with code `unsupported_version` and the
  connection is closed.
