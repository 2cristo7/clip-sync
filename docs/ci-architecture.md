# CI Architecture

ClipSync uses **per-product GitHub Actions workflows** so that each push only triggers the CI relevant to the branch's product line. This document describes which workflow fires on which branch and why.

## Why per-product workflows

After v0.1.0, the repository forks into three product lines that share a Rust core:

| Product | Branch | Stack |
|---------|--------|-------|
| Native (legacy/maintained) | `main` | Swift + Kotlin |
| Personal (new, Rust mesh) | `product/personal` | Tauri + React + Rust |
| Enterprise (new, Rust dedicated server) | `product/enterprise` | Tauri + React + Rust |

A single monolithic CI would either:
- run Swift/Android jobs on Rust pushes (waste minutes, fail because `mac/`+`android/` may be removed in product branches), or
- run Rust jobs on Swift/Android pushes (waste minutes on the legacy product).

Splitting by branch + path scopes each workflow to exactly the product it serves.

## Workflow matrix

| Workflow file | Active branches | Path filter | Job content |
|---------------|-----------------|-------------|-------------|
| `ci-mac-android.yml` | `main` only | `mac/**`, `android/**` | macOS `xcodebuild test` + Android `assembleDebug` / `testDebugUnitTest` / `lintDebug` |
| `ci-rust-core.yml` | `dev`, `fix/rust-**`, `feat/rust-**`, `chore/rust-**`, `test/rust-**`, `docs/rust-**` | `rust/**` | Linux `cargo fmt --check`, `cargo check`, `cargo clippy -D warnings`, `cargo nextest run --workspace` |
| `ci-enterprise.yml` | `product/enterprise`, `feat/enterprise-**`, `chore/enterprise-**`, `test/enterprise-**`, `docs/enterprise-**` | `rust/**` | Same as `ci-rust-core` initially. Tauri build matrix added in Plan 2 phase 2.16. |
| `ci-personal.yml` | `product/personal`, `feat/personal-**`, `chore/personal-**`, `test/personal-**`, `docs/personal-**` | `rust/**` | Same as `ci-rust-core` initially. Tauri build matrix added in Plan 3 phase 3.13. |

Future workflows (created in later phases):

| Workflow file | Trigger | Job content |
|---------------|---------|-------------|
| `release-enterprise.yml` | tag push `enterprise-v*` | Win MSI + Mac DMG + Linux AppImage/deb/rpm bundling, draft GitHub release upload |
| `release-personal.yml` | tag push `personal-v*` | Win MSI + Mac DMG + Linux AppImage/deb bundling, draft GitHub release upload |

## Branch isolation guarantees

- `ci-mac-android.yml` fires only on `main` push/PR — Rust branches never trigger it.
- `ci-rust-core.yml` fires on `dev` and `*-rust-**` branches only — `main` never triggers it.
- `ci-enterprise.yml` and `ci-personal.yml` are inherited on `dev` (because `dev` is the merge base of every product branch) but their `branches:` lists exclude `dev` and rust-only patterns, so they stay dormant until product branches are created in Plans 2 / 3.
- Per-branch file content of `.github/workflows/` means deleting `ci.yml` on `dev` does **not** affect `main`'s `ci.yml`. The Swift+Kotlin product on `main` keeps its original CI.

## Speed boosts (applied to every Rust workflow)

- `Swatinem/rust-cache@v2` — caches `target/` and `~/.cargo/registry`, ~3× faster on cache hit.
- `taiki-e/install-action@v2` to install `cargo-nextest` from prebuilt binaries (no `cargo install` compile cost).
- `cargo nextest run --workspace` instead of `cargo test --workspace` — 3–5× faster, fail-fast, better stdout.
- `paths-ignore: ['**.md', 'docs/**', '**/screenshots/**', '*.png']` to skip docs-only commits.
- `concurrency: { group: <workflow>-${{ github.ref }}, cancel-in-progress: true }` cancels stale runs when a new commit lands on the same branch.
- Linux runner only (`ubuntu-latest`) for check/test. Mac/Win runners are 4× slower per minute and reserved for tag-driven release bundling (Plan 2 / Plan 3).

## Bootstrap context

This split was bootstrapped in **Plan 1 Phase 1.1** before any Rust code change was pushed. That guarantees `dev` and its descendants never trigger the legacy mac+android workflow. Phases 2.16 and 3.13 will enrich `ci-enterprise.yml` and `ci-personal.yml` with their Tauri build matrices and add the corresponding `release-*.yml` files.

See `docs/plans/README.md` for the full multi-product roadmap.
