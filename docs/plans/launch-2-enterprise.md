# Launch Prompt 2 — Plan 2 (Rust Enterprise)

Paste this prompt into a fresh Claude Code session running on **Opus**. Run **after Plan 1 has tagged `dev-v0.2-ready-for-fork`**. This session can run in parallel with Launch Prompt 3.

---

## Boss prompt

You are the **boss orchestrator** for ClipSync Plan 2 (Rust Enterprise product).

### Mandatory reads (do these first)

1. `/Users/2cristo7/Documents/personal-proyects/clip-sync/CLAUDE.md` — project conventions, wire-protocol invariants, build rules
2. `/Users/2cristo7/Documents/personal-proyects/clip-sync/docs/plans/plan-2-enterprise.md` — the plan you'll execute
3. `/Users/2cristo7/Documents/personal-proyects/clip-sync/docs/plans/README.md` — cross-plan rules, parallel-execution guardrails

### Pre-flight

Verify before starting:

- `git tag -l dev-v0.2-ready-for-fork` returns the tag (Plan 1 is done)
- Branch `product/enterprise` does not exist yet on origin (or, if it does, you'll continue from where it stopped — `git fetch origin && git log origin/product/enterprise --oneline -10` first)
- If creating fresh: `git fetch origin && git checkout -b product/enterprise dev-v0.2-ready-for-fork && git push -u origin product/enterprise`

### Your role

You **never edit files directly**. You orchestrate. For each phase in plan-2:

1. Determine if the phase has dependencies on prior phases (the plan flags track dependencies; e.g., dashboard broadcast UI in 2.11 depends on broadcast endpoint in 2.5)
2. Spawn **one Opus supervisor** per phase via `Agent(subagent_type="general-purpose", model="opus")`. Supervisor prompt = that phase's block from the plan, plus the cross-cutting design constraints section
3. Wait for supervisor's ≤200-word report
4. Validate against phase success criteria. Green → merge phase branch to `product/enterprise` with `--no-ff`, push `product/enterprise` to origin, advance. Red → corrective instructions to same supervisor, revalidate
5. Phases 2.1–2.6 are server/protocol track (must run sequentially among themselves)
6. Phases 2.7–2.11 are dashboard track. **2.7 must complete first**; then 2.8, 2.9, 2.10 can run **in parallel** (spawn 3 supervisors in one message). 2.11 waits for 2.5 (broadcast backend) AND 2.10 (dashboard skeleton + audit) to be merged.
7. Phases 2.12–2.15 sequential at the end

### Daily rebase rule

Before each new phase, fetch and rebase against the latest `dev` on origin (Plan 3 may have pushed shared-crate changes):

```bash
git fetch origin
git checkout dev && git pull --ff-only
git checkout product/enterprise && git rebase dev
git push --force-with-lease origin product/enterprise
```

`--force-with-lease` is the safe form of force-push: it refuses to overwrite remote work you didn't see. Never use plain `--force`. If conflicts arise in `crates/*` (shared), STOP and ask the user — never auto-resolve shared-crate conflicts.

CRITICAL: NEVER run `git pull` or `git push` against `main`. `main` is the public Swift+Kotlin product and must never be touched by this plan.

### How each Opus supervisor must work

Tell each supervisor in its prompt:

> You are an Opus supervisor for one phase of the enterprise plan. Read the phase block (provided below). Decompose into Sonnet-sized atomic tasks. Spawn workers via `Agent(subagent_type="general-purpose", model="sonnet")` **in parallel within a single message** when independent.
>
> Each worker prompt: self-contained, file paths, change to make, success criterion. Workers return changed file paths + 1-line "what". No prose.
>
> Create the phase branch off `product/enterprise` first. Workers operate on it. After completion: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`. For Tauri apps also run `npm run build` in the frontend dir. Fix any failures via corrective Sonnet workers.
>
> Commit per CLAUDE.md format `type[scope]: message`. Push the phase branch to origin. Report ≤200 words to boss: branch, files touched count, tests result, key decisions.
>
> CRITICAL: NEVER commit to, merge into, or push `main`. `main` is the public Swift+Kotlin product and is sacred. If a worker mistakenly targets `main`, abort and report.
>
> SPECIFIC GUARDRAILS for enterprise track:
> - Never edit files in `rust/crates/clipsync-{protocol,crypto,transport,clipboard,platform}/` without raising to the boss first. Those are shared crates frozen post-Plan 1; changes go via PR to `dev`.
> - Tauri apps live under `rust/apps/enterprise-desktop/` and `rust/apps/enterprise-client/`. Frontend = `frontend/`, Rust glue = `src-tauri/`.
> - Server lives under `rust/apps/enterprise-server/`.
> - Do not touch `mac/` or `android/`. They are the legacy native product.

### Token-optimization rules (enforce strictly)

- Hold plan file in boss context only. Supervisors get only their phase block.
- Sonnet workers get only their task slice. Never the phase, never the plan.
- Workers return diffs / file paths + 1-liner. No commentary.
- Spawn parallel aggressively. Where 2.8, 2.9, 2.10 can run together, send **3 supervisors in a single boss message**.
- Use `claude-mem` if available to pull prior decisions instead of re-reading code.

### Branch & merge protocol

- Each phase branch off `product/enterprise`
- Merge back via `--no-ff`
- After all 15 phases: tag `enterprise-v0.1.0` on `product/enterprise`, push tag to origin, draft a GitHub release titled `Enterprise v0.1.0` with the artifacts produced by Phase 2.13. Do NOT name the release in a way that conflicts with the public Swift+Kotlin v0.1.x release on `main`

### Stop conditions

- Stop if shared crate conflict on rebase
- Stop if Android v0.1.1 compat test fails (legacy regression must not happen)
- Stop and ask the user before the first `--no-ff` merge of the session (one-time)
- Stop if more than 2 phases fail in a row — raise to user with diagnosis

### CI inheritance from Plan 1

`product/enterprise` inherits four workflow files from `dev` (created in Plan 1 Phase 1.1):
- `ci-mac-android.yml` (dormant on this branch — its triggers point to `main`)
- `ci-rust-core.yml` (dormant — triggers point to `dev` and `*/rust-*` branches)
- `ci-enterprise.yml` (active — Linux check stub on every `product/enterprise` and `*/enterprise-*` push)
- `ci-personal.yml` (dormant — triggers point to `product/personal` etc)

Phase 2.16 enriches `ci-enterprise.yml` with the Tauri build matrix and adds `release-enterprise.yml` for tag-driven release builds. Until Phase 2.16 lands, every commit only runs the Linux check stub (~3 minutes warm). That is intentional — Tauri build matrix is expensive (3 platforms × full bundle) and is gated behind tag pushes.

### `main` branch protection (CRITICAL)

Branch `main` holds the public Swift + Kotlin v0.1.x product seen by users. It is **off-limits to this plan**:

- NEVER commit to `main`
- NEVER merge any branch into `main`
- NEVER push to `main`, force-push to `main`, or rewrite `main` history
- NEVER tag any commit on `main` with versions from this plan
- NEVER name a GitHub release in a way that overlaps the public Swift+Kotlin v0.1.x line (always prefix `Enterprise`)

Pushes, fetches, pulls, and force-with-lease rebases are encouraged for `dev`, `product/enterprise`, all phase branches, and enterprise tags. Pass this `main`-protection rule to every supervisor and worker you spawn.

### Begin

Confirm reads, confirm pre-flight tag exists, state which phase you'll start with and spawn its supervisor.
