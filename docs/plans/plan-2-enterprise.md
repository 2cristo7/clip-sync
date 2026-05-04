# Plan 2 — ClipSync Enterprise (Rust)

## Goal

Build the office/enterprise variant of ClipSync as a Rust workspace product. Topology: one **dedicated server** + N **clients**. The server hosts a serious admin **dashboard** for granular control:

- Per-device policy: read-only, write-only, mute, follow-leader, paused
- Real-time device list with status (online, latency, last paste)
- Audit log of all clipboard events (queryable, filterable)
- Broadcast feature: server pushes 1 file to N selected clients (max ~50 MB, no chunking)
- Quick toggles: "only push what *I* copy" mode, "freeze syncing", "kick device"

UI tone: **serious, dense admin panel**. Neutral palette (grays + corporate blue/teal accent), system fonts, tabular layouts, no decorative animations beyond functional transitions. Keyboard-driven where reasonable.

Audience: small/medium offices (5–30 devices) wanting frictionless file-passing across machines instead of WhatsApp/email.

Mac users CAN install the enterprise client alongside the existing Swift personal app. Both coexist.

## Base branch

`product/enterprise` off tag `dev-v0.2-ready-for-fork` (created at end of Plan 1).

## Cross-cutting design constraints

- Mobile (Android Kotlin) is reused as-is via the same wire protocol — no Android changes required for Plan 2 minimum viable product. If Android needs enterprise UI hooks later, that's a separate plan.
- Mac Swift app on `main` is **not** an enterprise client. Mac users wanting enterprise install the Tauri enterprise client.
- Server is headless by default (Linux daemon). It can also run on Mac/Windows for small offices that don't have a Linux box.
- Dashboard is a separate Tauri+React app that connects to the server over the same authenticated WebSocket + REST API.
- Protocol changes go through `clipsync-protocol` crate via PR to `dev`, then this branch rebases. Plan 3 stays compatible.

## Phases (one branch per phase, off `product/enterprise`)

Phases 2.1 → 2.6 are the server-side / protocol track. Phases 2.7 → 2.11 are the dashboard track. Track 1 must reach 2.5 (broadcast) before track 2 can implement broadcast UI; otherwise the two tracks can interleave.

### Phase 2.1 — `feat/enterprise-server-binary-headless`

Files:
- New `rust/apps/enterprise-server/` (binary crate)
- `rust/apps/enterprise-server/src/main.rs`, `config.rs`, `cli.rs`

Rules:
- Headless daemon: no tray, no UI, structured JSON logs to stdout, configurable via TOML config file + CLI flags
- Flags: `--config <path>`, `--port <int>`, `--data-dir <path>`, `--log-level`, `--bind <addr>`
- SIGTERM handling: graceful shutdown, drain WS connections within 10s
- Reuses `clipsync-protocol`, `clipsync-crypto`, `clipsync-transport` crates from Plan 1

Success criteria:
- `cargo run -p enterprise-server -- --port 7010 --data-dir /tmp/cs` starts and accepts WS connections
- Existing Android v0.1.1 client can pair and sync clipboard against this server (regression check)
- Systemd unit file checked in at `rust/apps/enterprise-server/packaging/clipsync-enterprise.service`

### Phase 2.2 — `feat/enterprise-device-registry`

Files:
- `rust/crates/clipsync-storage/` (new crate, SQLite via `sqlx` async)
- `rust/apps/enterprise-server/src/registry.rs`

Rules:
- SQLite database at `<data-dir>/clipsync.db`, migrations in `rust/crates/clipsync-storage/migrations/`
- Tables:
  - `devices(id PK, name, fingerprint, role, paired_at, last_seen, token_hash)`
  - `tokens(token_id PK, device_id FK, created_at, revoked_at)`
- Each pairing creates a unique device + token. Tokens can be revoked individually.
- Token hash, not raw token, stored at rest.

Success criteria:
- Pair 3 mock clients sequentially → all 3 visible via `SELECT * FROM devices`
- Revoke one → it can no longer authenticate WS connection (returns 401)
- Migration tests via `sqlx::test`

### Phase 2.3 — `feat/enterprise-protocol-role-handshake`

Files:
- `rust/crates/clipsync-protocol/src/handshake.rs` (extending Plan 1 Phase 1.9 hooks)
- Server WS handler enforces role + capabilities

