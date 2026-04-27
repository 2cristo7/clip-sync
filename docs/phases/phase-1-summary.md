# Phase 1 Summary — macOS Server Core

**Branch**: `feature/mac-server-core` → merged into `main` with `--no-ff`.

## What shipped

- `mac/ClipSync.xcodeproj` (hand-authored `project.pbxproj`, objectVersion 56).
- Target `ClipSync`: macOS menu-bar app, SwiftUI lifecycle + `AppDelegate` adapter, `LSUIElement=YES`, deployment target **macOS 14.0** (required by Hummingbird 2.x).
- `Info.plist`: `LSUIElement`, `NSLocalNetworkUsageDescription`, `NSBonjourServices=[_clipsync._tcp]`.
- SwiftPM dependency: `hummingbird-project/hummingbird` (up-to-next-major from 2.0.0).
- `ClipServer` runs Hummingbird on `0.0.0.0:7010` inside a detached `Task`; logs port-in-use distinctly.
- `ServerConfig` carries host/port/log level (`ServerConfig.default`).
- `AppDelegate` installs `NSStatusItem` with SF Symbol `doc.on.clipboard` and a Quit menu item; starts/stops the server.
- `GET /health` → `{"ok":true,"version":"0.1.0","platform":"macos"}`.

## Commits

1. `chore[mac]: bootstrap xcode menubar app`
2. `feat[mac-server]: embed hummingbird http server on 0.0.0.0:7010`
3. `feat[mac-server]: add /health endpoint with version metadata`

## Validation

- `xcodebuild -project mac/ClipSync.xcodeproj -scheme ClipSync -configuration Debug build` → `BUILD SUCCEEDED`.
- `curl http://127.0.0.1:7010/health | jq .ok` → `true`.
- `curl http://$(ipconfig getifaddr en0):7010/health` → same JSON (bind `0.0.0.0` verified).
- `lsof -iTCP:7010 -sTCP:LISTEN` → `ClipSync` process listening.

## Deviations from plan

- Plan said macOS 13+; Hummingbird 2.x requires macOS 14, so deployment target is 14.0.

## Out of scope (next phases)

- NSPasteboard watching + `/inject` → Phase 2.
- mDNS/Bonjour advertising + pairing UI → Phase 3.
- TLS + HMAC auth → Phase 4.
