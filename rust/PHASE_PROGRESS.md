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
