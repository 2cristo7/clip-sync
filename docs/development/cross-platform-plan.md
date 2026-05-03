# ClipSync Cross-Platform Master Plan

## Objective

Migrate ClipSync to a **single Rust codebase** that produces 6 binaries: server + client for macOS, Linux, and Windows. The existing Mac Swift server (`mac/`) is retired — Rust replaces it completely. All 6 desktop binaries and the existing Android client (Kotlin) are fully interoperable: any client can connect to any server.

**What gets built:**

| Platform | Server | Client |
|----------|--------|--------|
| macOS | `clipsync-server` (Rust, replaces Swift) | `clipsync-client` (Rust, new) |
| Linux | `clipsync-server` (Rust, new) | `clipsync-client` (Rust, new) |
| Windows | `clipsync-server` (Rust, new) | `clipsync-client` (Rust, new) |

**What stays:** Android client (Kotlin) — unchanged, must remain compatible.
**What is retired:** `mac/` Swift server — archived in `mac-legacy/` after Rust server is proven equivalent.

---

## Technology Decision: Rust

A single Rust codebase compiles natively to macOS, Linux, and Windows. No runtime dependencies, no Electron bloat, excellent crypto/TLS/HTTP ecosystem.

**Core crate** (`clipsync-core`): shared protocol, HMAC, TLS, payload serialization — used by both server and client binaries. One `cargo build` per platform produces both binaries.

### Key Rust Dependencies

| Purpose | Crate |
|---------|-------|
| HTTP server | `axum` + `axum-extra` |
| WebSocket server | `axum` (built-in upgrade) |
| WebSocket client | `tokio-tungstenite` |
| TLS | `rustls` + `rcgen` (self-signed cert gen) |
| HMAC-SHA256 | `hmac` + `sha2` |
| mDNS advertise | `mdns-sd` |
| mDNS discover | `mdns-sd` |
| Clipboard (Mac) | `arboard` |
| Clipboard (Linux) | `arboard` (X11/Wayland via `wl-clipboard`) |
| Clipboard (Windows) | `arboard` (Win32 API) |
| System tray | `tray-icon` + `muda` (menu) |
| Keychain/secrets | `keyring` |
| JSON | `serde` + `serde_json` |
| Async runtime | `tokio` |
| Logging | `tracing` + `tracing-subscriber` |
| CLI | `clap` |
| Base64 | `base64` |
| UUID | `uuid` |
| File watcher | `notify` (for clipboard file changes) |

---

## Repository Structure (New)

```
clip-sync/
├── android/                    # Existing Android client (Kotlin) — unchanged
├── mac-legacy/                 # Archived Swift server (moved from mac/ after Rust proven)
├── rust/                       # Cross-platform Rust workspace (server + client)
│   ├── Cargo.toml              # Workspace root
│   ├── crates/
│   │   ├── clipsync-core/      # Shared library
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── protocol.rs     # ClipPayload, endpoints, constants
│   │   │       ├── hmac.rs         # HMAC-SHA256 sign + verify
│   │   │       ├── tls.rs          # Self-signed EC P-256 cert generation
│   │   │       ├── fingerprint.rs  # SPKI-SHA256 base64url
│   │   │       ├── pairing.rs      # 6-digit code gen, token exchange
│   │   │       ├── token_store.rs  # ~/.clipsync/tokens.json
│   │   │       ├── mdns.rs         # Advertise + discover _clipsync._tcp
│   │   │       ├── clipboard.rs    # Platform-abstracted clipboard trait
│   │   │       └── config.rs       # Port, version, constants
│   │   │
│   │   ├── clipsync-server/    # Server binary (Linux/Windows/Mac)
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── main.rs
│   │   │       ├── routes.rs       # /health, /pair, /inject, /ws
│   │   │       ├── auth.rs         # Bearer + HMAC middleware
│   │   │       ├── ws_hub.rs       # WebSocket client manager + broadcast
│   │   │       ├── clipboard_watcher.rs  # Poll clipboard, broadcast changes
│   │   │       ├── clipboard_injector.rs # Write received payloads to clipboard
│   │   │       └── tray.rs         # System tray icon + menu
│   │   │
│   │   └── clipsync-client/    # Client binary (Mac/Linux/Windows)
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── main.rs
│   │           ├── connector.rs    # WebSocket connect + reconnect
│   │           ├── sender.rs       # POST /inject with HMAC
│   │           ├── pairing_flow.rs # Discover → pair → store creds
│   │           ├── clipboard_watcher.rs  # Poll clipboard, send changes
│   │           ├── clipboard_injector.rs # Write received payloads to clipboard
│   │           └── tray.rs         # System tray icon + menu
│   │
│   ├── tests/                  # Integration tests
│   │   ├── protocol_compat.rs  # Test against golden data from original Swift server
│   │   ├── hmac_vectors.rs     # Known HMAC test vectors
│   │   ├── pairing_flow.rs     # End-to-end pairing test
│   │   └── payload_round_trip.rs
│   │
│   └── resources/
│       └── icons/              # Tray icons per platform
│           ├── icon_mac.png
│           ├── icon_linux.png
│           └── icon_windows.ico
│
├── docs/
│   └── development/
│       └── cross-platform-plan.md  # This file
└── ...
```

