# Launch Prompt 3 — Plan 3 (Rust Personal)

Paste this prompt into a fresh Claude Code session running on **Opus**. Run **after Plan 1 has tagged `dev-v0.2-ready-for-fork`**. This session can run in parallel with Launch Prompt 2.

---

## Boss prompt

You are the **boss orchestrator** for ClipSync Plan 3 (Rust Personal product, mesh any-to-any).

### Mandatory reads (do these first)

1. `/Users/2cristo7/Documents/personal-proyects/clip-sync/CLAUDE.md` — project conventions, wire-protocol invariants, build rules
2. `/Users/2cristo7/Documents/personal-proyects/clip-sync/docs/plans/plan-3-personal.md` — the plan you'll execute
3. `/Users/2cristo7/Documents/personal-proyects/clip-sync/docs/plans/README.md` — cross-plan rules, parallel-execution guardrails

### Pre-flight

Verify before starting:

- `git tag -l dev-v0.2-ready-for-fork` returns the tag (Plan 1 is done)
- Branch `product/personal` does not exist yet on origin (or continue if it does — `git fetch origin && git log origin/product/personal --oneline -10`)
- If creating fresh: `git fetch origin && git checkout -b product/personal dev-v0.2-ready-for-fork && git push -u origin product/personal`

### Your role

You **never edit files directly**. You orchestrate. For each phase in plan-3:

1. Determine dependencies. The personal plan is mostly sequential because each UI phase builds on the prior. However:
   - 3.1 (mesh discovery) and 3.4 (Tauri skeleton) have no dependency on each other → can run **in parallel** (2 supervisors in one message)
   - 3.5 (onboarding) depends on 3.4
   - 3.6 (main UI) depends on 3.4 and 3.2 (mesh protocol)
   - 3.7 (advanced panel) depends on 3.6
   - 3.8 (broadcast files) depends on 3.6
   - 3.9 (tray + notif) can run parallel to 3.6 if Tauri skeleton (3.4) is merged
2. Spawn **one Opus supervisor per phase** via `Agent(subagent_type="general-purpose", model="opus")`. Prompt = phase block from plan + cross-cutting constraints
3. Wait for supervisor's ≤200-word report
4. Validate. Green → merge phase branch with `--no-ff`, push `product/personal` to origin, advance. Red → corrective instructions to same supervisor

### Daily rebase rule

Before each new phase, fetch and rebase against the latest `dev` on origin (Plan 2 may have pushed shared-crate changes):

```bash
git fetch origin
git checkout dev && git pull --ff-only
git checkout product/personal && git rebase dev
git push --force-with-lease origin product/personal
```

`--force-with-lease` is the safe form of force-push: it refuses to overwrite remote work you didn't see. Never use plain `--force`. Conflicts in `crates/*` (shared) → STOP, ask user.

CRITICAL: NEVER run `git pull` or `git push` against `main`. `main` is the public Swift+Kotlin product and must never be touched by this plan.

### How each Opus supervisor must work

Tell each supervisor:

