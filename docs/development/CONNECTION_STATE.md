# Connection Robustness Overhaul — State
## Status: COMPLETE
## Current Phase: DONE
## Completed Phases: [1, 2, 3, 4, 5]
## Branch: fix/connection-robustness
## Last Commit: ac817e6 fix[android-sync]: fix Shizuku hash mismatch and reconnect race conditions

## Summary

### Phase 1 — WebSocket Keepalive
- Mac: periodic ping/pong loop in WebSocketHub (30s interval, 45s timeout)
- Android: OkHttp pingInterval(30s) + explicit connect/read/write timeouts

### Phase 2 — Android Health Check + State Sync
- ClipForegroundService exposes ServiceState StateFlow (Disconnected/Connecting/Connected/Paused)
- Periodic health check pings /health every 15s, disconnects after 3 failures
- SettingsViewModel observes service StateFlow as single source of truth

### Phase 3 — Discovery Robustness
- Per-service ResolveListener prevents FAILURE_ALREADY_ACTIVE error
- Discovery auto-restarts on disconnect (2s delay) and any network change
- Auto-restart with 5s backoff on unexpected flow completion

### Phase 4 — Mac Network + Server Lifecycle
- ReachabilityMonitor notifies AppDelegate via onNetworkChange callback
- BonjourAdvertiser tracks publish state, surfaces failures via onPublishFailed
- Advertiser restart has 500ms delay to prevent race condition R3
- Server Task monitored with 3-retry auto-restart on crash

### Phase 5 — Races + Shizuku Hash
- Shizuku hash uses getClipboardHash() consistently (fixes echo loop)
- wsGeneration counter invalidates stale WS callbacks on network change
- Image write + hash update serialized on handler thread

## Bugs Fixed: B1-B13, R1-R3
## Plan: docs/development/connection-robustness-plan.md