---

## Protocol Compatibility Contract

The Rust implementation MUST be 100% wire-compatible with the original Swift server protocol and the existing Kotlin client. This is the absolute hard constraint. The Swift server is archived in `mac-legacy/` as the reference implementation.

### Exact Wire Format

```json
{
  "type": "text|image|file",
  "mime": "text/plain|image/png|...",
  "data": "<base64-standard>",
  "ts": 1714000000,
  "nonce": "550e8400-e29b-41d4-a716-446655440000",
  "name": null
}
```

- `ts` is Unix seconds (NOT milliseconds)
- `data` is standard base64 (NOT base64url)
- `name` is null (not absent) when not a file
- Max payload: 20 MB
- JSON field order doesn't matter (both sides use keyed parsing)

### HMAC Signature

```
Header: X-ClipSync-Signature
Format: t=<unix_seconds>, v1=<lowercase_hex>
Sign:   HMAC-SHA256(secret_bytes, "<ts>.<json_body_bytes>")
Skew:   ±60 seconds
```

### TLS Certificate

- Algorithm: EC P-256 (secp256r1)
- Self-signed, 365-day validity
- Fingerprint: SHA-256 of SubjectPublicKeyInfo (DER), encoded as base64url WITHOUT padding
- SAN: localhost, hostname, *.local, primary IPv4, 127.0.0.1

### mDNS

- Service type: `_clipsync._tcp.`
- Port: 7010
- TXT: `version=0.1.0`, `name=<hostname>`, `fp=<spki-sha256-base64url-nopad>`

### Pairing

- Code: 6 random digits, TTL 120 seconds
- Request: `GET /pair?code=XXXXXX` with header `X-ClipSync-Device: <label>`
- Response: `{"token":"<b64-32bytes>","sig":"<b64-hmac>","secret":"<b64-32bytes>"}`
- Token stored hashed (SHA-256 hex), not plaintext

### HTTP Endpoints

| Path | Method | Auth | Body | Response |
|------|--------|------|------|----------|
| `/health` | GET | None | — | `{"ok":true,"version":"0.1.0","platform":"<os>"}` |
| `/pair` | GET | None | query `code` | `{"token":"...","sig":"...","secret":"..."}` |
| `/inject` | POST | Bearer+HMAC | ClipPayload JSON | `{"ok":true,"nonce":"..."}` |
| `/ws` | GET→Upgrade | Bearer | — | WebSocket (JSON frames) |

### WebSocket

- URL: `wss://<host>:7010/ws`
- Auth: `Authorization: Bearer <token>` header on upgrade
- Frames: Text frames containing JSON-encoded ClipPayload
- Close: Code 1000 for graceful shutdown

---

## Execution Phases

### Phase 0: Archive Swift Server & Extract Golden Tests
**Estimated effort: ~30 min**
**Branch: `chore/archive-swift` from `dev`**

#### 0.1 — Create `dev` branch and extract protocol golden data
- `git checkout -b dev` from `main`
- `git checkout -b chore/archive-swift` from `dev`
- Run the existing Swift server locally and capture golden test data:
  - `curl -k https://localhost:7010/health` → save response JSON
  - Capture a known HMAC signature (hardcode a secret + body + timestamp → expected hex)
  - Capture a ClipPayload JSON sample from the WebSocket
  - Capture the TLS certificate fingerprint format
- Save golden data to `rust/tests/golden/` as `.json` files

**Commits:**
- `chore[dev]: create dev branch from main`

#### 0.2 — Archive Swift server
- `git mv mac/ mac-legacy/`
- Update `CLAUDE.md` to note `mac-legacy/` is archived and Rust is the active server
- Update `README.md` to reflect the migration

**Commits:**
- `chore[archive]: move mac/ to mac-legacy/ — Rust server replaces Swift`
- `docs[readme]: update for Rust migration`

#### 0.3 — Merge to dev
- `git checkout dev && git merge --no-ff chore/archive-swift`

**Success criteria Phase 0:**
- `mac-legacy/` contains the full Swift source (unchanged, just moved)
- Golden test data exists in `rust/tests/golden/`
- `dev` branch is clean with archive merged

---

### Phase 1: Core Library (`clipsync-core`)
**Estimated effort: ~2000 lines of Rust**
**Branch: `feature/rust-core` from `dev`**

#### 1.1 — Workspace + Core Skeleton
- Create `rust/Cargo.toml` workspace
- Create `rust/crates/clipsync-core/Cargo.toml` with all dependencies
- Create `src/lib.rs` with module declarations
- Create `src/config.rs` with constants (PORT=7010, VERSION="0.1.0", MAX_PAYLOAD=20MB, etc.)

**Commits:**
- `feat[rust]: initialize Cargo workspace and clipsync-core crate`

