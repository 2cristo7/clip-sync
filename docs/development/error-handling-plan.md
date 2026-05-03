# Plan: Error Handling UX Overhaul

**Goal:** User-friendly, expandable error messages on both platforms. No silent failures. Every error the user can act on must be visible.

---

## Current State (Problems)

### Mac — Silent Failures
| Where | Problem |
|-------|---------|
| `App.swift:startPipeline` | TLS failure silently falls back to plain HTTP — user never knows |
| `WebSocketHub` broadcast | Client dropped on write failure — only `debug` log |
| `ClipServer` startup | "Address in use" detected via fragile string match; other startup errors only logged |
| `AuthMiddleware` | HMAC failures logged at `.info()` — low visibility |
| `MenuBarController` | No error state shown — icon is always the same regardless of server health |

### Android — Silent Catches
| Where | Problem |
|-------|---------|
| `SettingsViewModel.bootstrap()` | Catches all errors, logs, but **never updates UI state** |
| `SettingsViewModel.startNetworkWatch()` | Discovery errors swallowed silently |
| `SettingsViewModel.requestShizukuPermission()` | Permission failures not shown |
| `PairingApi.ping()` | Returns `false` on any error — can't distinguish timeout vs cert mismatch vs unreachable |
| `NsdDiscovery` | Start/resolve failures close flow or drop silently — caller gets empty list, no error |
| `SettingsScreen` error card | Only shows if `state.error` is set — many paths skip that |

### TLS Certificate (Mac — Specific Concern)
- `TLSManager.loadOrCreate()` generates a self-signed cert on first launch
- If generation fails (`serializationFailed`, `storageFailed`), error propagates to `startPipeline`
- `startPipeline` catches this and **silently falls back to plain HTTP** — no warning to user
- Keychain errors (`-34018` etc.) can prevent cert storage — user has no idea sync is unencrypted

---

## Design: Expandable Error Banner

### Concept
Both platforms get a **two-level error display**:

1. **Collapsed:** Short, human-readable summary + colored indicator (red/orange)
2. **Expanded:** Tap/click to reveal technical details + suggested action

### Mac — Menu Bar Error Badge + Alert Panel

#### Menu Bar Icon States
| State | Icon |
|-------|------|
| Healthy | Current icon (normal) |
| Warning (degraded) | Icon + orange dot badge |
| Error (broken) | Icon + red dot badge |

#### Error Panel (in menu dropdown)
```
┌─────────────────────────────────────┐
│ ⚠ Running without TLS encryption   │  ← orange, collapsed
│   ▸ Show details                    │
├─────────────────────────────────────┤
│ TLSManager failed to generate       │  ← expanded on click
│ certificate: storageFailed           │
│                                      │
│ Keychain returned OSStatus -34018.   │
│ Try: Restart the app, or check       │
│ Keychain Access for ClipSync items.  │
│                                      │
│ [Copy Error] [Dismiss]               │
└─────────────────────────────────────┘
```

#### Implementation

**New file:** `mac/ClipSync/UI/ErrorState.swift`
```
enum AppError: Identifiable {
    case tlsFallback(detail: String)
    case serverStartFailed(detail: String)
    case portInUse(port: Int)
    case keychainFailure(status: OSStatus)
    case webSocketDropped(clientCount: Int)
    case pairingFailed(detail: String)

    var id: String       // unique key
    var severity: Severity  // .warning | .error
    var summary: String     // human-readable, 1 line
    var detail: String      // technical, expandable
    var suggestion: String? // actionable next step
}
```

**Changes to existing files:**

| File | Change |
|------|--------|
| `App.swift` (AppDelegate) | Add `@Published var errors: [AppError]` array. On TLS failure, append `.tlsFallback` **before** falling back to HTTP. On server start failure, append `.serverStartFailed`. |
| `MenuBarController` | Observe `errors` array. Show badge on icon. Add error items at top of menu with disclosure triangle. |
| `ClipServer` | On startup error, throw typed error (not string-match). On "address in use", throw `.portInUse`. |
| `WebSocketHub` | On client drop, post notification or call delegate — don't just log at debug. |
| `TLSManager` | Add `LocalizedError` conformance to `TLSManagerError` with `errorDescription` and `recoverySuggestion`. |
| `PairingWindow` | Show inline error if pairing session creation fails (`.randomFailure`). |

### Android — Error Banner Component + Expandable Detail

#### Error Banner (in SettingsScreen)
```
┌─────────────────────────────────────┐
│ 🔴 Connection failed                │  ← red NeuCard, collapsed
│     Server unreachable on 10.0.0.5  │
│     ▾ Details                       │
├─────────────────────────────────────┤
│ javax.net.ssl.SSLHandshakeException │  ← expanded
│ SPKI pin mismatch!                   │
│ Expected=abc123 Actual=xyz789        │
│                                      │
│ This usually means the Mac app       │
│ regenerated its certificate.         │
│ Re-pair to fix.                      │
│                                      │
│ [Copy Error]  [Re-pair]  [Dismiss]   │
└─────────────────────────────────────┘
```

#### Implementation

**New composable:** `ErrorBanner` in `SettingsScreen.kt` (or extracted to `ui/components/ErrorBanner.kt`)
```kotlin
@Composable
fun ErrorBanner(
    error: AppError,
    onDismiss: () -> Unit,
    onAction: (() -> Unit)? = null,
    actionLabel: String? = null
)
```

