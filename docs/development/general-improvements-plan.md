# Plan: General Improvements Audit

**Scope:** Security hardening, performance, UX, build config, code quality — all areas.
**Source:** Full codebase audit (April 30, 2026).

---

## Issues by Priority Tier

### TIER 1 — Security (HIGH)

| # | Issue | Platform | File | Line |
|---|-------|----------|------|------|
| S1 | No rate limiting on `/pair` — 6-digit code brute-forceable | Mac | ClipServer.swift | 183-200 |
| S2 | No rate limiting on `/inject` — clipboard flood attack | Mac | ClipServer.swift | 155-180 |
| S3 | No pre-decode size validation — 25MB base64 → OOM | Mac | ClipServer.swift | 162-174 |
| S4 | Keychain uses `kSecAttrAccessibleAfterFirstUnlock` — too permissive | Mac | Keychain.swift | 44 |
| S5 | Payload size limits defined but NEVER checked on receipt | Android | ClipForegroundService.kt | onFrame() |
| S6 | No payload field validation (type, mime, nonce, ts) | Android | ClipPayload.kt | 9-23 |
| S7 | Hostname verification disabled (`hostnameVerifier { _, _ -> true }`) | Android | ClipClient.kt | 52-53, 95 |
| S8 | HMAC clock skew 60s — replay window too large | Mac | HMACValidator.swift | 28-39 |
| S9 | MIME type not validated — accepts arbitrary strings | Mac | ClipPayload.swift | 8-26 |
| S10 | Bearer token no length validation — arbitrarily long strings | Mac | AuthMiddleware.swift | 69-77 |

### TIER 2 — Performance & Battery (HIGH)

| # | Issue | Platform | File | Line |
|---|-------|----------|------|------|
| P1 | Shizuku polling every 500ms — major battery drain | Android | ClipForegroundService.kt | 461 |
| P2 | No ImageCache size limit — only time-based cleanup (24h) | Android | ImageCache.kt | 35-48 |
| P3 | Health check creates unbounded new Thread every 15s | Android | ClipForegroundService.kt | 66-99 |
| P4 | Base64 decode + BitmapFactory can allocate 60+ MB for large images | Android | IncomingClipNotifier.kt | 110-130 |
| P5 | Bitmap never recycled if exception after BitmapFactory | Android | IncomingClipNotifier.kt | 133 |
| P6 | CoroutineScope never cancelled in SendClipActivity.onDestroy() | Android | SendClipActivity.kt | 36 |
| P7 | Mac pasteboard polling hardcoded 500ms, not configurable | Mac | PasteboardWatcher.swift | 33 |

### TIER 3 — Build & Config (HIGH/MEDIUM)

| # | Issue | Platform | File | Line |
|---|-------|----------|------|------|
| C1 | targetSdk=34 vs compileSdk=35 mismatch | Android | build.gradle.kts | 14 |
| C2 | R8 minification disabled in release (`isMinifyEnabled = false`) | Android | build.gradle.kts | 23 |
| C3 | `securityCrypto = 1.1.0-alpha06` — alpha dep in production | Android | libs.versions.toml | 9 |
| C4 | Port 7010 hardcoded everywhere — no config override | Both | ServerConfig.swift, Prefs.kt, SettingsScreen.kt | multiple |
| C5 | .gitignore missing `*.pem`, `*.key`, `*.p12` patterns | Both | .gitignore | — |
| C6 | CI: no test artifact uploads, no integration tests | Both | ci.yml | — |
| C7 | No centralized version tracking between platforms | Both | build.gradle.kts, pbxproj | — |
| C8 | Gradle configuration cache disabled | Android | gradle.properties | — |

### TIER 4 — UX & Feature Gaps (HIGH/MEDIUM)

| # | Issue | Platform | File | Line |
|---|-------|----------|------|------|
| U1 | ErrorAction handlers unimplemented (Repair, Retry, OpenUrl) | Android | SettingsScreen.kt | 706-720 |
| U2 | No sync pause/resume toggle on Mac | Mac | MenuBarController.swift | — |
| U3 | SettingsScreen is 953 lines monolithic — unmaintainable | Android | SettingsScreen.kt | 97-758 |
| U4 | SettingsViewModel is 706 lines — too many responsibilities | Android | SettingsViewModel.kt | — |
| U5 | No retry logic on transient network errors in ClipSender | Android | ClipSender.kt | 41-76 |
| U6 | No accessibility labels on status indicators | Android | SettingsScreen.kt | 282, 450, 664 |
| U7 | PairingCodeDialog has no timeout — stays open forever | Android | SettingsScreen.kt | 869-934 |
| U8 | TLS cert validity 5 years — should be 1 year max | Mac | TLSManager.swift | 104 |
| U9 | No token rotation mechanism | Mac | TokenStore.swift | 35-52 |
| U10 | Echo suppression window too small (8 entries) | Mac | PasteboardWatcher.swift | 87-90 |
| U11 | Dead code: Clay* aliases in NeuComponents.kt | Android | NeuComponents.kt | 405-436 |
| U12 | ClipForegroundService.kt 606 lines — mixed responsibilities | Android | ClipForegroundService.kt | — |

