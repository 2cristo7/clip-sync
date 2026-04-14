# ClipSync — macOS Menu Bar App

Menu bar app (`LSUIElement=YES`) that hosts an HTTP/WebSocket server for
clipboard synchronisation with paired devices.

## Requirements

- macOS 13+
- Xcode 15+ (tested with Xcode 26)
- Swift 5.9+

## Build

```bash
xcodebuild \
  -project mac/ClipSync.xcodeproj \
  -scheme ClipSync \
  -configuration Debug \
  build
```

The resulting bundle is written to Xcode's DerivedData. To build into
`mac/build/Debug/ClipSync.app` explicitly:

```bash
xcodebuild \
  -project mac/ClipSync.xcodeproj \
  -scheme ClipSync \
  -configuration Debug \
  -derivedDataPath mac/build \
  build
```

## Run

Open the `.app` bundle or launch it from Xcode. A clipboard icon appears
in the menu bar; use **Quit ClipSync** to exit.

## Health endpoint

Once launched, the embedded HTTP server listens on `0.0.0.0:7010`:

```bash
curl -s http://127.0.0.1:7010/health | jq .
# { "ok": true, "version": "0.1.0", "platform": "macos" }
```

## Phase scope

This phase (Phase 1) intentionally does **not** implement NSPasteboard
watching (Phase 2) or mDNS/Bonjour advertising (Phase 3).
