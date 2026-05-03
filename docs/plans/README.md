# Plans Directory — ClipSync Rust Evolution

This directory contains the three-product roadmap for ClipSync's Rust line of work.

## Context

After v0.1.0 of the native Mac+Android product (branch `main`), the project splits into three products that share a Rust core:

| Product | Branch | UI | Topology | Audience |
|---------|--------|----|----------|----------|
| Native (legacy/maintained) | `main` | Swift + Kotlin | star (Mac=server, Android=client) | personal mobile-first |
| Personal (new, Rust) | `product/personal` | Tauri + React (warm palette) | mesh any-to-any | regular user, 2–3 PCs + phone |
| Enterprise (new, Rust) | `product/enterprise` | Tauri + React (admin/serious) | dedicated server + N clients | office, granular control |

Mobile (Android) stays Kotlin-native and works as client to all three topologies.
Mac stays Swift-native for the personal product on `main`. Mac users who want enterprise install the Tauri enterprise client alongside.

## Execution order

1. **Plan 1** — resync `dev` Rust with `main` protocol fixes + workspace split + protocol extensibility hook. Tag `dev-v0.2-ready-for-fork`.
2. After Plan 1: `archive/native-mac-android-v0.1` branch created from `main` as immutable snapshot.
3. **Plans 2 & 3 in parallel** off the post-fork tag. Both branches rebase daily on local `dev` for shared-crate updates.

## CI architecture

The repository uses **per-product workflows** to keep each push lean and prevent the wrong CI from firing on a Rust branch:

| Workflow file | Active branches | Job content |
|---------------|-----------------|-------------|
| `ci-mac-android.yml` | `main` only | Mac build (arm64) + Android compileDebug — current `ci.yml` renamed and scoped |
| `ci-rust-core.yml` | `dev`, `fix/rust-**`, `feat/rust-**`, `chore/rust-**`, `test/rust-**`, `docs/rust-**` | Linux `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest run` |
| `ci-enterprise.yml` | `product/enterprise`, `feat/enterprise-**`, `chore/enterprise-**`, `test/enterprise-**`, `docs/enterprise-**` | Linux check + frontend lint/typecheck/build (Tauri bundle gated to tag pushes via `release-enterprise.yml`) |
| `ci-personal.yml` | `product/personal`, `feat/personal-**`, `chore/personal-**`, `test/personal-**`, `docs/personal-**` | Same shape as enterprise CI |
| `release-enterprise.yml` | tag push `enterprise-v*` | Win MSI + Mac DMG + Linux AppImage/deb/rpm bundling, draft GitHub release upload |
| `release-personal.yml` | tag push `personal-v*` | Win MSI + Mac DMG + Linux AppImage/deb bundling, draft GitHub release upload |

Common speed boosts across all Rust workflows:
- `Swatinem/rust-cache@v2` — caches `target/` and `~/.cargo/registry`
- `cargo nextest` instead of `cargo test` (3–5× faster, fail-fast)
- Linux runner default for check/test (Mac/Win runners are 4× slower per minute and reserved for tag-driven release bundling)
- `concurrency: cancel-in-progress: true` per branch
- `paths-ignore: ['**.md', 'docs/**', '**/screenshots/**', '*.png']`
- Tauri bundling matrix only on tag pushes (`release-*` workflows), not on every commit

The bootstrap of all `ci-*.yml` files happens in **Plan 1 Phase 1.1** before any Rust code change is pushed. That guarantees `dev` and its descendants never trigger the legacy mac+android workflow. Phases 2.16 and 3.13 enrich enterprise/personal with their Tauri build matrices.

## Branch protection — `main` is sacred

Branch `main` holds the public Swift + Kotlin v0.1.x product. None of the three plans is allowed to commit to, merge into, push to, or in any way mutate `main`. The only acceptable read-only operation against `main` is creating the immutable archive branch in Plan 1 phase 1.0.

All other branches (`dev`, `archive/native-mac-android-v0.1`, `product/enterprise`, `product/personal`, and any phase branches) **can be pushed to the remote normally**. Pushes are encouraged for backup and so Plan 2 / Plan 3 sessions can rebase against the latest shared `dev`. Both Plan 2 and Plan 3 boss sessions operate on the same repo and exchange shared-crate updates through `dev` via push + fetch.

Tags and GitHub releases for `enterprise-v0.X.Y` and `personal-v0.X.Y` are allowed. Tags or releases referencing `main` (or v0.1.x of the public Swift+Kotlin product) must NOT be created by these plans.

## Files

- `plan-1-rust-dev-resync.md` — port main's protocol/UX/security fixes into Rust + workspace restructure
- `plan-2-enterprise.md` — Rust enterprise product with dashboard, policy, broadcast
- `plan-3-personal.md` — Rust personal mesh product
- `launch-1-rust-dev-resync.md` — launch prompt for Plan 1 hierarchical execution
- `launch-2-enterprise.md` — launch prompt for Plan 2 hierarchical execution
- `launch-3-personal.md` — launch prompt for Plan 3 hierarchical execution

## Hierarchy used in launch prompts

```
Opus boss (orchestrator, holds plan + master state)
    │
    └── spawns one phase at a time:
        Opus supervisor (phase coordinator, validates worker output)
            │
            └── spawns N parallel:
                Sonnet workers (atomic tasks: one file, one test, one feature slice)
```

Token-optimization rules baked into the launch prompts:
- Boss reads plan file once; subsequent prompts to supervisor reference phase by ID only
- Supervisor receives only its phase block, never full plan
- Workers receive only their atomic task description + relevant file paths
- Workers return diffs / changed file paths only, no prose explanation
- Supervisor consolidates into a phase-completion report (≤200 words) for the boss
- Boss validates the report against success criteria, merges branch, proceeds

## Branch hygiene rules (apply to all plans)

- Each phase = one branch off the plan's base branch
- Branch name = `<type>/<plan-prefix>-<phase-id>-<short-topic>` (kebab-case, English)
- Commit format: `type[scope]: message` per CLAUDE.md
- Phase merges to base via `--no-ff` after supervisor sign-off
- Tests must pass before merge (boss verifies with `cargo test --workspace` or equivalent)
- No phase advances if a previous phase failed
- **Never commit to or merge into `main`** — `main` is the public Swift + Kotlin product and is off-limits to all three plans
- Pushes to remote are allowed (and encouraged) for every branch except `main`