#### 1.2 — Protocol Types
- `src/protocol.rs`:
  - `ClipType` enum: Text, Image, File (serialize as lowercase strings)
  - `ClipPayload` struct with serde: type, mime, data (base64 string), ts (i64), nonce (String), name (Option<String>)
  - Custom serialize/deserialize to match exact wire format
  - `ClipPayload::digest()` → SHA-256 of content (for echo detection)
  - Unit tests: serialize/deserialize round-trip, test against golden JSON files

**Commits:**
- `feat[core]: implement ClipPayload wire format with serde`

#### 1.3 — HMAC Module
- `src/hmac.rs`:
  - `sign(secret: &[u8], timestamp: i64, body: &[u8]) -> String` → returns `t=..., v1=...`
  - `verify(secret: &[u8], header: &str, body: &[u8], max_skew: i64) -> Result<()>`
  - Parse header format: extract `t` and `v1` fields
  - Constant-time comparison for signature
  - Unit tests with known vectors (from golden test data or hardcoded)

**Commits:**
- `feat[core]: implement HMAC-SHA256 signing and verification`

#### 1.4 — TLS Module
- `src/tls.rs`:
  - `generate_self_signed(hostnames: Vec<String>, ips: Vec<IpAddr>) -> (CertificateDer, PrivateKey)`
  - EC P-256 via `rcgen`
  - Persist cert + key to `~/.clipsync/cert.der` and `~/.clipsync/key.pem`
  - Load existing if present and not expired
  - `rustls::ServerConfig` builder with the generated cert
  - `rustls::ClientConfig` builder with custom verifier (TOFU + fingerprint pinning)
- `src/fingerprint.rs`:
  - `spki_sha256(cert_der: &[u8]) -> String` → base64url without padding
  - Parse X.509 DER to extract SubjectPublicKeyInfo
  - Unit tests: known cert → known fingerprint

**Commits:**
- `feat[core]: implement self-signed TLS cert generation with EC P-256`
- `feat[core]: implement SPKI-SHA256 fingerprint computation`

#### 1.5 — Pairing Logic
- `src/pairing.rs`:
  - `PairingCode`: 6-digit random code with `created_at` and TTL (120s)
  - `generate_code() -> PairingCode`
  - `create_token(secret: &[u8]) -> (token_b64, sig_b64, secret_b64)`: generate 32 random bytes for token, HMAC(token, secret) for sig, 32 random bytes for shared secret
  - `validate_code(code: &str, active: &PairingCode) -> bool`: check match + TTL
- `src/token_store.rs`:
  - `TokenRecord`: id, token_hash_hex, device_label, created_at, last_seen_at
  - `TokenStore`: load/save `~/.clipsync/tokens.json`
  - `register(token: &[u8], label: &str)`
  - `validate(token: &[u8]) -> Option<&TokenRecord>`
  - `touch(token: &[u8])`: update last_seen_at
  - `revoke(id: &str)`
  - `list() -> Vec<TokenRecord>`
  - Tokens are stored as SHA-256 hex (never plaintext)

**Commits:**
- `feat[core]: implement pairing code generation and token exchange`
- `feat[core]: implement persistent token store with hashed tokens`

#### 1.6 — mDNS Module
- `src/mdns.rs`:
  - `advertise(port: u16, hostname: &str, fingerprint: &str) -> MdnsGuard`
  - `discover(timeout: Duration) -> Vec<DiscoveredServer>`: service type `_clipsync._tcp.`
  - `DiscoveredServer`: host, port, name, fingerprint, version
  - Parse TXT record for `version`, `name`, `fp`

**Commits:**
- `feat[core]: implement mDNS advertisement and discovery`

#### 1.7 — Clipboard Abstraction
- `src/clipboard.rs`:
  - `ClipboardProvider` trait:
    - `fn read(&self) -> Option<ClipPayload>`
    - `fn write(&self, payload: &ClipPayload) -> Result<()>`
    - `fn watch(&self, tx: Sender<ClipPayload>)` (polling at 500ms)
  - Platform implementations via `arboard`:
    - Text: read/write string
    - Image: read/write PNG bytes
    - File: platform-specific (macOS: file URL, Linux: file path, Windows: HDROP)
  - Echo suppression: track last-written digest, skip if matches

**Commits:**
- `feat[core]: implement cross-platform clipboard abstraction with echo suppression`

#### 1.8 — Core Tests
- Integration tests in `rust/tests/`:
  - `hmac_vectors.rs`: test known HMAC outputs match golden vectors
  - `payload_round_trip.rs`: serialize → deserialize → compare
  - `protocol_compat.rs`: parse golden JSON payloads, verify Kotlin compatibility
- Run: `cargo test -p clipsync-core`

**Commits:**
- `test[core]: add protocol compatibility and HMAC vector tests`

**Success criteria Phase 1:**
- `cargo test -p clipsync-core` passes all tests
- `cargo clippy -p clipsync-core` has no warnings
- HMAC output matches golden test vectors for same inputs
- ClipPayload JSON matches wire format exactly

---

