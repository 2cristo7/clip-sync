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
