# Plan 1 — Rust `dev` Resync + Workspace Restructure

## Goal

Bring the Rust workspace on branch `dev` (already at phase-6 of the original pipeline) up to parity with the protocol/UX/security fixes that landed on `main` for the Swift + Kotlin native apps between phase-6 and v0.1.1. Then restructure the workspace into granular shared crates and add forward-compat fields to the wire protocol so Plans 2 and 3 can fork in parallel without conflict.

End state: tag `dev-v0.2-ready-for-fork` on `dev`. Both `product/enterprise` and `product/personal` will branch from this tag.

## Base branch

`dev` (currently at `fa3f071e feat[phase-6]: merge compatibility testing & hardening`).

Before starting, also create `archive/native-mac-android-v0.1` from `main` as immutable snapshot of the current Swift + Kotlin v0.1.1 product.

## Source-of-truth fixes to port

These commits on `main` modify the wire protocol or server semantics. The Rust server in `crates/clipsync-server` and core in `crates/clipsync-core` lag behind:

| Phase | Source commit(s) on `main` | Topic |
|-------|---------------------------|-------|
| 1.2 | `f615624e`, `cb27b4cb`, `62ad7bc6` (test) | Timestamps in milliseconds for `ClipPayload.ts`; HMAC `t=` header stays in seconds |
| 1.3 | `4f8ab8de` | `/inject` returns HTTP 400 with structured error body on decode/validate failure (not 500) |
| 1.4 | `e2cb5451` | Move rate limiter to pre-auth middleware so it fires before 401 |
| 1.5 | `62ad7bc6`, `b3cc3159` | Pairing 401 body carries specific reason: `invalid` / `expired` / `consumed` / `notStarted` |
| 1.6 | `de5ec40c` | Detect port-in-use on startup, return specific error rather than crash |
| 1.7 | `fc9b1d38` | Connection tuning: ping interval 5s, read timeout 15s, health check 10s, failure threshold 2 |
| 1.8 | `4c8c285c` | (Mac CLI) detect `CLIError` token in stdout for correct daemonDown state — not applicable to Rust binary, but document for compat tests |

Compat test golden vectors must be regenerated against the Mac Swift implementation post-fix.

## Phases (one branch per phase)

All branches off `dev`. Each merges back via `--no-ff` after supervisor sign-off.

### Phase 1.0 — `chore/archive-native-mac-android-v0.1`

- Create branch `archive/native-mac-android-v0.1` from `main` HEAD as immutable snapshot
- Push to origin
- Do NOT commit to or merge into `main` itself; this phase only branches off `main` to create a sibling archive branch
- Document the archive in `docs/plans/README.md` (already documented)
- No code changes
- Success: `git branch -a` shows the archive branch on origin

### Phase 1.1 — `chore/ci-split-workflows`

**MUST run before any other phase that pushes Rust changes**, otherwise the existing `.github/workflows/ci.yml` on `dev` will fire on every push and fail (it expects `mac/`+`android/` source).