### Phase 2: Server Binary (`clipsync-server`)
**Estimated effort: ~1500 lines of Rust**

#### 2.1 — Server Skeleton
- Create `rust/crates/clipsync-server/Cargo.toml`
- `src/main.rs`:
  - Parse CLI args: `--port`, `--data-dir`, `--no-tray`
  - Initialize: TLS certs, token store, pairing secret (from keyring or generate)
  - Start mDNS advertiser
  - Start clipboard watcher
  - Start axum server with TLS
  - Start tray icon (if not `--no-tray`)

**Commits:**
- `feat[server]: create server binary skeleton with CLI args`

#### 2.2 — HTTP Routes
- `src/routes.rs`:
  - `GET /health` → `{"ok":true,"version":"0.1.0","platform":"<os>"}`
    - Platform: `std::env::consts::OS` → "macOS" / "linux" / "windows"
  - `GET /pair?code=XXXX` with header `X-ClipSync-Device`:
    - Validate code against active PairingCode
    - Generate token + secret + sig
    - Register token in TokenStore
    - Return JSON response
  - `POST /inject`:
    - Parse ClipPayload from body
    - Validate size ≤ 20MB
    - Write to local clipboard
    - Broadcast to WebSocket clients (except sender)
    - Return `{"ok":true,"nonce":"..."}`
  - `GET /ws` → WebSocket upgrade:
    - Validate Bearer token
    - Register in WsHub
    - Forward incoming frames to clipboard + other clients

**Commits:**
- `feat[server]: implement /health and /pair endpoints`
- `feat[server]: implement /inject endpoint with clipboard injection`
- `feat[server]: implement /ws WebSocket upgrade and frame handling`

#### 2.3 — Auth Middleware
- `src/auth.rs`:
  - Axum middleware layer
  - Skip auth for `/health` and `/pair`
  - For `/inject`: validate Bearer token + HMAC signature
  - For `/ws`: validate Bearer token only
  - Return 401 with `"Invalid"` body on failure (matches original protocol)

**Commits:**
- `feat[server]: implement Bearer + HMAC auth middleware`

#### 2.4 — WebSocket Hub
- `src/ws_hub.rs`:
  - `WsHub`: holds `HashMap<Uuid, WsClient>`
  - `WsClient`: sender channel, device label, last_seen
  - `broadcast(payload: &ClipPayload, exclude: Option<Uuid>)`: send to all except sender
  - `register(sender, label) -> Uuid`
  - `unregister(id: Uuid)`
  - Stale client cleanup (ping/pong or timeout)
  - Thread-safe via `Arc<RwLock<...>>` or `tokio::sync::RwLock`

**Commits:**
- `feat[server]: implement WebSocket hub with broadcast and client management`

#### 2.5 — Clipboard Integration
- `src/clipboard_watcher.rs`:
  - Spawns a tokio task that polls clipboard every 500ms
  - On change: create ClipPayload, broadcast via WsHub
  - Echo suppression: skip payloads we just injected
- `src/clipboard_injector.rs`:
  - Receives ClipPayload from routes/ws
  - Writes to local clipboard via core's ClipboardProvider
  - Updates echo suppression state

**Commits:**
- `feat[server]: implement clipboard watching and injection`

#### 2.6 — System Tray
- `src/tray.rs`:
  - Tray icon with ClipSync logo
  - Menu items:
    - "ClipSync Server — Running" (disabled label)
    - "Start Pairing" → generate code, show in tooltip/notification
    - "Connected Devices" → submenu listing devices from TokenStore
    - separator
    - "Quit"
  - Platform notes:
    - **macOS**: native `NSStatusItem` via tray-icon
    - **Linux**: `libappindicator` or StatusNotifierItem (tray-icon handles this)
    - **Windows**: Win32 `Shell_NotifyIcon` (tray-icon handles this)

**Commits:**
- `feat[server]: implement system tray with pairing and device list`

#### 2.7 — Server Tests
- Test routes with `axum::test` helpers
- Test auth middleware rejects bad tokens/signatures
- Test WsHub broadcast logic
- Test clipboard watcher integration

**Commits:**
- `test[server]: add route, auth, and WebSocket integration tests`

**Success criteria Phase 2:**
- `cargo test -p clipsync-server` passes
- Server starts on port 7010 with TLS
- Android client can discover via mDNS, pair, and sync text
- `curl -k https://localhost:7010/health` returns valid JSON

---

### Phase 3: Client Binary (`clipsync-client`)
**Estimated effort: ~1200 lines of Rust**

#### 3.1 — Client Skeleton
- Create `rust/crates/clipsync-client/Cargo.toml`
- `src/main.rs`:
  - Parse CLI args: `--server`, `--no-tray`, `--data-dir`
  - If no stored credentials: run pairing flow
  - Else: connect WebSocket + start clipboard watcher

#### 3.2 — Pairing Flow
- `src/pairing_flow.rs`:
  - Auto mode: mDNS discover → select server → prompt for code → pair
  - Manual mode: `--server <ip:port>` → TOFU cert capture → prompt for code → pair
  - Store credentials: token, secret, host, port, fingerprint (via `keyring` crate)