**New model:** `AppError` data class
```kotlin
data class AppError(
    val id: String,
    val severity: Severity,     // WARNING, ERROR
    val summary: String,        // "Connection failed"
    val detail: String?,        // technical stacktrace/message
    val suggestion: String?,    // "Re-pair to fix"
    val action: ErrorAction?    // optional button action
)

enum class Severity { WARNING, ERROR }

sealed class ErrorAction {
    data object Repair : ErrorAction()
    data object Retry : ErrorAction()
    data class OpenUrl(val url: String) : ErrorAction()
}
```

**Changes to existing files:**

| File | Change |
|------|--------|
| `SettingsState` | Replace `error: String?` with `errors: List<AppError>` |
| `SettingsViewModel.bootstrap()` | On catch: add `AppError` to state instead of just logging |
| `SettingsViewModel.startNetworkWatch()` | On discovery error: add warning `AppError` |
| `SettingsViewModel.pair()` | Classify exceptions: SSLHandshakeException → cert mismatch suggestion, ConnectException → unreachable suggestion, PairingException → wrong code suggestion |
| `SettingsViewModel.requestShizukuPermission()` | On catch: add `AppError` with Shizuku-specific suggestion |
| `PairingApi.ping()` | Return `Result<Boolean>` instead of bare `Boolean` — propagate error type |
| `NsdDiscovery` | Emit sealed `DiscoveryEvent` (Found/Lost/Error) instead of just `NsdServiceInfo` |
| `SettingsScreen` | Replace current error card with `ErrorBanner` composable. Show list of active errors. |

---

## Phase Plan

### Phase 1: Error Model + Mac TLS Warning (highest value)
**Files:** `ErrorState.swift` (new), `App.swift`, `TLSManager.swift`, `MenuBarController.swift`

1. Create `AppError` enum with `LocalizedError` conformance
2. Add `errors` array to `AppDelegate`
3. Surface TLS fallback as `.tlsFallback` warning
4. Surface server start failure as `.serverStartFailed`
5. Show error count badge on menu bar icon
6. Add expandable error items in menu dropdown

**Success:** User sees orange badge + "Running without TLS" if cert generation fails.

### Phase 2: Android Error Banner + Silent Catch Fixes
**Files:** `SettingsScreen.kt`, `SettingsViewModel.kt`, `SettingsState`

1. Create `AppError` data class and `ErrorBanner` composable
2. Replace `state.error: String?` with `state.errors: List<AppError>`
3. Fix `bootstrap()` silent catch → add `AppError`
4. Fix `requestShizukuPermission()` silent catch → add `AppError`
5. Classify pairing exceptions with specific suggestions
6. Add dismiss + copy-error functionality

**Success:** All current silent catches surface in UI with expandable details.

### Phase 3: Mac Server + WebSocket Error Propagation
**Files:** `ClipServer.swift`, `WebSocketHub.swift`, `MenuBarController.swift`

1. Replace string-match error detection with typed errors in `ClipServer`
2. Surface "port in use" as specific `AppError`
3. Add client-drop notification from `WebSocketHub`
4. Show connected client count + drop events in menu

**Success:** Server startup failures and client drops visible in menu bar.

### Phase 4: Android Discovery + Network Error Propagation
**Files:** `NsdDiscovery.kt`, `PairingApi.kt`, `SettingsViewModel.kt`

1. Change `NsdDiscovery` flow to emit `DiscoveryEvent` sealed class (Found/Lost/Error)
2. Change `PairingApi.ping()` to return `Result<Boolean>`
3. Fix `startNetworkWatch()` silent catch → surface as warning
4. Show discovery errors in UI ("No servers found — check Wi-Fi")

**Success:** Discovery failures visible. Network errors propagated with context.

### Phase 5: Polish + Notification Fallback
**Both platforms**

1. Mac: Add macOS notification for critical errors (TLS fallback, server crash) as fallback if menu not open
2. Android: Add persistent notification for connection errors when app backgrounded
3. Add "Copy full error" button on both platforms for bug reports
4. Test all error paths manually

---

## Error Classification Reference

| Error | Severity | Summary | Suggestion |
|-------|----------|---------|------------|
| TLS cert generation failed | Warning | Running without encryption | Restart app; check Keychain Access |
| TLS cert expired/invalid | Warning | Certificate needs renewal | Restart app to regenerate |
| Keychain -34018 | Error | Cannot access secure storage | Restart Mac; reset Keychain if persists |
| Port in use | Error | Port {N} already in use | Close other ClipSync instance or change port |
| Server start failed | Error | Server couldn't start | Check console logs; restart app |
| WebSocket client dropped | Warning | Device disconnected | Will reconnect automatically |
| SPKI pin mismatch | Error | Certificate changed on server | Re-pair with Mac |
| Pairing code wrong | Error | Pairing code didn't match | Try again with new code |
| Pairing expired | Warning | Pairing code expired | Generate new code on Mac |
| Discovery failed | Warning | Can't find servers on network | Check Wi-Fi; both devices same network |
| Network unreachable | Error | Can't reach server | Check network connection |
| Tailscale VPN not active | Warning | Tailscale required for remote sync | Open Tailscale and connect |

---

## Not In Scope
- Error analytics/telemetry
- Automatic error recovery (except existing auto-reconnect)
- Localization (English only for now)
- Custom error sounds
