# Plan 3 — ClipSync Personal (Rust)

## Goal

Build the regular-user variant of ClipSync as a Rust workspace product. Topology: **mesh any-to-any**. Every desktop is a peer that both serves and consumes clipboard. Mobile (Android Kotlin) joins as an additional peer using the same wire protocol. There is no "server" the user has to choose.

Audience: a normal user with 2–3 PCs (Windows, Linux, Mac) plus their phone, who wants frictionless clipboard sharing without thinking about topology.

UI tone: **friendly, easy, hides complexity**. Warm palette (off-white + soft accent like coral or mint), rounded corners, gentle motion, big-tap targets. Onboarding-led. Advanced controls live behind a hidden panel — main screen assumes "you want everything connected".

Mac Swift personal app on `main` continues to exist for users who prefer the native Mac experience. The Rust personal app on Mac coexists with it — they speak the same protocol, so they can pair with each other if a user installs both. Most Mac users will pick one or the other.

## Base branch

`product/personal` off tag `dev-v0.2-ready-for-fork` (created at end of Plan 1).

## Cross-cutting design constraints

- No central server — every peer advertises and discovers via mDNS, then forms direct WS connections
- All peers are equal: clipboard events propagate via a small gossip set (each peer pushes to all *paired* peers it has direct WS link to)
- Single binary per OS: `clipsync-personal` runs as a tray app, no separate "server" install
- Mobile (Android Kotlin app) reused as-is — it pairs to one of the desktop peers using the existing pairing flow
- Mac Swift app remains compatible — it can pair to a Rust personal peer like any other client (they treat each other as a peer)
- Protocol changes go through `clipsync-protocol` crate via PR to `dev`, then this branch rebases. Plan 2 stays compatible.

## Phases (one branch per phase, off `product/personal`)

### Phase 3.1 — `feat/personal-mesh-discovery`

Files:
- `rust/crates/clipsync-transport/src/mesh.rs` (new module — extends existing mDNS)
- `rust/apps/personal-desktop/src/discovery.rs`

Rules:
- Each peer advertises mDNS service `_clipsync._tcp.local` with TXT record `role=peer,proto=2,id=<device_id>`
- Each peer continuously browses for other peers; when one appears, attempt connection if already paired (otherwise show in "discovered" list awaiting pairing)
- mDNS does not work over Tailscale — so the app also accepts a manual "Add by IP" fallback (documented in `docs/guides/tailscale-setup.md`)

Success criteria:
- 3 peers on the same LAN auto-discover each other within 3s
- Connection survives a peer leaving and re-joining the LAN

### Phase 3.2 — `feat/personal-mesh-protocol`

Files:
- `rust/crates/clipsync-protocol/src/mesh.rs` (additive, optional fields in `Hello`)
- `rust/apps/personal-desktop/src/peer_link.rs`