---

## Implementation Phases

### Phase 1: Security Hardening (most critical)
**Fixes:** S1-S6, S8-S10

**1A — Mac Rate Limiting + Input Validation**
- Add per-IP rate limiter to ClipServer (10 req/s for `/inject`, 5 attempts/min for `/pair`)
- Add pre-decode size check: `base64.count * 3/4 < maxFileBytes`
- Validate MIME type whitelist: `text/*`, `image/*`, `application/octet-stream`
- Validate `ts` within ±5min, `nonce` non-empty, `name` max 1024 chars
- Add max Bearer token length (512 chars)
- Reduce HMAC clock skew from 60s to 30s

**1B — Android Payload Validation**
- Check `payload.data.length * 3/4 < MAX_IMAGE_BYTES` before base64 decode in `onFrame()`
- Validate `type` enum, `mime` non-empty ≤256 chars, `nonce` non-empty, `ts` within ±5min
- Reject unknown `type` values

**1C — Keychain Hardening (Mac)**
- Change `kSecAttrAccessibleAfterFirstUnlock` → `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`
- Test that existing keychain items migrate correctly (may need delete + re-save)

### Phase 2: Performance & Battery
**Fixes:** P1-P7

**2A — Android Battery Optimization**
- Increase Shizuku poll from 500ms to 1000ms
- Add adaptive polling: if unchanged 10 polls in a row, increase to 2000ms; reset on change
- Replace `Thread { }` health check with `Dispatchers.IO` coroutine
- Add `scope.cancel()` to `SendClipActivity.onDestroy()`

**2B — Android Memory Safety**
- Add ImageCache size limit: 200MB max, LRU eviction
- Add pre-decode size check in `IncomingClipNotifier`: skip BitmapFactory for images >5MB, show placeholder
- Add `bitmap?.recycle()` in finally block for notification builder
- Use `BitmapFactory.Options { inSampleSize }` for large images in notifications

**2C — Mac Polling Optimization**
- Make pasteboard polling interval configurable (default 750ms, range 250-2000ms)
- Increase echo suppression window from 8 to 16 entries

### Phase 3: Build & Config
**Fixes:** C1-C8

**3A — Android Build Fixes**
- `targetSdk = 35` (match compileSdk)
- `isMinifyEnabled = true` for release + test ProGuard rules
- Change `securityCrypto` from `1.1.0-alpha06` to stable `1.0.0` or latest stable
- Enable `org.gradle.configuration-cache=true`

**3B — Cross-Platform Config**
- Extract port 7010 to constants: Mac `ServerConfig.defaultPort`, Android `ClipSyncConstants.DEFAULT_PORT`
- Add environment variable override: `CLIPSYNC_PORT` on Mac
- Add `.gitignore` entries: `*.pem`, `*.key`, `*.p12`, `secrets/`
- Create `VERSION` file at repo root, sourced by both builds

**3C — CI Improvements**
- Pin macOS runner to `macos-14`
- Add test artifact uploads for both platforms
- Add Gradle cache step
- Add dependency vulnerability scan step

### Phase 4: UX Polish
**Fixes:** U1-U7, U11

**4A — Android ErrorAction Handlers**
- Implement `ErrorAction.Repair` → navigate to re-pair flow
- Implement `ErrorAction.Retry` → call relevant retry function
- Implement `ErrorAction.OpenUrl` → open external browser
- Add pairing code dialog timeout (2 min auto-dismiss)
- Add accessibility `contentDescription` to `StatusDot`, `NeuStatusBadge`

**4B — Mac Sync Toggle**
- Add "Pause Sync" / "Resume Sync" menu item in MenuBarController
- Wire to PasteboardWatcher start/stop
- Show paused state in menu bar icon (grey dot)

**4C — Android ClipSender Retry**
- Add retry with exponential backoff for HTTP 500-599 and timeout errors
- Max 3 retries, 1s/2s/4s backoff
- Distinguish permanent (401, 403, cert mismatch) from transient (timeout, 500+) errors
- Show retry count in UI

### Phase 5: Code Quality (refactors — lowest priority)
**Fixes:** U3, U4, U11, U12

**5A — Android Code Splits**
- Split SettingsScreen.kt into sub-composables:
  - `ConnectionModeSection.kt`
  - `ServerDiscoverySection.kt`
  - `ManualConnectionSection.kt`
  - `TailscaleSection.kt`
  - `PermissionsSection.kt`
- SettingsScreen becomes orchestrator (~200 lines)

**5B — Android ViewModel Split**
- Extract from SettingsViewModel:
  - `DiscoveryManager.kt` (discovery + network watch)
  - `PairingManager.kt` (pair flow + persist)
- SettingsViewModel delegates to these, stays as UI state holder

**5C — Cleanup**
- Remove Clay* aliases from NeuComponents.kt (dead code)
- Extract ClipForegroundService clipboard handling to `ClipboardHandler.kt`

---

## Not In Scope

- Wire protocol changes
- New features (file transfer, multi-device)
- Tailscale-specific improvements (separate plan)
- Full test suite expansion (separate effort)
- Android instrumented tests (separate CI ticket)
