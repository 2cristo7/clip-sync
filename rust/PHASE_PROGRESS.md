# Phase 0: Archive Swift Server & Extract Golden Tests — COMPLETE
## Tasks
- [x] 0.1 Create dev branch, extract golden test data
- [x] 0.2 Archive Swift server (git mv mac/ mac-legacy/), update docs
- [x] 0.3 Merge chore/archive-swift → dev

---

# Phase 1: Core Library (clipsync-core)
## Tasks
- [x] 1.1 Workspace + Core Skeleton
- [x] 1.2 Protocol Types
- [x] 1.3 HMAC Module
- [x] 1.4 TLS Module
- [x] 1.5 Pairing Logic
- [x] 1.6 mDNS Module
- [x] 1.7 Clipboard Abstraction
- [x] 1.8 Core Tests
## Test Results
62 tests passed (47 unit + 15 integration), 0 failed. Clippy clean.
## Notes
Branch: feature/rust-core from dev
Parallelism: 1.2+1.3 parallel, 1.4 parallel with 1.3, 1.5 depends on 1.3, 1.7 independent, 1.8 depends on all

---

# Phase 2: Server Binary (clipsync-server)
## Tasks
- [x] 2.1 Server Skeleton — CLI args, Cargo.toml deps, main.rs init sequence
- [x] 2.2 HTTP Routes — /health, /pair, /inject, /ws (implemented in skeleton)
- [x] 2.3 Auth Middleware — Bearer + HMAC validation (implemented in skeleton)
- [x] 2.4 WebSocket Hub — broadcast, register/unregister (implemented in skeleton)
- [x] 2.5 Clipboard Integration — watcher + injector (implemented in skeleton)
- [x] 2.6 System Tray — tray-icon + muda with menu items
- [x] 2.7 Server Tests — 15 integration tests
## Test Results
77 tests passed (62 core + 15 server), 0 failed. Clippy clean.
## Notes
Branch: feature/rust-server from dev
All modules implemented: routes, auth, ws_hub, clipboard watcher/injector, tray.
Fixed rustls crypto provider conflict (ring-only across workspace).

---

# Phase 3: Client Binary (clipsync-client)
## Tasks
- [x] 3.1 Client Skeleton — CLI args (clap), Cargo.toml deps, credentials module
- [x] 3.2 Pairing Flow — mDNS discovery, TOFU cert pinning, code exchange
- [x] 3.3 WebSocket Connector — wss:// with Bearer auth, fingerprint pinning, exponential backoff
- [x] 3.4 Clipboard Send + Watcher — POST /inject with HMAC, polling with echo suppression
- [x] 3.5 System Tray Client — tray-icon with status, pair/pause/resume/quit menu
- [x] 3.6 Client Tests — 12 integration tests + 15 unit tests
## Test Results
104 tests passed (47 core + 15 server + 15 client unit + 12 client integration + 15 server integration), 0 failed. Clippy clean.
## Notes
Branch: feature/rust-client from dev
Fixed core TLS tests (crypto provider ambiguity from aws-lc-rs pulled by tokio-tungstenite).

---

# Phase 4: Cross-Platform Clipboard Polish — COMPLETE
## Tasks
- [x] 4.1 Image Clipboard Support — TIFF→PNG (macOS), Wayland wl-paste fallback (Linux), BMP→PNG (Windows), arboard RGBA encode/decode
- [x] 4.2 File Clipboard Support — macOS AppleScript file URL, Linux text/uri-list, Windows CF_HDROP via PowerShell, save to ~/Downloads with conflict resolution
- [x] 4.3 Desktop Notifications — notify-rust in core, server clipboard_injector, client connector; image/file payloads trigger native toast
## Test Results
111 tests passed (54 core + 15 server unit + 15 server integration + 15 client unit + 12 client integration), 0 failed. Clippy clean.
## Notes
Branch: feature/clipboard-polish from dev
Added deps: image 0.25 (png/tiff/bmp), notify-rust 4, to core/server/client.
New helpers: tiff_to_png, bmp_to_png, save_received_file, mime_from_extension, url_decode.
7 new tests in clipboard.rs (PNG roundtrip, MIME detection, URL decode, file ops, notification truncation).