Rules:
- Each peer maintains a WS connection to every other paired peer (mesh, not star)
- Clipboard event → push to all connected paired peers in a single broadcast (each peer's `WebSocketHub` from Plan 1 reused)
- Echo suppression: each event carries `origin_device_id`; receivers ignore events they originated, and de-duplicate by `(origin_id, ts)` for 30s
- N peers → N×(N−1)/2 edges. For N ≤ 10 (typical personal use) this is fine; document the upper bound

Success criteria:
- Copy on peer A → appears on B and C within 200ms (LAN)
- Echo test: copy on A, then immediately on B → A doesn't re-broadcast B's event back to B

### Phase 3.3 — `feat/personal-pairing-ux`

Files:
- `rust/apps/personal-desktop/frontend/src/pages/Pairing.tsx`
- Pairing logic in `rust/apps/personal-desktop/src/pairing.rs`

Rules:
- Pairing modes (in priority order):
  1. **Auto-trust on same LAN with confirmation prompt**: when a peer is discovered, the user sees a toast "Found 'Pablo's MacBook' on this network. Pair?". One-tap accept.
  2. **6-digit OTP** for manual pairing (matches existing flow, used when auto-trust is dismissed or for cross-network pairing)
  3. **QR code** (mainly for phone ↔ desktop pairing — phone scans desktop's QR)
- HMAC pairing-secret + token model from Plan 1 reused unchanged
- Auto-trust does NOT skip cryptographic pairing — it just presents the OTP-equivalent flow as a single tap by exchanging the shared secret over the LAN with confirmation on both sides

Success criteria:
- Pair desktop ↔ desktop in under 5 seconds via auto-trust
- Phone scans QR on desktop → paired in under 10 seconds
- Reject pairing from a peer the user dismissed (no nag spam)

### Phase 3.4 — `feat/personal-tauri-skeleton`

Files:
- `rust/apps/personal-desktop/` (Tauri 2 + React + Vite + TypeScript)
- Tailwind CSS with warm palette (config token: cream `#FAF7F2`, accent coral `#FF6B6B` or mint `#5EE2C7`, dark variant inverts)

Rules:
- Tauri 2, single window 480×640 px (resizable, but designed at this size)
- Routing minimal: Home, Onboarding (first run), Advanced (hidden behind a settings cog)
- Visual references: Linear's mobile companion, Notion mobile, Things 3. Friendly, breathable, generous spacing. Avoid corporate density.
- Light/dark/system themes

Success criteria:
- `npm run tauri dev` shows the home shell on Mac/Win/Linux
- Theme toggle works
- Window sizing feels right at the design size

### Phase 3.5 — `feat/personal-onboarding-wizard`

Files:
- `rust/apps/personal-desktop/frontend/src/pages/Onboarding.tsx`

Rules:
- 3 screens, no backstep dread:
  1. **Welcome** — illustration + 1-line value prop ("Copy here, paste anywhere") + Continue
  2. **Discover** — auto-scans LAN, shows peers as cards, each with "Pair" button. Or "Skip — I'll do it later"
  3. **Done** — checkmark, "You're all set" + tip about the tray icon
- Persists `onboarding_completed: true` so it doesn't show again

Success criteria:
- A user with no prior knowledge can install + onboard + receive a clipboard event in < 60 seconds
- Skip path leaves the app functional with empty peer list

### Phase 3.6 — `feat/personal-main-ui`

Files:
- `rust/apps/personal-desktop/frontend/src/pages/Home.tsx`

Rules:
- Top: master pause toggle (big switch, "Sync is on / off")
- Body: list of paired devices, each as a card with name, online dot, last sync time, simple "Forget" action on long-press / right-click
- Bottom: "Add device" button (re-opens discovery flow)
- Cog icon (top-right) → opens Advanced panel
- That's it. No tabs, no submenus on the home screen.

Success criteria:
- 3 paired devices visible as cards with live status
- Master pause stops all clipboard flow within 1s
- Adding a 4th device works without leaving the home flow more than the discovery overlay

### Phase 3.7 — `feat/personal-advanced-panel`

Files:
- `rust/apps/personal-desktop/frontend/src/pages/Advanced.tsx`

Rules:
- Sectioned settings:
  - Per-device toggle: receive / send / both (this is the personal-version equivalent of enterprise's policy engine, but minimal)
  - Clipboard kinds: text, image, files (toggle each)
  - Notifications: toast on receive (on/off), beep (on/off)
  - Autostart on login (on/off)
  - Tailscale fallback hostname (optional input)
  - Debug log viewer (collapsible, last 200 lines)
  - Reset & re-pair everything (destructive button with confirm)

Success criteria:
- All toggles persist (TOML config under `<config-dir>/clipsync-personal/`)
- Disabling "image" stops images from syncing without affecting text

### Phase 3.8 — `feat/personal-broadcast-files`

Files:
- `rust/apps/personal-desktop/frontend/src/components/SendFile.tsx`
- `rust/apps/personal-desktop/src/broadcast.rs`

Rules:
- Drag-drop a file onto the home or tray menu → "Send to N devices?" overlay with peer list + Send
- Reuses the same broadcast frame as Plan 2 (`BroadcastFile { id, name, mime, bytes_b64, ... }`)
- Max 50 MB; reject larger with friendly "File too big — share via email instead" message
- Receiver shows notification + "Open" / "Save As" / "Reveal"

Success criteria:
- Drop 3 MB image on home screen, send to 2 peers → both receive within 5s
- Receiver notification has working "Reveal in Finder/Explorer" action

### Phase 3.9 — `feat/personal-tray-and-notif`

Files:
- `rust/apps/personal-desktop/src/tray.rs`
- Notification glue using `notify-rust`

Rules:
- Tray menu (left-click on Mac, right-click on Win/Linux):
  - Status line: "Synced just now" / "Paused" / "Disconnected"
  - Quick toggles: pause, send file, show window, quit
- Notification on receive: title = "Clipboard from <device>", body = preview (first 80 chars for text, "Image received", "File: <name>")
- Click notification → focus app or open file
- Optional sound (off by default)

Success criteria:
- All tray actions work on Mac/Win/Linux
- Notification preview truncation correct in all 3 OSes (no broken UTF-8)
- Don't bombard: throttle to 1 notification per second per peer

### Phase 3.10 — `chore/personal-packaging`

Files:
- `rust/apps/personal-desktop/src-tauri/tauri.conf.json`
- Bundlers: Mac DMG, Win MSI, Linux AppImage + deb

Rules:
- Mac: arm64 only initially (matches existing CLAUDE.md guidance for mac DMG); add x86_64 only if user demand surfaces
- Win: x64 MSI
- Linux: AppImage (universal) + deb (amd64) + rpm (amd64)
- Versioning: `personal-v0.1.0` tag on first release (push tag to origin). Artifacts written to `releases/personal/v0.1.0/` and uploaded to the draft GitHub release for the personal product (the public Swift+Kotlin v0.1.x release on `main` is NOT touched)

Success criteria:
- All artifacts build on CI and install cleanly on a fresh VM
- Mac DMG opens without Gatekeeper prompt locally (ad-hoc signed); from-web download will trigger Gatekeeper which is acceptable for v0.1.0

### Phase 3.11 — `test/personal-compat`

Files:
- `rust/tests/personal/` integration tests
- Update `docs/cross-platform-interop.md`

Rules:
- Personal mesh ↔ Android v0.1.1 (Android pairs to one peer, full clipboard syncs)
- Personal mesh ↔ Mac Swift v0.1.1 (treats Mac Swift as a peer, clipboard flows both ways)
- 3-peer mesh stress test: copy 100 events in 60s across 3 peers, verify no duplicates, no losses
- Network partition test: peer leaves LAN → other peers detect disconnect within 30s, retry on rejoin

Success criteria:
- All compat scenarios green on CI
- 100-event stress test: 0 duplicates, ≤ 1% loss

### Phase 3.12 — `docs/personal-user-guide`

Files:
- `docs/personal/install.md`
- `docs/personal/quickstart.md`
- `docs/personal/faq.md`

Rules:
- Install steps for Mac DMG, Win MSI, Linux AppImage
- Quickstart: first pair + first clipboard sync in pictures
- FAQ: "It says 'no devices found'" / "How do I use it over Tailscale?" / "How do I uninstall?"

Success criteria:
- A non-technical friend can follow `quickstart.md` and reach a paired state without asking questions

### Phase 3.13 — `chore/personal-ci-tauri-matrix`

Plan 1 Phase 1.1 already shipped `.github/workflows/ci-personal.yml` as a Linux check stub. This phase enriches it with the multi-platform Tauri build matrix and the GitHub Release upload on tag.

Files:
- `.github/workflows/ci-personal.yml` (extend, not replace)
- `.github/workflows/release-personal.yml` (new — fires on tag push `personal-v*`)

Rules for `ci-personal.yml` (every push to `product/personal` and `feat/personal-**`, `chore/personal-**`, `test/personal-**`):
- Job `lint-and-test` (Linux ubuntu-latest): `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo nextest run --workspace --exclude-from-binary tauri-apps`. Cache via `Swatinem/rust-cache@v2`. Includes frontend lint: `cd rust/apps/personal-desktop/frontend && npm ci && npm run lint && npm run typecheck`
- Job `frontend-build` (Linux): `cd rust/apps/personal-desktop/frontend && npm ci && npm run build`. Validates Vite build but doesn't bundle Tauri
- `concurrency: { group: ci-personal-${{ github.ref }}, cancel-in-progress: true }`
- `paths-ignore: ['**.md', 'docs/**', '**/screenshots/**', '*.png']`

Rules for `release-personal.yml` (only on tag push matching `personal-v*`):
- Matrix: `ubuntu-latest` (Linux AppImage + deb), `windows-latest` (Win MSI), `macos-14` (Mac DMG arm64). Use `tauri-apps/tauri-action@v0` for Tauri bundling
- Each runner uploads to a draft GitHub release named exactly `Personal v0.X.Y` (must include the `Personal` prefix per Plan 3 master criteria)
- Code signing: ad-hoc on macOS (`codesign --force --deep --sign -`), self-signed on Windows for v0.1.x (real Authenticode cert deferred). Linux artifacts unsigned for v0.1.x

Speed boosts:
- Tag-only trigger means no Tauri builds on every commit
- `Swatinem/rust-cache@v2` per matrix job
- `actions/cache@v4` for `~/.npm` and Vite cache

Success criteria:
- Push a no-op commit to `product/personal` → only `ci-personal.yml` fires, completes lint+test+frontend-build in <5 minutes
- Push tag `personal-v0.0.1-test` → `release-personal.yml` fires, all three matrix jobs succeed, draft release exists on origin with at least 3 artifacts (one per platform)
- Delete the test tag and draft release after validation
- `actionlint` passes on both workflow files

## Master success criteria for Plan 3

- All 13 phase branches merged to `product/personal`
- Personal app builds + packages across Mac/Win/Linux
- Compat tests pass against Android v0.1.1 and Mac Swift v0.1.1 (mesh treats them as peers)
- Tag `personal-v0.1.0` on `product/personal`, push tag to origin
- Draft GitHub release (named `Personal v0.1.0`, NOT touching the public Swift+Kotlin v0.1.x release naming) with all installer artifacts
- `main` branch unchanged throughout the plan
- A first-time user can install + pair + sync a clipboard event in under 90 seconds following the user guide
