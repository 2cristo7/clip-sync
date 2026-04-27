# ClipSync — macOS Menu Bar App

Menu bar app (`LSUIElement=YES`) that runs an HTTP/WebSocket server for real-time clipboard synchronisation with paired Android devices.

## Requirements

- macOS 14.0+
- Xcode 15+ (tested with Xcode 26)
- Swift 5.9+

## Build

```bash
xcodebuild \
  -project mac/ClipSync.xcodeproj \
  -scheme ClipSync \
  -configuration Debug \
  -derivedDataPath mac/build \
  build
```

The app bundle is written to `mac/build/Build/Products/Debug/ClipSync.app`.

## Run

Open the `.app` bundle or launch from Xcode. A clipboard icon appears in the menu bar; use **Quit ClipSync** to exit.

## Health endpoint

Once running, the embedded server listens on `0.0.0.0:7010`:

```bash
curl -s http://127.0.0.1:7010/health | python3 -m json.tool
# { "ok": true, "version": "0.1.0", "platform": "macos" }
```

## Tests

```bash
xcodebuild test \
  -project mac/ClipSync.xcodeproj \
  -scheme ClipSync \
  -destination 'platform=macOS'
```

## Code signing

See `scripts/setup-signing.sh` and the [installation guide](../docs/installation.md#2-macos--build-and-install).
