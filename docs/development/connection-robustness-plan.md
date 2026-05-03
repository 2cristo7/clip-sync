# Plan: Connection Robustness Overhaul

**Goal:** Eliminate all silent connection failures. Both platforms must accurately reflect connection state at all times. Reconnection must be automatic and visible.

---

## Bugs Found (Audit)

### Confirmed Bugs (user-reported)

| # | Bug | Platform | Location |
|---|-----|----------|----------|
| B1 | No periodic health check — `startNetworkWatch` polls network type every 3s but never pings server | Android | `SettingsViewModel.startNetworkWatch()` L610-649 |
| B2 | Discovery not restarted on disconnect — mDNS job stops and never restarts | Android | `NsdDiscovery` + `SettingsViewModel.startDiscovery()` |

### Silent Bugs (discovered in audit)

| # | Bug | Platform | Location |
|---|-----|----------|----------|
| B3 | No WebSocket ping/pong — server never sends pings; stale connections persist indefinitely | Mac | `WebSocketHub.swift` — no ping logic |
| B4 | `lastSeen` field tracked but never used for timeout enforcement | Mac | `WebSocketHub.Client` struct L13-36 |
| B5 | Network change only restarts mDNS, not server — server keeps old IP binding | Mac | `ReachabilityMonitor.handlePathUpdate()` L87-88 |
| B6 | Shared `ResolveListener` — concurrent mDNS resolves fail with error 3 (`FAILURE_ALREADY_ACTIVE`) | Android | `NsdDiscovery.kt` L39, L85 |
| B7 | No WebSocket keepalive configured in OkHttp — relies on default 10s read timeout | Android | `ClipClient.kt` `baseBuilder()` L130-135 |
| B8 | ViewModel and ForegroundService disagree on connection state — no sync mechanism | Android | `SettingsViewModel` vs `ClipForegroundService` |
| B9 | Bootstrap pings server once on startup, never again — shows Connected even if Mac crashes | Android | `SettingsViewModel.bootstrap()` L99-161 |
| B10 | Discovery only restarts on WiFi gain (`!prev.isOnWifi && onWifi`), not on any network change | Android | `startNetworkWatch()` L640 |
| B11 | Server startup is fire-and-forget `Task.detached` — errors invisible to app lifecycle | Mac | `App.swift startPipeline()` L93-103 |
| B12 | mDNS publication failure only logged, never surfaced — advertiser can silently fail | Mac | `BonjourAdvertiser.swift` delegates L70-82 |
| B13 | Shizuku hash mismatch: text uses `String.hashCode()`, images use `mgr.getClipboardHash()` — echo loop risk | Android | `ClipForegroundService.onFrame()` vs `pollViaShizuku()` |

### Race Conditions

| # | Race | Platform |
|---|------|----------|
| R1 | Network change clears host → old WS onFailure fires → scheduleReconnect races with new discovery | Android |
| R2 | Shizuku poll can fire between `handler.post { writeImage }` and actual write → sees stale hash → echo | Android |
| R3 | `advertiser.stop()` then `advertiser.start()` without awaiting stop completion | Mac |

---

## Architecture Changes

### Mac — Server-Side Health

**WebSocket Ping/Pong (fixes B3, B4):**
- Server sends WebSocket ping every 30s to all clients
- If pong not received within 10s → unregister client
- Use `lastSeen` field (already exists, unused) for timeout enforcement
- New periodic `Task` in `WebSocketHub` that iterates clients

**mDNS Reliability (fixes B5, B12):**
- `ReachabilityMonitor` notifies `AppDelegate` via callback (not just advertiser)
- On network change: restart advertiser AND log server binding status
- Track mDNS publication state; surface failure as `AppError`
- Add `await` between `stop()` and `start()` to prevent race R3

**Server Lifecycle (fixes B11):**
- `startPipeline()` wraps server start in monitored Task, not fire-and-forget
- On crash: append `AppError`, attempt restart with backoff (max 3 retries)

### Android — Client-Side Health

**Periodic Health Check (fixes B1, B9):**
- Add server ping every 15s in `ClipForegroundService`
- On 3 consecutive ping failures → set status to `Disconnected`, trigger reconnect
- `SettingsViewModel` observes service state via `SharedFlow` or broadcast

**Discovery Auto-Restart (fixes B2, B10):**
- On WebSocket disconnect: cancel discovery job + restart after 2s delay
- On any network change (not just WiFi gain): restart discovery
- Add exponential backoff to discovery restart (max 30s)

