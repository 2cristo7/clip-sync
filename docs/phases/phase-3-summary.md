# Phase 3 Summary — mDNS + Pairing + Dynamic Menu Bar

**Branch**: `feature/mac-discovery-pairing` → merged into `main` with `--no-ff` (`f448eeb`).

## What shipped

- `Keychain` wrapper (`Storage/Keychain.swift`): typed `SecItemAdd/Copy/Update/Delete` + `loadOrCreateSecret()` (32 bytes via `SecRandomCopyBytes`). Default service `com.clipsync.pairing-secret`.
- `BonjourAdvertiser` (`Network/BonjourAdvertiser.swift`): `NetService` on `_clipsync._tcp` port 7010, scheduled on `.main`/`.common`, `includesPeerToPeer=true`. TXT record: `version`, `name`, `fp` (first 16 hex of SHA-256 of pairing-secret).
- `PairingManager` (`Pairing/PairingManager.swift`): actor; 6-digit codes via `SecRandomCopyBytes` (rejection-sampling, bias-free), 5-min TTL, single-use. Returns `{token: base64(32 random), sig: base64(HMAC-SHA256(token, secret))}`. Injectable `PairingClock` for tests.
- `GET /pair?code=…` on `ClipServer`: 400 if missing, 401 on `notStarted|invalid|expired|consumed`, 200 `{token, sig}` on first valid consume.
- `WebSocketHub` extended: `ClipClientInfo` (id, remoteAddress, connectedAt, lastSeen) + `events()` → `AsyncStream<[ClipClientInfo]>` fan-out for UI.
- `MenuBarController` (`UI/MenuBarController.swift`, `@MainActor`): dynamic `NSMenu` with `⚪️ Idle` / `🟢 Connected (N)`, "Start Pairing…", `Clients` submenu (address + lastSeen), Quit. Refreshes on every hub event.
- `PairingWindow` (`UI/PairingWindow.swift`): SwiftUI view with 52-pt monospace code, QR (`CIFilter.qrCodeGenerator` scaled 8×) for `clipsync://pair?host=…&port=…&code=…`, live countdown (red at ≤30 s).
- `AppDelegate` refactored, `@MainActor`: bootstraps pairing secret, instantiates `PairingManager`/`ClipServer`/`BonjourAdvertiser`/`MenuBarController`/`PairingWindowController`.

## Commits

1. `feat[mac-storage]: add keychain wrapper for pairing secret`
2. `feat[mac-network]: add mDNS broadcasting for _clipsync._tcp`
3. `feat[mac-pairing]: generate pairing code and bootstrap shared secret`
4. `feat[mac-ui]: implement dynamic menu bar with connection state`
5. `test[mac-pairing]: verify pairing code single-use and expiration`

## Validation

- `xcodebuild test … -scheme ClipSync` → 17/17 green (7 PairingManager, 2 Keychain, 1 Bonjour smoke + the Phase 2 suite).
- `dns-sd -B _clipsync._tcp local.` → lists `macbook-air-de-diego`.
- `dns-sd -L "<name>" _clipsync._tcp` → `port 7010`, TXT `version=0.1.0 fp=<hex16> name=<host>`.
- `security find-generic-password -s com.clipsync.pairing-secret` → entry present.
- `curl "http://127.0.0.1:7010/pair"` → 400 `missing code`; `…?code=123456` before pairing → 401 `notStarted`.

## Not automated (require GUI/accessibility)

- Click "Start Pairing…" → window with code + QR.
- `curl /pair?code=<real>` → 200 `{token, sig}`, replay → 401.
- Menu bar flips to `🟢 Connected (1)` on WS connect.

## Deviations from plan

- `remoteAddress` field added to `WebSocketHub.Client` but populated as `nil` (channel address isn't easily exposed in Hummingbird 2.x router context); the menu shows the first 8 chars of the client UUID as a placeholder.
- TXT `fp` is SHA-256 of the pairing-secret (truncated to 16 hex), not the TLS cert fingerprint — there's no TLS in this phase (Phase 4).

## Out of scope (next phase)

- HTTPS/WSS + self-signed cert + real fingerprint pinning → Phase 4.
- Bearer auth + HMAC body signature → Phase 4.
- Token persistence (TokenStore) → Phase 4.