**Commits:**
- `feat[client]: implement mDNS discovery and pairing flow`

#### 3.3 — WebSocket Connector
- `src/connector.rs`:
  - Connect: `wss://<host>:<port>/ws` with Bearer auth header
  - TLS: custom verifier pinned to stored fingerprint
  - Reconnection: exponential backoff (1s, 2s, 4s, 8s, max 30s)
  - On message: parse ClipPayload, write to clipboard
  - On error: update status, schedule reconnect

**Commits:**
- `feat[client]: implement WebSocket connection with auto-reconnect`

#### 3.4 — Clipboard Send
- `src/sender.rs`:
  - POST /inject with Bearer + HMAC signature
  - Retry on 5xx (max 2 retries)
  - Source header: `X-ClipSync-Source: desktop-client`

- `src/clipboard_watcher.rs`:
  - Poll clipboard every 500ms
  - On change: build ClipPayload → send via sender
  - Echo suppression: skip payloads we just received from server

**Commits:**
- `feat[client]: implement clipboard watching and /inject sending`

#### 3.5 — System Tray (Client)
- `src/tray.rs`:
  - Icon with connection status indicator
  - Menu:
    - "Connected to <server-name>" / "Disconnected" (status)
    - "Pair with Server" → pairing flow
    - "Pause Sync" / "Resume Sync"
    - separator
    - "Quit"

**Commits:**
- `feat[client]: implement system tray with connection status`

#### 3.6 — Client Tests
- Mock server for pairing flow
- Test reconnection logic
- Test HMAC signing matches server validation

**Commits:**
- `test[client]: add pairing, connector, and sender tests`

**Success criteria Phase 3:**
- Client connects to Rust server, pairs, syncs text bidirectionally
- Client reconnects after server restart
- Android + Desktop client connected simultaneously, text syncs to both

---

### Phase 4: Cross-Platform Clipboard Polish
**Estimated effort: ~800 lines**

#### 4.1 — Image Clipboard Support
- **macOS**: TIFF→PNG conversion, PNG read/write via `arboard`
- **Linux/X11**: PNG via `arboard`, test with GNOME/KDE
- **Linux/Wayland**: PNG via `wl-copy`/`wl-paste` fallback if `arboard` fails
- **Windows**: DIB→PNG conversion, CF_DIB read, PNG write

#### 4.2 — File Clipboard Support
- **macOS**: `NSFilenamesPboardType` → read file paths, save received files to `~/Downloads/`
- **Linux**: `text/uri-list` clipboard target, save to `~/Downloads/`
- **Windows**: `CF_HDROP` clipboard format, save to `%USERPROFILE%\Downloads\`
- Received files always saved to Downloads with desktop notification

#### 4.3 — Desktop Notifications
- Platform-native notifications when receiving files/images:
  - macOS: `notify-rust` crate (uses UserNotifications)
  - Linux: `notify-rust` (uses libnotify/D-Bus)
  - Windows: `notify-rust` (uses WinRT Toast)

**Commits:**
- `feat[clipboard]: implement image clipboard support on all platforms`
- `feat[clipboard]: implement file clipboard support on all platforms`
- `feat[clipboard]: add native desktop notifications for received content`

**Success criteria Phase 4:**
- Copy image on Android → appears on Mac/Linux/Windows clipboard
- Copy image on Linux → appears on Android
- Send file from Android → saved in Downloads on all desktop platforms
- Notification shown on receiving image/file

---

### Phase 5: Build, Package, and CI
**Estimated effort: ~500 lines of config**

#### 5.1 — Build Matrix
- GitHub Actions workflow: `.github/workflows/rust-ci.yml`
  - Matrix: `[ubuntu-latest, macos-latest, windows-latest]` × `[server, client]`
  - Steps: `cargo clippy`, `cargo test`, `cargo build --release`
  - Artifacts: upload release binaries

#### 5.2 — Package Scripts
- **macOS**: `.app` bundle (client) / daemon + tray (server) via `cargo-bundle`
- **Linux**: `.deb` + `.AppImage` via `cargo-deb` + `linuxdeploy`
- **Windows**: `.msi` installer via `cargo-wix` or `.exe` + tray

#### 5.3 — Cross-Compilation
- Use `cross` for ARM Linux (Raspberry Pi etc.)
- CI produces binaries for: x86_64-linux, aarch64-linux, x86_64-windows, x86_64-darwin, aarch64-darwin

**Commits:**
- `chore[ci]: add Rust cross-platform build and test workflow`
- `chore[pkg]: add packaging configs for macOS, Linux, and Windows`

**Success criteria Phase 5:**
- CI builds pass on all 3 platforms
- Release binaries produced for 5 targets
- `cargo test` passes on all platforms

---

### Phase 6: Compatibility Testing & Hardening
**Estimated effort: ~600 lines**

#### 6.1 — Cross-Platform Interop Matrix

Test every combination:

| Server ↓ / Client → | Android | Mac (Rust) | Linux (Rust) | Windows (Rust) |
|----------------------|---------|------------|--------------|----------------|
| macOS (Rust)         | test | test | test | test |
| Linux (Rust)         | test | test | test | test |
| Windows (Rust)       | test | test | test | test |

Every cell marked "test" must pass: pair → text sync → image sync → file transfer.

#### 6.2 — Protocol Conformance Tests
- Golden test files: JSON payloads captured from the original Swift server before archival
- Replay against Rust server → must produce identical behavior
- HMAC vectors: same inputs → same outputs across Kotlin/Rust

#### 6.3 — Edge Cases
- Server restart → clients reconnect
- Network change → mDNS re-advertise/re-discover
- Concurrent clipboard changes from multiple clients
- Large payload (20MB image) transfer
- Token revocation → client disconnected
- Clock skew ≥60s → HMAC rejected

**Commits:**
- `test[compat]: add cross-platform interop test suite`
- `test[compat]: add protocol conformance golden tests`

---

## Branch Strategy

```
main ─────────────────────────────────────────────────────── (released v0.1.0, untouched)
  └── dev ─────────────────────────────────────────────────── (integration branch)
       ├── chore/archive-swift ──── Phase 0 ───── merge → dev
       ├── feature/rust-core ────── Phase 1 ───── merge → dev
       ├── feature/rust-server ──── Phase 2 ───── merge → dev
       ├── feature/rust-client ──── Phase 3 ───── merge → dev
       ├── feature/clipboard-polish Phase 4 ───── merge → dev
       ├── chore/rust-ci ────────── Phase 5 ───── merge → dev
       └── feature/compat-tests ─── Phase 6 ───── merge → dev