Files (touch on `dev` only — `main` and its workflow stay untouched):
- DELETE `.github/workflows/ci.yml` from `dev` (this leaves `main`'s copy intact since workflows are per-branch)
- CREATE `.github/workflows/ci-mac-android.yml` on `dev` — same job content as current `ci.yml`, but `on:` block scoped to:
  ```yaml
  on:
    push:
      branches: [main]
      paths: ['mac/**', 'android/**', '.github/workflows/ci-mac-android.yml']
    pull_request:
      branches: [main]
      paths: ['mac/**', 'android/**', '.github/workflows/ci-mac-android.yml']
  ```
  (This file lives on `dev` so it is inherited by `product/enterprise` and `product/personal`. It will only fire when those branches push to `main`, which they never do — effectively making it dormant on Rust branches.)
- CREATE `.github/workflows/ci-rust-core.yml` on `dev` — Linux-only `cargo check`, `cargo nextest`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`. Triggers:
  ```yaml
  on:
    push:
      branches:
        - dev
        - 'fix/rust-**'
        - 'feat/rust-**'
        - 'chore/rust-**'
        - 'test/rust-**'
        - 'docs/rust-**'
      paths:
        - 'rust/**'
        - '.github/workflows/ci-rust-core.yml'
    pull_request:
      branches: [dev]
  concurrency:
    group: ci-rust-core-${{ github.ref }}
    cancel-in-progress: true
  ```
- CREATE `.github/workflows/ci-enterprise.yml` on `dev` — stub workflow that triggers on `product/enterprise` and `feat/enterprise-**` etc. Initial content: same as `ci-rust-core` (Linux check + nextest + clippy). Tauri build matrix added later in Plan 2 Phase 2.16.
  ```yaml
  on:
    push:
      branches:
        - product/enterprise
        - 'feat/enterprise-**'
        - 'chore/enterprise-**'
        - 'test/enterprise-**'
        - 'docs/enterprise-**'
      paths:
        - 'rust/**'
        - '.github/workflows/ci-enterprise.yml'
    pull_request:
      branches: [product/enterprise]
  ```
- CREATE `.github/workflows/ci-personal.yml` on `dev` — analogous stub for `product/personal` and `feat/personal-**` etc.

Speed boosts (apply to all four new workflows):
- `Swatinem/rust-cache@v2` action (caches `target/` + `~/.cargo/registry`)
- `taiki-e/install-action@v2` to install `cargo-nextest`
- `cargo nextest run --workspace` instead of `cargo test --workspace` (3–5× faster)
- `paths-ignore: ['**.md', 'docs/**', '**/screenshots/**', '*.png']` at workflow level
- `concurrency: { group: <workflow>-${{ github.ref }}, cancel-in-progress: true }` for branch-level run cancellation
- Linux runner only (`ubuntu-latest`) for check/test — Mac/Win runners reserved for Tauri tag builds in later phases

Rules:
- Do NOT touch `main`. The current `ci.yml` on `main` continues serving Swift+Kotlin pushes
- The deletion of `ci.yml` only happens on `dev`'s working tree — the `main` HEAD's `ci.yml` is unaffected (per-branch file content)
- Document the workflow architecture in `docs/ci-architecture.md`

Success criteria:
- Push a no-op commit to `dev` → only `ci-rust-core.yml` fires, completes Linux check in <3 minutes (cold) or <1 minute (warm cache)
- Push a no-op commit to `main` (manually if needed for verification, otherwise wait for next legitimate `main` push) → only `ci-mac-android.yml` fires
- `ci-enterprise.yml` and `ci-personal.yml` do NOT fire on `dev` pushes (their branches don't match)
- All four workflow files YAML-valid (`actionlint` clean)

### Phase 1.2 — `fix/rust-protocol-timestamps`

Files:
- `rust/crates/clipsync-core/src/payload.rs` (or equivalent: where `ClipPayload` is defined and validated)
- `rust/crates/clipsync-core/src/hmac.rs` (where HMAC header is built/validated)
- `rust/crates/clipsync-server/src/...` (uses of `Date::now()` / `Instant::now()` against payload `ts`)
- `rust/crates/clipsync-client/src/...` (when sending: build `ts = unix_millis()`)
- All affected tests

Rules:
- `ClipPayload.ts` field = unix epoch in **milliseconds** (`SystemTime::now().duration_since(UNIX_EPOCH).as_millis() as i64`)
- Server validates `(now_ms - ts).abs() < 5*60*1000` (5 minute window in ms)
- HMAC header `X-ClipSync-Signature` `t=` parameter = unix **seconds** (`as_secs()`)
- HMAC validator skew = 60 seconds (matches Mac `HMACValidator.swift`)
- Add explicit doc-comments on the two timestamp call sites referencing `CLAUDE.md` §"Wire Protocol Invariants"

Success criteria:
- Unit tests cover ms vs sec confusion (a payload with `ts` in seconds must fail validation)
- Cross-test golden vector `payload_v1.json` decodes against Mac Swift `ClipPayload.swift`

### Phase 1.3 — `fix/rust-inject-error-mapping`

Files:
- `rust/crates/clipsync-server/src/routes/inject.rs` (or wherever `/inject` handler lives)

Rules:
- Wrap JSON decode + payload validation in a result; on error return HTTP 400 with body:
  ```json
  { "error": "<machine-code>", "message": "<human-readable>" }
  ```
  Codes: `decode_error`, `timestamp_out_of_range`, `payload_too_large`, `unsupported_kind`
- Propagate other errors as 500 only when truly unexpected
- Match Hummingbird's `HTTPError(.badRequest, ...)` shape on Mac for parity

Success criteria:
- Integration test: malformed JSON → 400 with `decode_error`
- Integration test: stale timestamp → 400 with `timestamp_out_of_range`
- Existing tests still pass

### Phase 1.4 — `fix/rust-rate-limit-preauth`

Files:
- `rust/crates/clipsync-server/src/middleware/` (rate limiter + auth middleware order)
- Server router registration

Rules:
- Rate limiter runs before auth middleware so an attacker spamming bad tokens is throttled before reaching auth
- Limit: 30 req/min per remote IP for `/inject`, configurable
- 429 response with `Retry-After` header

Success criteria:
- Test: 31 unauth'd requests within 60s → last is 429 (not 401)
- Test: rate limit per-IP, not global

### Phase 1.5 — `fix/rust-pairing-error-bodies`

Files:
- `rust/crates/clipsync-server/src/routes/pair.rs` (pairing endpoints)
- Pairing manager state machine

Rules:
- 401 responses carry body `{ "error": "<code>" }` with codes:
  - `invalid` — wrong code
  - `expired` — TTL elapsed (5 min, see Mac `PairingManager.swift`)
  - `consumed` — already used
  - `notStarted` — pairing not initiated
- Match Mac error vocabulary so Android client (and future Tauri client) parses identically

Success criteria:
- Tests cover all four error paths
- Android `PairingApi.kt` parser test (compat test) passes against Rust server

### Phase 1.6 — `fix/rust-port-in-use-detection`

Files:
- `rust/crates/clipsync-server/src/main.rs` (or server bootstrap)

Rules:
- Before binding, attempt a probe `TcpListener::bind` with the configured port
- On `AddrInUse` error, log `port {port} is already in use; another ClipSync instance may be running` and exit code 2
- Do not panic with the generic Tokio bind error

Success criteria:
- Manual test: launch two server binaries on same port → second exits cleanly with the specific message
- Integration test using `tokio::net::TcpListener` to occupy port

### Phase 1.7 — `fix/rust-connection-tuning`

Files:
- `rust/crates/clipsync-server/src/ws.rs` (WebSocket hub)
- `rust/crates/clipsync-client/src/connection.rs` (client connection + reconnect)
- Any healthcheck/ping module

Rules (match `fc9b1d38`):
- WS ping interval: 5s
- Read timeout: 15s
- Healthcheck endpoint poll interval: 10s
- Consecutive failures before declaring disconnect: 2

Success criteria:
- Constants extracted into `clipsync-core::config` so both server and client share them
- Tests confirm a stalled WS connection triggers reconnect within 25s (worst case 2× read timeout + jitter)

### Phase 1.8 — `test/rust-compat-vectors-update`

Files:
- `rust/tests/compat/` (existing golden vector dir from phase 6)
- Vector files: payload, HMAC header, pairing error body

Rules:
- Regenerate golden vectors using the post-fix Mac Swift implementation as canonical source
- Add three new vectors:
  - `payload_v1_ms.json` — payload with millisecond ts
  - `pairing_error_invalid.json` — 401 body
  - `inject_400_decode.json` — 400 body
- Cross-platform interop matrix in `docs/cross-platform-interop.md` updated

Success criteria:
- `cargo test --test compat` passes
- Each vector has a comment header pointing to the Mac/Android source file it was captured from

### Phase 1.9 — `chore/rust-workspace-split`

Files:
- `rust/Cargo.toml` (workspace members)
- New crate scaffolding under `rust/crates/`
- Move existing code from `clipsync-core` into focused crates

Restructure:
```
rust/
  Cargo.toml                       # workspace root
  crates/
    clipsync-protocol/             # wire format types: ClipPayload, frames, error codes
    clipsync-crypto/               # HMAC, TLS identity, fingerprint
    clipsync-transport/            # WebSocket hub, mDNS discovery, reachability
    clipsync-clipboard/            # arboard wrapper + per-OS quirks
    clipsync-platform/             # tray, notif, autostart, hotkeys per OS (cfg-gated)
  legacy/                          # temporary: clipsync-core, clipsync-server, clipsync-client
                                   # kept until apps/ shells exist
```

Rules:
- Move types and code into the new crates without behavioral change
- Old crates `clipsync-core/server/client` become thin re-exports during this phase
- Cargo workspace builds; all existing tests still pass
- No new features; pure restructure

Success criteria:
- `cargo build --workspace` clean
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` clean
- Each new crate has a 1-paragraph `README.md` explaining its purpose

### Phase 1.10 — `chore/rust-protocol-extensible`

Files:
- `rust/crates/clipsync-protocol/src/payload.rs`
- `rust/crates/clipsync-protocol/src/handshake.rs` (new file if needed)

Rules — additive only, backward-compatible:
- `ClipPayload` gains optional fields (with `#[serde(default, skip_serializing_if = "Option::is_none")]`):
  - `policy: Option<PolicyHints>` — for enterprise broadcast filtering (struct kept private; flag-only fields acceptable)
  - `origin_role: Option<DeviceRole>` — `server` / `client` / `peer`
- New handshake message type for WS opening frame carrying:
  - `device_id`, `role`, `capabilities` (`["broadcast", "policy", "audit"]` etc.)
- Enterprise will populate; personal will leave as `None`
- All existing payloads still round-trip without these fields

Success criteria:
- Round-trip tests: old payload → encode → decode → equals
- Tests confirm Rust ↔ Mac Swift payloads (without these fields) still pass golden vector checks

### Phase 1.11 — `chore/rust-dev-tag-and-doc`

Files:
- `docs/phase-N-summary.md` style summary at `docs/phase-rust-resync-summary.md`
- `rust/PHASE_PROGRESS.md` updated
- Tag

Rules:
- Write summary doc following the pattern of existing phase summaries
- Run full workspace build, tests, clippy
- Tag `dev-v0.2-ready-for-fork` on `dev`
- Push tag to origin

Success criteria:
- Tag visible on origin via `git ls-remote --tags origin`
- Summary doc lists every merged phase branch with one-line description
- `cargo test --workspace --all-features` passes on Mac+Linux CI

## Master success criteria for Plan 1

- All 12 phase branches (1.0 through 1.11) merged to `dev`
- Tag `dev-v0.2-ready-for-fork` exists
- Cross-platform interop matrix passes against latest Mac Swift v0.1.1 binary and Android v0.1.1 APK
- Branch `archive/native-mac-android-v0.1` exists on origin
- `main` is unchanged (no commits, no merges, no force-push)
- `dev` Rust server can pair with Android v0.1.1 (test on real device or via test harness using golden requests)
