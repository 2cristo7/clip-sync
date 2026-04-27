# Phase 8 Summary — Integración Tailscale + Pruebas Remotas

**Branch**: `feature/tailscale-validation` (merged into `main` via `--no-ff`).

## What shipped

### Documentation
- `docs/tailscale-setup.md` (229 lines): comprehensive setup guide covering Tailscale installation on Mac and Android, tailnet configuration, manual IP pairing for ClipSync, firewall troubleshooting (macOS pfctl / System Settings), known limitations (mDNS not available cross-tailnet), and a "Tested scenarios" table with placeholder statuses for manual device validation.

### macOS — ReachabilityMonitor
- `mac/ClipSync/Network/ReachabilityMonitor.swift`: uses `NWPathMonitor` to detect network interface changes. When the path changes, logs available interfaces and re-announces Bonjour service via DiscoveryManager. Actor-based with `start()`/`stop()` lifecycle.
- Wired into `App.swift` — starts with the app, stops on termination.
- 2 new unit tests in `ReachabilityMonitorTests.swift`.

### Android — NetworkChangeObserver
- `android/app/src/main/java/com/clipsync/net/NetworkChangeObserver.kt`: extracted from inline `ConnectivityManager.NetworkCallback` in ClipForegroundService into a dedicated, testable class. Detects network changes and triggers WebSocket reconnection when state was Connected.
- Wired into `ClipForegroundService` lifecycle (register on start, unregister on stop).
- 6 new unit tests in `NetworkChangeObserverTest.kt`.

## Commits
```
1bee9c6 docs[tailscale]: add end-to-end setup guide
a91d200 feat[mac-net]: reconnect on nwpath change
581a05c feat[android-net]: observe connectivity and trigger reconnect
```

## Validation
- `xcodebuild test`: **35 passed, 0 failed** (was 33 → +2 ReachabilityMonitor tests)
- `./gradlew :app:testDebugUnitTest`: **33 passed, 0 failed** (was 27 → +6 NetworkChangeObserver tests)
- `./gradlew :app:assembleDebug`: BUILD SUCCESSFUL
- `./gradlew :app:lintDebug`: clean

## Deviations from plan
- The "Tested scenarios" section in tailscale-setup.md has placeholder statuses — actual Tailscale E2E testing requires physical devices on different networks (manual validation).
- ReachabilityMonitor re-announces Bonjour but does not restart the HTTP server (server already listens on 0.0.0.0, so no restart needed).

## Out of scope / Follow-ups (for Phase 9)
- Manual device testing over Tailscale (placeholder statuses in docs).
- "Clients → Revoke" menu in macOS status bar (Phase 9 polish).
- AGP 8.5.2 warning "compileSdk 35 tested up to 34" persists (non-blocking).