Rules:
- Client opens WS → first frame is `Hello { device_id, role: "client", capabilities: [...], protocol_version: 2 }`
- Server replies `Welcome { server_id, server_capabilities, your_policy }` where `your_policy` is the device's current ACL snapshot
- Subsequent clipboard frames go through the policy filter (Phase 2.4)
- Backward-compat: if client doesn't send `Hello` (older clients like Android v0.1.1 + Mac Swift), server treats as legacy peer with default policy `read_write`

Success criteria:
- Old Android v0.1.1 client still pairs and syncs (no `Hello` frame)
- New enterprise client sends `Hello`, server records role
- Reject `protocol_version` greater than supported with explicit error frame

### Phase 2.4 — `feat/enterprise-policy-engine`

Files:
- `rust/crates/clipsync-policy/` (new crate)
- `rust/apps/enterprise-server/src/policy_runtime.rs`

Rules:
- Policy enum per device:
  - `ReadWrite` (default; bidirectional)
  - `ReadOnly` (receives clipboard, cannot push)
  - `WriteOnly` (pushes clipboard, doesn't receive)
  - `Muted` (paired but no clipboard flow either direction)
  - `FollowLeader { leader_device_id }` (only receives clipboard from a specific device)
- Policy stored in `devices.policy` JSON column (migration)
- WS hub consults policy before fan-out and before accepting `/inject` from a device
- Policy changes apply to live connections within 1s (no reconnect required)

Success criteria:
- Three integration tests: `read_only` device cannot push, `write_only` cannot receive, `follow_leader` only receives from designated source
- Policy change from API endpoint applies to running session

### Phase 2.5 — `feat/enterprise-broadcast-endpoint`

Files:
- `rust/apps/enterprise-server/src/routes/broadcast.rs`
- New WS frame variant: `BroadcastChunk` — wait, we said no chunking. Single frame with full file.
- New WS frame variant: `BroadcastFile { id, name, mime, bytes_b64, sender_device_id, target_device_ids }`

Rules:
- `POST /broadcast` (multipart): file + `target_device_ids[]`
- Server stores file temporarily (`<data-dir>/broadcasts/<id>`) for retry, expires after 1h
- Server pushes `BroadcastFile` frame to each target client; if a client is offline, server queues and delivers on reconnect (within 1h)
- Per-client delivery progress: server reports `{ device_id, status: pending|delivered|failed }` over a status WS event
- Hard cap 50 MB; reject larger with 413
- Audit log entry created (Phase 2.6)

Success criteria:
- Upload 10 MB PDF, target 3 mock clients → all 3 receive identical bytes
- Offline target receives on reconnect within 1h window
- Over-50MB upload returns 413 with descriptive body

### Phase 2.6 — `feat/enterprise-audit-log`

Files:
- `rust/crates/clipsync-storage/migrations/00X_audit.sql`
- `rust/apps/enterprise-server/src/audit.rs`

Rules:
- Table `audit(id PK, ts, event_type, device_id, payload_summary, metadata_json)`
- Event types: `device_paired`, `device_revoked`, `clipboard_pushed`, `clipboard_delivered`, `broadcast_sent`, `broadcast_delivered`, `policy_changed`, `connection_opened`, `connection_closed`
- Payload summary: hashed content + size + kind (no raw clipboard text stored — privacy)
- Query API `GET /audit?from=…&to=…&device_id=…&event_type=…&limit=…`
- Retention policy: rolling 30 days, configurable

Success criteria:
- Every state change touches the audit table
- Query API returns paginated results
- Logs do NOT contain raw clipboard contents (verified by test)

### Phase 2.7 — `feat/enterprise-tauri-skeleton`

Files:
- `rust/apps/enterprise-desktop/` (Tauri 2 + React + Vite + TypeScript)
- Tailwind CSS configured with corporate palette
- Routing via `react-router`

Rules:
- Tauri 2, frontend in `frontend/`, Rust glue in `src-tauri/`
- Layout: left sidebar (sections: Devices, Audit, Broadcast, Settings), top bar (server status, version), main content area
- Auth: dashboard authenticates to server with admin token (env var or one-time setup screen)
- Connects via WS for real-time updates + REST for queries
- Visual: serious, dense, data-rich. Reference: Linear, Stripe Dashboard, Sentry. Avoid playful.

Success criteria:
- `npm run tauri dev` opens window with sidebar + empty pages
- Dark mode + light mode both work, default = system
- App connects to a running enterprise-server and shows "connected"

### Phase 2.8 — `feat/enterprise-dashboard-devices`

Files:
- `rust/apps/enterprise-desktop/frontend/src/pages/Devices.tsx`
- Backing Tauri commands

Rules:
- Real-time table: name, role, status (online/offline + latency badge), policy, last_seen, paired_at, actions
- Sortable columns, search by name/id
- Row actions: change policy, revoke, kick session
- Detail drawer: full device info, recent activity (last 50 audit events for that device)

Success criteria:
- Pair 5 mock devices → all visible, status updates within 2s
- Revoke a device → row updates, device disconnects on next ping
- Sort by last_seen works correctly across timezone boundaries

### Phase 2.9 — `feat/enterprise-dashboard-policy-editor`

Files:
- `rust/apps/enterprise-desktop/frontend/src/components/PolicyEditor.tsx`

Rules:
- Per-device policy editor with the 5 policy modes
- For `FollowLeader`, dropdown picks leader from currently paired devices
- "Apply to all" bulk action to set the same policy on multi-selected rows
- Confirmation modal for `Muted` and revocation actions

Success criteria:
- Edit policy on a device → change reflects in table within 1s
- Bulk-apply `ReadOnly` to 3 devices → all 3 update atomically (server-side transaction)

### Phase 2.10 — `feat/enterprise-dashboard-audit-viewer`

Files:
- `rust/apps/enterprise-desktop/frontend/src/pages/Audit.tsx`

Rules:
- Filterable table: date range, device, event type
- Virtualized rows (use `@tanstack/react-virtual`) for >10k rows performance
- CSV export
- Real-time tail toggle: when on, new events stream in at the top

Success criteria:
- Load 10k synthetic events → smooth scroll
- Filter by event type narrows correctly
- CSV export opens in Excel/Numbers without encoding issues

### Phase 2.11 — `feat/enterprise-dashboard-broadcast-ui`

Files:
- `rust/apps/enterprise-desktop/frontend/src/pages/Broadcast.tsx`

Rules:
- Drag-drop file zone → max 50 MB validation client-side
- Target picker: device list with multi-select + "All online" / "All paired" presets
- Send → progress per device with status badges (pending/delivered/failed)
- History pane: previous broadcasts (last 20)

Success criteria:
- Send 5 MB file to 3 devices → progress updates live, completes
- Cancel during send → server cancels pending deliveries

### Phase 2.12 — `feat/enterprise-client-tauri`

Files:
- `rust/apps/enterprise-client/` (Tauri 2 + React, lighter UI)

Rules:
- Tray-only app: no main window by default. Tray menu shows: connection status, current policy mode, "Show recent clips", "Pause sync", "Quit".
- Receives `BroadcastFile` frames → notification with "Open" / "Save As" / "Reveal in Finder/Explorer"
- Sends clipboard to server respecting current policy
- No admin features in this app — purely the receiving-side counterpart

Success criteria:
- Pair against enterprise-server, receive clipboard pushes
- Receive a broadcast file, save to `~/Downloads/ClipSync/`, open native reveal
- Pause from tray → no clipboard frames sent for the duration

### Phase 2.13 — `chore/enterprise-packaging`

Files:
- `rust/apps/enterprise-server/packaging/` — Linux deb (`cargo-deb`), Linux rpm (`cargo-generate-rpm`), Windows MSI (`wix-rs` or Tauri bundler), Mac DMG
- `rust/apps/enterprise-desktop/src-tauri/tauri.conf.json` — bundle settings for Win MSI, Linux AppImage + deb, Mac DMG (universal arm64+x86_64 if feasible; arm64-only acceptable)
- `rust/apps/enterprise-client/src-tauri/tauri.conf.json` — same matrix

Rules:
- Server: arm64 + amd64 for Linux, x64 + arm64 Windows MSI, universal Mac DMG
- Desktop + Client: same matrix
- Code signing: ad-hoc on Mac (no Developer ID until later), self-signed on Win acceptable for dev, GPG-signed deb/rpm with project key

Success criteria:
- CI builds all artifacts and uploads to a draft release on origin (the GitHub release for enterprise products is allowed; only the Swift+Kotlin product on `main` is off-limits to release tagging by these plans). Local build script `scripts/build-enterprise-release.sh` reproduces the same artifacts in `releases/enterprise/<version>/`
- Each artifact installs cleanly on a fresh VM

### Phase 2.14 — `test/enterprise-compat`

Files:
- `rust/tests/enterprise/` — integration tests
- `docs/cross-platform-interop.md` — extend matrix

Rules:
- Test enterprise-server ↔ Android v0.1.1 (legacy compat)
- Test enterprise-server ↔ enterprise-client (full feature path)
- Test enterprise-server ↔ Mac Swift personal client (legacy compat — should work as `ReadWrite` device with no `Hello` frame)
- Policy enforcement tests: each of 5 policies behaves correctly across reconnects
- Broadcast test: 3 simulated clients receive a 10MB file

Success criteria:
- All compat scenarios green on CI
- Mac Swift personal client can connect to enterprise-server in `ReadWrite` mode (no breaking changes for legacy clients)

### Phase 2.15 — `docs/enterprise-deployment-guide`

Files:
- `docs/enterprise/installation.md`
- `docs/enterprise/admin-guide.md`
- `docs/enterprise/security.md`

Rules:
- Install steps for Linux server (deb + systemd), Windows server (MSI + service), Mac server (DMG + launchd)
- Admin walkthrough: pair first device, set policies, run broadcast
- Security: TLS bring-your-own-cert option, token rotation, audit retention

Success criteria:
- A non-author can follow the install doc on a clean Ubuntu 24.04 VM and reach a working enterprise-server in under 15 minutes

### Phase 2.16 — `chore/enterprise-ci-tauri-matrix`

Plan 1 Phase 1.1 already shipped `.github/workflows/ci-enterprise.yml` as a Linux check stub. This phase enriches it with the multi-platform Tauri build matrix and the GitHub Release upload on tag.

Files:
- `.github/workflows/ci-enterprise.yml` (extend, not replace)
- `.github/workflows/release-enterprise.yml` (new — fires on tag push `enterprise-v*`)

Rules for `ci-enterprise.yml` (every push to `product/enterprise` and `feat/enterprise-**`, `chore/enterprise-**`, `test/enterprise-**`):
- Job `lint-and-test` (Linux ubuntu-latest): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo nextest run --workspace --exclude-from-binary tauri-apps`. Cache via `Swatinem/rust-cache@v2`. Includes frontend lint: `cd rust/apps/enterprise-desktop/frontend && npm ci && npm run lint && npm run typecheck`
- Job `frontend-build` (Linux): `cd rust/apps/enterprise-desktop/frontend && npm ci && npm run build`. Validates Vite build but doesn't bundle Tauri
- `concurrency: { group: ci-enterprise-${{ github.ref }}, cancel-in-progress: true }`
- `paths-ignore: ['**.md', 'docs/**', '**/screenshots/**', '*.png']`

Rules for `release-enterprise.yml` (only on tag push matching `enterprise-v*`):
- Matrix: `ubuntu-latest` (Linux AppImage + deb + rpm + server tar.gz), `windows-latest` (Win MSI for desktop, client, server), `macos-14` (Mac DMG arm64 for desktop, client, server). Use `tauri-apps/tauri-action@v0` for Tauri bundling
- Each runner uploads its artifacts to a draft GitHub release named exactly `Enterprise v0.X.Y` (must include the `Enterprise` prefix per Plan 2 master criteria)
- Server-only artifacts produced by `cargo build --release -p enterprise-server` and packaged via `cargo-deb` / `cargo-generate-rpm` / `wix` / `dmgbuild`
- Code signing: ad-hoc on macOS (`codesign --force --deep --sign -`), self-signed on Windows for v0.1.x (real Authenticode cert deferred), GPG-signed deb/rpm with project key (skip if no key in secrets — log warning, do not fail)

Speed boosts:
- Tag-only trigger means no Tauri builds on every commit (3-platform matrix bundling is the slow part)
- `Swatinem/rust-cache@v2` per matrix job
- `actions/cache@v4` for `~/.npm` and Vite cache
- `cargo-deb` and `cargo-generate-rpm` are pre-installed via `taiki-e/install-action@v2`

Success criteria:
- Push a no-op commit to `product/enterprise` → only `ci-enterprise.yml` fires, completes lint+test+frontend-build in <5 minutes
- Push tag `enterprise-v0.0.1-test` → `release-enterprise.yml` fires, all three matrix jobs succeed, draft release exists on origin with at least 6 artifacts (3 desktop + 3 client + 3 server)
- Delete the test tag and draft release after validation
- `actionlint` passes on both workflow files

## Master success criteria for Plan 2

- All 16 phase branches merged to `product/enterprise`
- Server, desktop dashboard, and client all build + package across Win/Linux/Mac
- Compat tests against Android v0.1.1 and Mac Swift v0.1.1 personal client pass
- Tag `enterprise-v0.1.0` on `product/enterprise`, push tag to origin
- Draft GitHub release (named `Enterprise v0.1.0`, NOT touching the public Swift+Kotlin v0.1.x release naming) with all installer artifacts
- `main` branch unchanged throughout the plan
