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

# Mac Build & Install

## Run locally (development)
Open `mac/ClipSync.xcodeproj` in Xcode and press **⌘R**. This builds a Debug arm64 binary with full Xcode signing. Logs appear in the Xcode console.

## Build for installation
Run from the repo root:
```bash
cd mac && rm -rf build && xcodebuild build \
  -project ClipSync.xcodeproj \
  -scheme ClipSync \
  -configuration Release \
  -derivedDataPath build/DerivedData \
  CODE_SIGN_IDENTITY="-" \
  CODE_SIGNING_REQUIRED=YES \
  CODE_SIGNING_ALLOWED=YES \
  -quiet
```

## Install to /Applications
```bash
rm -rf /Applications/ClipSync.app && \
cp -R mac/build/DerivedData/Build/Products/Release/ClipSync.app /Applications/ClipSync.app
```

> **Important:** always use `rm -rf` before `cp -R` — a plain `cp` over an existing `.app` produces a broken merge. The installed binary must show `flags=adhoc,runtime` (not `linker-signed`) when checked with `codesign -dvvv /Applications/ClipSync.app`.

---

# UI Notes

- `PairingWindow` is fixed at **380×460** pt (not resizable).
- The QR code is generated once in `onAppear` and cached in `@State` — do not recompute on every render.

---

# Wire Protocol Invariants (DO NOT VIOLATE)

## Two distinct timestamp units — never confuse them

| Field | Unit | Where |
|-------|------|-------|
| `ClipPayload.ts` | **milliseconds** | JSON body of `/inject` and WS frames |
| HMAC `t=` in `X-ClipSync-Signature` header | **seconds** | only the HMAC header |

- Mac is canonical: `ts = Int64(Date().timeIntervalSince1970 * 1000)`. Mac validates `abs(now_ms - ts) < 5*60*1000`.
- Android must build `ts = System.currentTimeMillis()` (NEVER `/1000L`). Android validate uses `nowMs` and `5*60*1000` window.
- HMAC header: `ClipSender` passes `clockMs / 1000L` to `HmacSigner.signatureHeader(secret, timestampSec, body)`. Mac `HMACValidator` skew is 60s. Keep seconds for HMAC only.
- Symptom of confusion: Android logs `[WS] error … bad frame: Timestamp out of range` (Mac→Android decode), or Mac `/inject` returns HTTP 500 with `timestampOutOfRange` (Android→Mac).

## Server error mapping
- `ClipServer` `/inject` MUST wrap `JSONDecoder.decode` + `payload.validate()` in `do/catch` and `throw HTTPError(.badRequest, ...)`. Otherwise Hummingbird converts uncaught Swift errors to 500 and clients can't show useful messages.

---

# Android Foreground Service Rules (Android 12+)

- `ClipForegroundService` must **stay foreground for its entire lifetime**. Calling `startForeground()` from a WS/network callback after a previous `stopForeground(true)` throws `ForegroundServiceStartNotAllowedException` ("Service.startForeground() not allowed due to mAllowStartForeground false") — causes a tight WS-error → reconnect loop.
- DO NOT call `stopForeground(true)` on WS close/error. Update the existing notification text via `NotificationManager.notify(NOTIF_ID, buildNotification(...))` instead — use the `updateNotification()` helper.
- `startForeground()` is only safe inside `onCreate` / `onStartCommand` (allowed window opened by `startForegroundService()`).

---

# Coroutine Cancellation Rule

- `catch (t: Throwable)` around a `Flow.collect { }` (or any suspending block) MUST first rethrow `CancellationException`, otherwise legitimate cancellations (job restart, viewmodel clear, network change) surface to the user as fake error toasts (`StandaloneCoroutine was cancelled`). Pattern:

```kotlin
} catch (t: CancellationException) {
    throw t
} catch (t: Throwable) {
    // real error handling
}
```