**Fix Shared ResolveListener (fixes B6):**
- Create new `ResolveListener` instance per `resolveService()` call
- Alternative: queue resolves serially (resolve one, then next)

**WebSocket Keepalive (fixes B7):**
- Configure OkHttp `pingInterval(30, TimeUnit.SECONDS)` in `baseBuilder()`
- OkHttp handles ping/pong automatically

**ViewModel-Service State Sync (fixes B8):**
- `ClipForegroundService` exposes `connectionState: StateFlow<ConnectionStatus>`
- `SettingsViewModel` collects from service StateFlow instead of maintaining separate state
- Single source of truth for connection status

**Fix Shizuku Hash (fixes B13):**
- After text write via Shizuku: also update `lastShizukuHash` from `mgr.getClipboardHash()` (not `text.hashCode()`)
- Ensures poll comparison uses same hash source

---

## Phase Plan

### Phase 1: WebSocket Keepalive (both platforms — highest impact)

Fixes B3, B4, B7. Most connection "ghost" bugs stem from no keepalive.

**Mac — `WebSocketHub.swift`:**
1. Add periodic ping task (30s interval)
2. Track `lastPongReceived` per client
3. Timeout clients that don't respond within 10s
4. Use existing `lastSeen` for timeout (update on pong receipt)

**Android — `ClipClient.kt`:**
1. Add `pingInterval(30, TimeUnit.SECONDS)` to OkHttp builder
2. OkHttp handles rest automatically

### Phase 2: Android Health Check + State Sync

Fixes B1, B8, B9. User sees accurate connection state.

**`ClipForegroundService`:**
1. Add `_connectionState: MutableStateFlow<ServiceConnectionState>` exposed as `StateFlow`
2. `ServiceConnectionState`: `Disconnected | Connecting | Connected(host) | Paused(host)`
3. Add health check: ping `/health` every 15s when connected
4. On 3 consecutive failures → transition to `Disconnected`, `scheduleReconnect()`
5. Expose state via companion `stateFlow`

**`SettingsViewModel`:**
1. Collect `ClipForegroundService.stateFlow` in `bootstrap()`
2. Map `ServiceConnectionState` → `ConnectionStatus` for UI
3. Remove duplicate connection state tracking

### Phase 3: Discovery Robustness (Android)

Fixes B2, B6, B10.

**`NsdDiscovery.kt`:**
1. Create new `ResolveListener` per `resolveService()` call (fix B6)
2. Add resolve queue if concurrent resolves still problematic

**`SettingsViewModel`:**
1. On WebSocket disconnect: restart discovery after 2s
2. On any network type change (not just WiFi gain): restart discovery
3. Add auto-restart with backoff if discovery flow completes unexpectedly

### Phase 4: Mac Network + Server Lifecycle

Fixes B5, B11, B12, R3.

**`ReachabilityMonitor.swift`:**
1. Add `onNetworkChange: () -> Void` callback to notify AppDelegate
2. AppDelegate logs server binding status on network change
3. Await advertiser stop before restart (fix R3)

**`BonjourAdvertiser.swift`:**
1. Track publication state: `.publishing | .published | .failed(Error)`
2. Surface failure via callback to AppDelegate → `AppError`

**`App.swift`:**
1. Monitor server Task — on unexpected exit, attempt restart (3 retries, 5s backoff)
2. On network change callback: verify server still accepting connections

### Phase 5: Fix Races + Shizuku Hash

Fixes B13, R1, R2.

**`ClipForegroundService`:**
1. After text Shizuku write: `lastShizukuHash = mgr.getClipboardHash()` instead of `text.hashCode()`
2. Add `synchronized` block around clipboard write + hash update to prevent R2
3. On network change: cancel pending reconnect before clearing host (prevent R1)

---

## Testing Strategy

Each phase validates with:

| Test | How |
|------|-----|
| Mac kill while Android connected | Kill Mac process → Android should show Disconnected within ~45s (ping timeout) |
| Network switch | Toggle WiFi → both platforms re-discover and reconnect |
| Multiple mDNS servers | Run 2 Mac instances → Android should discover both |
| Idle connection | Leave connected 1 hour → status still accurate on both |
| Rapid network toggles | Toggle WiFi 5x fast → no duplicate connections or zombie state |
| Shizuku echo | Copy text on Android → should NOT echo back from Mac |

---

## Not In Scope

- Protocol changes (wire format stays same)
- New pairing flow
- Tailscale-specific reconnection
- Battery optimization (keepalive intervals are conservative enough)