```

- **`main`**: NEVER touched during this work. Already has v0.1.0 release.
- **`dev`**: Created from `main` at the start. All phase merges go here with `--no-ff`.
- **Feature branches**: One per phase, created from `dev`, merged back to `dev`.
- Merge commit format: `Merge feature/rust-core into dev`

---

## Multi-Agent Execution Architecture

### Three-Tier Agent System

```
┌─────────────────────────────────────────────────────────┐
│  SUPER BOSS (Opus, minimal context)                     │
│  Role: Launch orchestrators, receive handoff reports,   │
│        relaunch with context when orchestrator dies      │
│  Token budget: ~5K per interaction                      │
│  Lives in: /loop with dynamic pacing                    │
│  State file: rust/ORCHESTRATOR_STATE.md                 │
└─────────────┬───────────────────────────────────────────┘
              │ launches
              ▼
┌─────────────────────────────────────────────────────────┐
│  ORCHESTRATOR (Opus, full context)                      │
│  Role: Execute one phase at a time, launch Sonnet       │
│        workers for individual tasks, validate results,  │
│        merge branches, write handoff on context limit   │
│  Token budget: full context window                      │
│  State file: rust/PHASE_PROGRESS.md                     │
└─────────────┬───────────────────────────────────────────┘
              │ launches (parallel when possible)
              ▼
┌─────────────────────────────────────────────────────────┐
│  WORKERS (Sonnet, isolated worktrees)                   │
│  Role: Write code for a specific task, run tests,       │
│        commit to feature branch, report back            │
│  Token budget: full context window                      │
│  Isolation: git worktree per worker                     │
└─────────────────────────────────────────────────────────┘
```

### State Files (Handoff Mechanism)

#### `rust/ORCHESTRATOR_STATE.md`
Written by the Orchestrator before it dies or completes a phase. Read by the Super Boss to decide what to launch next.

```markdown
# Orchestrator State
## Status: IN_PROGRESS | PHASE_COMPLETE | CONTEXT_LIMIT | ERROR
## Current Phase: 1
## Current Task: 1.3
## Completed Tasks: [1.1, 1.2]
## Branch: feature/rust-core
## Last Commit: <sha>
## Notes: <any context the next orchestrator needs>
## Error: <if status is ERROR>
```

#### `rust/PHASE_PROGRESS.md`
Detailed progress tracking within a phase.

```markdown
# Phase 1: Core Library

## Tasks
- [x] 1.1 Workspace + Core Skeleton — commit abc1234
- [x] 1.2 Protocol Types — commit def5678
- [ ] 1.3 HMAC Module — IN PROGRESS (worker launched)
- [ ] 1.4 TLS Module
- [ ] 1.5 Pairing Logic
- [ ] 1.6 mDNS Module
- [ ] 1.7 Clipboard Abstraction
- [ ] 1.8 Core Tests

## Test Results
- cargo test: PASS (12/12) as of commit def5678
- cargo clippy: 0 warnings

## Notes
- <any issues, decisions, workarounds>
```

### Super Boss Prompt Template

```
You are the Super Boss of a multi-agent pipeline building ClipSync cross-platform.
Your ONLY job is to:

