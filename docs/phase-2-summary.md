# Phase 2 Summary — Clipboard Watcher + WebSocket + /inject

**Branch**: `feature/mac-clipboard-core` → merged into `main` with `--no-ff` (`8243f01`).

## What shipped

- `ClipPayload` (Codable/Sendable): `type`, `mime`, `dataBase64`, `ts`, `nonce`.
- `PasteboardWatcher`: `DispatchSourceTimer` polling `NSPasteboard.general.changeCount` every 500 ms; publishes via `AsyncStream<ClipPayload>`; captures `.string`, `.png`, `.tiff`.
- `PasteboardInjector`: `clearContents + setData/setString`; calls `watcher.suppressNextMatching(payload)` **before** touching the pasteboard to win races.
- `WebSocketHub` (`actor`): `Set<Client>`, JSON broadcast, drops clients on write failure.
- `ClipServer`: adds `POST /inject` (reads `X-ClipSync-Source` header) and `/ws` upgrade via `.http1WebSocketUpgrade`; `autoPing` 30 s from `WebSocketServerConfiguration`.
- `AppDelegate`: wires `PasteboardWatcher.events() → WebSocketHub.broadcast(_:)` in a detached task; owns hub, watcher, injector, server.
- Anti-loop: SHA-256 digest of `(type, mime, dataBase64)` (ignoring `ts`/`nonce`); injector pushes a bounded (≤8) suppression window into the watcher.
- `ClipSyncTests` target: 7 XCTest cases — payload codable, digest invariance, injector text/image roundtrip, watcher emission on external change, anti-loop verification. All green.

## Commits

1. `feat[mac-clipboard]: add NSPasteboard watcher with changeCount polling`
2. `feat[mac-server]: broadcast clipboard changes via websocket`
3. `feat[mac-server]: accept POST /inject to write into NSPasteboard`
4. `test[mac-clipboard]: roundtrip text and image through pasteboard`

## Validation

- `xcodebuild test … -scheme ClipSync` → 7/7 passed.
- `curl -X POST /inject … {"…","dataBase64":"aG9sYQ=="}` → `pbpaste` → `hola`.
- WebSocket client on `/ws` receives JSON frames when text is `pbcopy`-ed.
- `osascript` writes PNG → frame with `type=image, mime=image/png`, PNG magic `89504e470d0a1a0a`.
- 3 consecutive `/inject` calls → exactly 3 broadcasts, zero echoed ticks.

## Deviations from plan

- **HummingbirdWebSocket lives in a separate package** (`hummingbird-project/hummingbird-websocket`) in 2.x. The Phase 1 note assumed it was in the same package. Added a second `XCRemoteSwiftPackageReference` (up-to-next-major from 2.0.0) + its `XCSwiftPackageProductDependency`.

## Out of scope (next phases)

- Bonjour/mDNS advertising + pairing UI → Phase 3.
- TLS + HMAC auth on `/inject` and `/ws` → Phase 4.
- Bluetooth LE iOS client → later.
