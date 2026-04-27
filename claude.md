# Git Conventions

## Commits

All commit messages must be in English and follow the Conventional Commits format:

```
type[scope]: message
```

Types allowed: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`.

Example: `feat[mac-network]: add mDNS broadcasting`

## Branches

All branches must follow kebab-case and use the following prefixes:

`feature/`, `fix/`, `hotfix/`, `release/`, `chore/`

Example: `feature/auth-token`

---

# Project State

All phases 0–9 are merged in `main`. Tag `v0.1.0` created. The original pipeline is complete. New work starts directly from `main` or opens a new cycle of phases defined in `master_plan.md`.

## Pipeline (sub-agent phases)

When the user asks to "run Phase N", "start Phase N", or "continue the pipeline", activate the sub-agent master behavior described in `master_plan.md`. For one-off fixes, refactors, or questions, work directly without the pipeline.

The master agent:
1. Reads `master_plan.md` for phase objectives, files, expected commits, and success criteria.
2. Launches sub-agents (`Agent` tool, `subagent_type: "general-purpose"`) with a fully self-contained prompt (the agent has zero context from this conversation).
3. Validates the `---FASE_COMPLETADA---` block from each sub-agent output.
4. Merges to `main` with `--no-ff` after validation (confirm with user the first time per session).
5. Creates `docs/phase-N-summary.md` following the pattern of existing summaries.

**Strict rules:**
- Never reuse a sub-agent across phases.
- Never advance if tests fail.
- Sub-agents never merge to main — the master does.
- The master never writes code directly during the pipeline.

---

# Stack & Targets

- macOS target: **14.0** (Hummingbird 2.x requires macOS 14)
- Android target: **API 33** (Android 13+)
- `Package.resolved` is in `.gitignore` — do not commit it

---

# Source File Structure (v0.1.0 + patches 22 Apr 2026)

**Mac — 17 Swift files** (`mac/ClipSync/`):
`App`, `Clipboard/{ClipPayload,PasteboardInjector,PasteboardWatcher}`,
`Network/{BonjourAdvertiser,ReachabilityMonitor}`,
`Pairing/{PairingManager,TokenStore}`,
`Security/{HMACValidator,TLSManager}`,
`Server/{AuthMiddleware,ClipServer,ServerConfig,WebSocketHub}`,
`Storage/Keychain`, `UI/{MenuBarController,PairingWindow}`

**Android — 22 Kotlin files** (`android/app/src/main/java/com/clipsync/`):
`MainActivity`, `clipboard/ClipboardWriter`, `crypto/{Fingerprint,HmacSigner}`,
`discovery/NsdDiscovery`, `images/ImageCache`,
`model/{ClipPayload,ClipPayloadBuilder}`,
`net/{ClipClient,NetworkChangeObserver,PairingApi}`,
`notifications/{ApplyClipActivity,IncomingClipNotifier}`,
`overlay/{ClipOverlayManager,ClipSender,SendClipActivity}`,
`service/ClipForegroundService`, `storage/Prefs`,
`ui/{SettingsScreen,SettingsViewModel,theme/ClipSyncTheme,theme/NeuComponents}`

---

# Implemented Features

- Real-time WebSocket sync (LAN + Tailscale)
- TOFU pairing with HMAC (timestamp ±60 s); pairing-secret in Keychain (`com.clipsync.pairing-secret`). A new secret invalidates all tokens.
- **Persistent FAB overlay** on Android (`ClipOverlayManager`): floating bubble over any app; tap to push clipboard to Mac via `POST /inject`. Requires `SYSTEM_ALERT_WINDOW`.
- **Sync pause/resume** from the FAB bubble or settings menu.
- **Neumorphic UI** on Android (`ClipSyncTheme` + `NeuComponents`).
- `NetworkChangeObserver` (Android) and `ReachabilityMonitor` (Mac): auto-reconnect on network change.
- CI in `.github/workflows/ci.yml` (mac + android jobs, push to main and PRs).

---

# pbxproj

- Pattern: `objectVersion 56`, 24-char hex IDs.
- Last used range: `A000000000000000000024xx`.
- Next free range: `A000000000000000000025xx`.
- Project migrated to **Xcode 26** (`LastUpgradeCheck = 2620`, scheme version `1.3`).
- `DEVELOPMENT_TEAM = 8FNGRJHHV4` set in both configs (Debug/Release).
- `DEAD_CODE_STRIPPING = YES` and `STRING_CATALOG_GENERATE_SYMBOLS = YES` active.

---

# Known Technical Debt

- `TokenStore.revoke()` exists but is not wired to the menu bar (UI pending).
- No pairing-secret rotation from the UI.
- macOS code signing: team ID `8FNGRJHHV4` configured but no certificate/provisioning profile; unsigned builds work (`CODE_SIGN_IDENTITY = "-"`). Android APK signing not configured.
- mDNS does not work over Tailscale (no multicast); documented in `docs/guides/tailscale-setup.md`.

---

# Android Build Rules

- **Do not compile for device installation** (`assembleDebug` + `adb install` are not permitted during development verification).
- To check for compilation errors use exclusively:
  ```
  ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:"
  ```
- `assembleDebug` is only permitted to verify a full build does not break (without installing).

---

# UI Notes

- `PairingWindow` is fixed at **380×460** pt (not resizable).
- The QR code is generated once in `onAppear` and cached in `@State` — do not recompute on every render.
