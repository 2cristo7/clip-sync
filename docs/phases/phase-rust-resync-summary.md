# Phase Rust Dev Resync (Plan 1) — Summary

**Branch base**: `dev` (forked from `main` after Phase 0 in the original Rust pipeline).
**Tag**: `dev-v0.2-ready-for-fork` placed on `dev` HEAD after this phase merges.
**Archive**: `archive/native-mac-android-v0.1` at `7b59027f` preserves the Swift / Kotlin v0.1.0 lineage. Plans 2 and 3 fork from `dev-v0.2-ready-for-fork`.
**Cross-platform interop reference**: `rust/tests/INTEROP_MATRIX.md`.

## Goal

Re-sync the Rust workspace on `dev` with the protocol corrections, server hardening, CI improvements, and protocol-extensibility work that landed after the original `v0.1.0` (Phases 0–6) tag. This plan ran as eleven incremental phases (1.0 through 1.10), each merged into `dev` with `--no-ff` after passing `ci-rust-core`. Phase 1.11 (this doc + tag) closes the plan.

## Phases

### Phase 1.0 — Archive native v0.1 lineage

- **Branch**: `archive/native-mac-android-v0.1` (sibling, not merged to `dev`).
- **Tip**: `7b59027f` (`chore[ci]: restore DerivedData cache keyed on pbxproj and swift files`).
- Snapshots the Swift menu-bar app + Kotlin Android client at the v0.1.0 line. Read-only reference for Plans 2/3.

### Phase 1.1 — CI split workflows

- **Merge**: `e5f71f7a chore[ci]: merge ci split workflows (phase 1.1)`.
- **Follow-ups merged on `dev`**: `917694bb fix[ci]: merge paths-ignore conflict fix`, `5bd51280 chore[rust]: merge cargo fmt cleanup`, `1238c7bc fix[rust]: clippy 1.95 — const block asserts, drop needless borrows, unused import`, `f7fd2ac1 fix[ci]: add libxdo-dev for clipsync-client linker on Linux`.
- Splits the original monolithic `ci.yml` into `ci-rust-core.yml`, `ci-android.yml`, `ci-mac.yml`. Each workflow filters on its own paths so Rust changes no longer fan out to the iOS/Android jobs and vice versa. Required for the cadence of the rest of Plan 1.

### Phase 1.2 — Protocol timestamps fix

- **Merge**: `c0442016 fix[rust-protocol]: merge timestamps fix (phase 1.2)`.
- Aligns `ClipPayload.ts` to milliseconds (matching the Mac canonical wire) while keeping the HMAC header `t=` in seconds. Adds doc-comments and tests guarding against the two-units invariant documented in `CLAUDE.md`.

### Phase 1.3 — Inject error mapping

- **Merge**: `98c09eb0 fix[rust-server]: merge inject error mapping (phase 1.3)`.
- `/inject` now returns structured `400 {"error":"decode_error","message":"..."}` bodies on bad JSON / failed payload validation instead of bubbling Hummingbird/axum 500s. Compat vector `inject_400_decode.json` added.

### Phase 1.4 — Rate limit pre-auth

- **Merge**: `aefdd234 fix[rust-server]: merge rate limit pre-auth (phase 1.4)`.
- Token-bucket rate limiter is now applied before the auth middleware on `/pair` and `/inject`, so unauthenticated floods cannot consume HMAC-validation CPU. New tests in `clipsync-server` integration suite.

### Phase 1.5 — Pairing 401 error bodies

- **Merge**: `c10a16f3 fix[rust-pairing]: merge pairing 401 error bodies (phase 1.5)`.
- `/pair` now returns `401 {"error":"invalid"}` (and friends) with consistent bodies. Compat vector `pairing_error_invalid.json` codifies the shape so Plan 2 / Plan 3 clients can match on field shape rather than HTTP status alone.

### Phase 1.6 — Port-in-use detection

- **Merge**: `03e1753c fix[rust-server]: merge port-in-use detection (phase 1.6)`.
- Server bind detects `EADDRINUSE` early and prints an actionable error with the offending port instead of panicking from inside the runtime.

### Phase 1.7 — Connection tuning

- **Merge**: `fa9ced71 fix[rust-conn]: merge connection tuning (phase 1.7)`.
- Tunes the WebSocket client reconnect ladder, ping/pong timing, and HTTP keep-alives to match the behaviour the Swift/Kotlin clients had been depending on. Reduces the false-disconnect rate over Tailscale.

### Phase 1.8 — Compat vectors update

- **Merge**: `d0ed98c5 test[rust-compat]: merge compat vectors update (phase 1.8)`.
- Refreshes `rust/tests/compat/` golden vectors to cover the post-1.2/1.3/1.5 wire shapes (`payload_v1_ms.json`, error-body fixtures). Conformance tests gate every PR on these vectors so Plan 2/3 forks cannot drift silently.

### Phase 1.9 — Workspace split

- **Merge**: `9367f690 chore[rust-workspace]: merge workspace split (phase 1.9)`.
- Re-organises `rust/crates/` so `clipsync-core`, `clipsync-server`, and `clipsync-client` each own their `Cargo.toml` with explicit feature flags, and the shared `clipsync-protocol` types graduate to their own crate ready to be reused by future fork crates.

### Phase 1.10 — Protocol extensibility

- **Merge**: `20aa476d chore[rust-protocol]: merge protocol extensibility (phase 1.10)`.
- Adds forward-compatible extension points to the wire types: `DeviceRole`, `PolicyHints`, and a versioned `Handshake` envelope. Existing v1 clients are unaffected; future plans can negotiate richer capabilities without breaking the v1 contract.

### Phase 1.11 — Resync tag + summary (this phase)

- **Branch**: `chore/rust-dev-tag-and-doc`.
- Adds this summary and updates `rust/PHASE_PROGRESS.md` with a Plan 1 section. After merge into `dev`, the boss creates and pushes the `dev-v0.2-ready-for-fork` tag on `dev` HEAD.

## Validation

Each phase 1.1 through 1.10 was gated by `ci-rust-core.yml` on the phase branch (Linux runner only — macOS runners are a Plan 2 concern). Locally verified before tagging:

- `cargo fmt --all -- --check` — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo test --workspace` — all suites pass

## Tag

`dev-v0.2-ready-for-fork` is created by the boss agent on `dev` HEAD after this branch merges, then pushed to `origin`. Plans 2 (next-generation Mac client) and 3 (next-generation Android client) branch from this tag.

## Out of scope / Follow-ups

- macOS / Windows CI runners for the Rust workspace (deferred — Linux CI is sufficient for protocol-level guarantees on Plan 1).
- The 2 MB axum `Json` extractor body limit discovered in Phase 6 remains tracked tech debt; not regressed by Plan 1.