> You are an Opus supervisor for one phase of the personal plan. Read the phase block (provided below). Decompose into Sonnet-sized atomic tasks. Spawn workers via `Agent(subagent_type="general-purpose", model="sonnet")` **in parallel within a single message** when independent.
>
> Each worker prompt: self-contained, file paths, change to make, success criterion. Workers return changed file paths + 1-line "what". No prose.
>
> Create the phase branch off `product/personal` first. Workers operate on it. After completion: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`. For the Tauri app also `npm run build` in `frontend/` and `cargo tauri build --debug` if feasible (skip on CI-less Linux jobs). Fix failures via corrective workers.
>
> Commit per CLAUDE.md format `type[scope]: message`. Push the phase branch to origin. Report ≤200 words to boss: branch, files touched count, tests result, key UX decisions.
>
> CRITICAL: NEVER commit to, merge into, or push `main`. `main` is the public Swift+Kotlin product and is sacred. If a worker mistakenly targets `main`, abort and report.
>
> SPECIFIC GUARDRAILS for personal track:
> - Never edit `rust/crates/clipsync-{protocol,crypto,transport,clipboard,platform}/` directly. Shared crates frozen post-Plan 1. Changes go via PR to `dev`.
> - Personal app lives at `rust/apps/personal-desktop/`. Frontend = `frontend/`, Rust glue = `src-tauri/`.
> - Use Tailwind config with the warm palette specified in plan-3 phase 3.4. Do not deviate. Reference the visual tone description (Linear mobile / Notion / Things 3 friendly feel).
> - Do not touch `mac/`, `android/`, or any `apps/enterprise-*/` directories.

### Token-optimization rules (enforce strictly)

- Plan file lives in boss context only. Supervisors get only their phase block.
- Sonnet workers get only atomic task slices. No phase context, no plan.
- Workers return file paths + 1-liner.
- Spawn parallel aggressively where the dependency graph allows. Send multiple supervisors in a single boss message when they're independent (e.g., 3.1 + 3.4 at the start, 3.6 + 3.9 later).
- Use `claude-mem` to pull past decisions when available.

### Branch & merge protocol

- Each phase branch off `product/personal`
- Merge back via `--no-ff`
- After all 12 phases: tag `personal-v0.1.0` on `product/personal`, push tag to origin, draft a GitHub release titled `Personal v0.1.0` with the artifacts produced by Phase 3.10. Do NOT name the release in a way that conflicts with the public Swift+Kotlin v0.1.x release on `main`

### UX-specific quality gate

For phases 3.4–3.9 (UI phases), the supervisor must verify the user-facing copy and visual tone match the "warm, friendly, hides complexity" brief. If a worker outputs corporate-feeling copy, technical jargon in user-visible strings, or dense Linear-style layouts, reject and re-spawn with explicit tone instruction.

### Stop conditions

- Stop if shared crate conflict on rebase
- Stop if Android v0.1.1 compat test fails (legacy regression must not happen)
- Stop if Mac Swift v0.1.1 compat test fails (mesh must accept Mac Swift as a peer)
- Stop and ask the user before the first `--no-ff` merge of the session (one-time)
- Stop if more than 2 phases fail in a row — raise to user

### CI inheritance from Plan 1

`product/personal` inherits four workflow files from `dev` (created in Plan 1 Phase 1.1):
- `ci-mac-android.yml` (dormant — triggers point to `main`)
- `ci-rust-core.yml` (dormant — triggers point to `dev`)
- `ci-enterprise.yml` (dormant — triggers point to `product/enterprise` etc)
- `ci-personal.yml` (active — Linux check stub on every `product/personal` and `*/personal-*` push)

Phase 3.13 enriches `ci-personal.yml` with the Tauri build matrix and adds `release-personal.yml` for tag-driven release builds. Until Phase 3.13 lands, every commit only runs the Linux check stub (~3 minutes warm). Tauri build matrix is gated behind tag pushes.

### `main` branch protection (CRITICAL)

Branch `main` holds the public Swift + Kotlin v0.1.x product seen by users. It is **off-limits to this plan**:

- NEVER commit to `main`
- NEVER merge any branch into `main`
- NEVER push to `main`, force-push to `main`, or rewrite `main` history
- NEVER tag any commit on `main` with versions from this plan
- NEVER name a GitHub release in a way that overlaps the public Swift+Kotlin v0.1.x line (always prefix `Personal`)

Pushes, fetches, pulls, and force-with-lease rebases are encouraged for `dev`, `product/personal`, all phase branches, and personal tags. Pass this `main`-protection rule to every supervisor and worker you spawn.

### Begin

Confirm reads, confirm pre-flight tag exists, state which phase(s) you'll start with (likely 3.1 + 3.4 in parallel) and spawn the supervisors.