1. Read rust/ORCHESTRATOR_STATE.md
2. If status is PHASE_COMPLETE: update the state, launch orchestrator for next phase
3. If status is CONTEXT_LIMIT: launch a NEW orchestrator with the handoff context
4. If status is ERROR: analyze and decide whether to retry or escalate
5. If all phases complete: write final report and stop

You MUST be extremely token-efficient. Do NOT read source code. Do NOT write code.
Only read state files and launch orchestrators.

Current state of the project: read rust/ORCHESTRATOR_STATE.md

The full plan is at: docs/development/cross-platform-plan.md
The branch strategy: feature branches → dev → (never main)

To launch an orchestrator, use the Agent tool with subagent_type "general-purpose"
and pass it the Orchestrator Prompt Template filled with the current phase info.
```

### Orchestrator Prompt Template

```
You are an Orchestrator building ClipSync Phase {N}: {phase_name}.

## Context
- Repo: /Users/2cristo7/Documents/personal-proyects/clip-sync
- Full plan: docs/development/cross-platform-plan.md (READ THIS FIRST)
- Branch strategy: work on feature/{branch_name}, merge to dev (NEVER main)
- Progress: rust/PHASE_PROGRESS.md (READ THIS to see what's done)
- Previous orchestrator notes: {handoff_notes}

## Your Job
1. Read the plan for Phase {N}
2. Read PHASE_PROGRESS.md to see what's already done
3. For each remaining task in the phase:
   a. Launch a Sonnet worker (Agent tool, model: "sonnet") with a COMPLETE prompt
      containing: exact file paths to create, exact code to write, the protocol spec,
      and the commit message to use
   b. When possible, launch independent workers in parallel
   c. After each worker completes, verify: git log, cargo test, cargo clippy
   d. Update PHASE_PROGRESS.md
4. After all tasks complete:
   a. Run full test suite: cargo test
   b. Merge feature branch to dev: git checkout dev && git merge --no-ff feature/{branch}
   c. Update rust/ORCHESTRATOR_STATE.md with status: PHASE_COMPLETE
5. If you're running low on context:
   a. Write everything you know to rust/ORCHESTRATOR_STATE.md with status: CONTEXT_LIMIT
   b. Include detailed notes for the next orchestrator
   c. Stop working

## Rules
- You write NO code yourself — only workers write code
- You validate ALL worker output (test, clippy, correct files)
- You commit and merge — workers commit to the feature branch
- If a worker produces bad code, launch a new worker to fix it
- If cargo test fails after a merge, revert and relaunch the worker

## Worker Prompt Template
When launching workers, include:
- The exact file path(s) to create/modify
- The full protocol spec section they need (copy from the plan)
- The Cargo.toml dependencies they need
- The commit message to use
- "Run cargo test and cargo clippy before reporting done"
- "You are on branch feature/{branch_name}"
```

### Execution Order

```
Super Boss launches:
  │
  ├── Orchestrator: Phase 0 (Archive Swift + Golden Tests)
  │   ├── Worker: 0.1 create dev branch, extract golden test data
  │   ├── Worker: 0.2 git mv mac/ → mac-legacy/, update docs
  │   └── Merge chore/archive-swift → dev
  │
  ├── Orchestrator: Phase 1 (Core Library)
  │   ├── Worker: 1.1 workspace setup
  │   ├── Worker: 1.2 protocol types
  │   ├── Worker: 1.3 HMAC module        ← can run parallel with 1.2
  │   ├── Worker: 1.4 TLS module         ← can run parallel with 1.3
  │   ├── Worker: 1.5 pairing logic      ← depends on 1.3 (HMAC)
  │   ├── Worker: 1.6 mDNS module        ← can run parallel with 1.5
  │   ├── Worker: 1.7 clipboard module   ← independent
  │   └── Worker: 1.8 core tests         ← depends on ALL above
  │   └── Merge feature/rust-core → dev
  │
  ├── Orchestrator: Phase 2 (Server)
  │   ├── Worker: 2.1 server skeleton
  │   ├── Worker: 2.2 HTTP routes        ← depends on 2.1
  │   ├── Worker: 2.3 auth middleware     ← can parallel with 2.2
  │   ├── Worker: 2.4 WebSocket hub      ← can parallel with 2.2
  │   ├── Worker: 2.5 clipboard integration ← depends on 2.2, 2.4
  │   ├── Worker: 2.6 system tray        ← independent of 2.5
  │   └── Worker: 2.7 server tests       ← depends on ALL above
  │   └── Merge feature/rust-server → dev
  │
  ├── Orchestrator: Phase 3 (Client)
  │   ├── Worker: 3.1 client skeleton
  │   ├── Worker: 3.2 pairing flow
  │   ├── Worker: 3.3 WebSocket connector ← depends on 3.1
  │   ├── Worker: 3.4 clipboard send     ← depends on 3.1
  │   ├── Worker: 3.5 system tray        ← independent
  │   └── Worker: 3.6 client tests       ← depends on ALL
  │   └── Merge feature/rust-client → dev
  │
  ├── Orchestrator: Phase 4 (Clipboard Polish)
  │   ├── Worker: 4.1 image clipboard
  │   ├── Worker: 4.2 file clipboard
  │   └── Worker: 4.3 notifications
  │   └── Merge feature/clipboard-polish → dev
  │
  ├── Orchestrator: Phase 5 (CI/Packaging)
  │   ├── Worker: 5.1 GitHub Actions
  │   ├── Worker: 5.2 package scripts
  │   └── Worker: 5.3 cross-compilation
  │   └── Merge chore/rust-ci → dev
  │
  └── Orchestrator: Phase 6 (Compat Testing)
      ├── Worker: 6.1 interop matrix
      ├── Worker: 6.2 conformance tests
      └── Worker: 6.3 edge case tests
      └── Merge feature/compat-tests → dev
```

### Parallelization Strategy Within Phases

**Phase 1 (most parallelizable):**
```
Sequential:  1.1 → then parallel batch → then 1.8
Parallel:    [1.2, 1.3, 1.4, 1.6, 1.7] (all independent of each other)
Sequential:  1.5 (needs 1.3 done for HMAC)
Final:       1.8 (integration tests, needs everything)
```

**Phase 2:**
```
Sequential:  2.1 → then parallel batch → then 2.5 → 2.7
Parallel:    [2.2, 2.3, 2.4, 2.6]
Sequential:  2.5 (needs routes + ws_hub)
Final:       2.7 (tests)
```

**Phases 4-6:** Workers mostly independent, maximize parallelism.

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| `arboard` clipboard doesn't work on Wayland | Fallback to `wl-copy`/`wl-paste` subprocess |
| `tray-icon` doesn't work on some Linux DEs | Headless mode (`--no-tray`) as fallback |
| `mdns-sd` needs elevated perms on some Linux | Document: user must be in `netdev` group or use `avahi-daemon` |
| `rcgen` P-256 fingerprint differs from reference | Golden test: compare fingerprint of known cert from archived server |
| Context limit during complex phase | State file handoff mechanism (described above) |
| Worker produces code that doesn't compile | Orchestrator catches via `cargo check`, relaunches worker |
| Merge conflict between parallel workers | Workers work on different files; orchestrator resolves if needed |
| Windows-specific clipboard issues | `arboard` handles Win32 API; test in CI with windows-latest |

---

## Time Estimate

| Phase | Estimated Duration | Workers Needed |
|-------|-------------------|----------------|
| Phase 0: Archive Swift + Golden Tests | 20 min | 2 workers |
| Phase 1: Core Library | 2-3 hours | 5-7 workers |
| Phase 2: Server | 2-3 hours | 6-7 workers |
| Phase 3: Client | 1.5-2 hours | 5-6 workers |
| Phase 4: Clipboard Polish | 1-1.5 hours | 3 workers |
| Phase 5: CI/Packaging | 1 hour | 3 workers |
| Phase 6: Compat Testing | 1-1.5 hours | 3 workers |
| **Total** | **~9.5-13.5 hours** | **~27-31 worker launches** |

Overhead for orchestrator handoffs: ~30 min total.
**Estimated total overnight run: 10-14 hours.**

---

## Initialization Checklist (Run Before Starting Overnight)

1. `git checkout -b dev` from `main`
2. `mkdir -p rust/crates/clipsync-core/src rust/crates/clipsync-server/src rust/crates/clipsync-client/src rust/tests/golden rust/resources/icons`
3. Create `rust/ORCHESTRATOR_STATE.md` with initial state:
   ```markdown
   # Orchestrator State
   ## Status: NOT_STARTED
   ## Current Phase: 0
   ## Current Task: 0.1
   ## Completed Tasks: []
   ## Branch: chore/archive-swift
   ## Last Commit: (none)
   ## Notes: Fresh start. Read docs/development/cross-platform-plan.md for full plan. Phase 0 archives the Swift server and extracts golden test data.
   ```
4. Create `rust/PHASE_PROGRESS.md` with empty task list for Phase 0
5. Verify: `rustup show` (Rust toolchain installed)
6. Verify: `cargo --version` (Cargo available)
7. Push `dev` branch to remote
8. Launch Super Boss

---

## File Reference Quick Sheet (For Worker Prompts)

Workers need these constants copied into their prompts:

```
PORT = 7010
SERVICE_TYPE = "_clipsync._tcp."
VERSION = "0.1.0"
MAX_PAYLOAD_BYTES = 20 * 1024 * 1024
PAIRING_CODE_TTL_SECS = 120
HMAC_MAX_SKEW_SECS = 60
SECRET_BYTES = 32
TOKEN_BYTES = 32
CERT_VALIDITY_DAYS = 365
POLL_INTERVAL_MS = 500
```

Wire format JSON:
```json
{"type":"text","mime":"text/plain","data":"aGVsbG8=","ts":1714000000,"nonce":"uuid-v4","name":null}
```

HMAC header:
```
X-ClipSync-Signature: t=1714000000, v1=abcdef0123456789...
Signing: HMAC-SHA256(secret, "1714000000.{json_body}")
```
