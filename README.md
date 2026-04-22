# ClipSync -- Shared Clipboard (Mac <-> Android)

Real-time clipboard synchronization between macOS and Android over your local
network or Tailscale. No cloud services, no accounts, fully open-source.

The Mac runs a lightweight menu-bar server (Swift / Hummingbird 2.x) that
exposes HTTPS + WebSocket on port 7010. The Android client (Kotlin / Jetpack
Compose) connects, receives clipboard changes as push notifications, and sends
content to the Mac via Android's native Share sheet.

## Features

- Real-time clipboard sync (text and images) over WebSocket.
- Clipboard overlay FAB -- copy anything on Android and tap the floating
  button to send it to your Mac instantly.
- Rich notifications on Android with one-tap copy to clipboard.
- TOFU pairing with a one-time six-digit code; all subsequent requests are
  authenticated with Bearer tokens.
- HMAC-SHA256 integrity on every payload.
- Self-signed TLS with SPKI fingerprint pinning (published via mDNS TXT
  record).
- Automatic discovery on LAN via mDNS/Bonjour (`_clipsync._tcp`).
- Tailscale support for syncing outside the local network.
- Foreground service on Android keeps the connection alive.

## Requirements

| Component | Minimum version               |
|-----------|-------------------------------|
| macOS     | 14.0 (Sonoma) or later        |
| Xcode     | 15.0 or later                 |
| Android   | 13 (API 33) or later          |
| JDK       | 17                            |

## Installation

### Mac (build from source)

```bash
# Clone the repository
git clone https://github.com/2cristo7/shared-clipboard.git
cd shared-clipboard

# Build a release archive (output: mac/build/Release/ClipSync.app)
bash mac/scripts/build-release.sh

# Or open in Xcode and build manually
open mac/ClipSync.xcodeproj
```

### Android (build with Gradle)

```bash
cd android

# Debug APK
./gradlew :app:assembleDebug
# Output: app/build/outputs/apk/debug/app-debug.apk

# Release APK (requires signing config)
./gradlew :app:assembleRelease
```

Install the APK on your device via `adb install` or transfer it directly.

## Setup

### 1. Start the Mac server

Launch **ClipSync** from Applications or build output. A menu-bar icon appears.

### 2. Pair the Android client

1. Open ClipSync on Android.
2. Ensure both devices are on the same Wi-Fi network (or connected via
   Tailscale).
3. The Mac will appear automatically via mDNS, or enter the Mac's IP address
   manually.
4. Tap **Pair**. A six-digit code is displayed on the Mac -- enter it on
   Android.
5. Once paired, clipboard sync starts automatically.

### 3. Tailscale (optional)

For syncing outside your local network, install Tailscale on both devices and
use the Mac's Tailscale IP (`100.x.x.x`) in the Android client. See
[docs/tailscale-setup.md](docs/tailscale-setup.md) for detailed instructions.

## Architecture

```
Mac (Server)                          Android (Client)
+---------------------------+         +---------------------------+
| NSPasteboard monitor      |         | ClipboardManager listener |
| Hummingbird HTTPS :7010   | <-WS-> | OkHttp WebSocket client   |
| WebSocket push            |         | Foreground Service        |
| mDNS _clipsync._tcp       |         | NsdManager discovery      |
| Keychain (pairing secret) |         | EncryptedSharedPrefs      |
+---------------------------+         +---------------------------+
```

**Endpoints:**

| Method | Path      | Description                        |
|--------|-----------|------------------------------------|
| GET    | `/health` | Server health check                |
| POST   | `/inject` | Push content to Mac clipboard      |
| GET    | `/pair`   | Exchange pairing code for token    |
| WS     | `/ws`     | Real-time clipboard change stream  |

All requests require a valid Bearer token (obtained during pairing) and an
HMAC-SHA256 signature. The connection uses TLS with SPKI fingerprint pinning.

See [docs/protocol.md](docs/protocol.md) and
[docs/security.md](docs/security.md) for protocol and security details.

## Repository structure

```
mac/        macOS app (Swift, Hummingbird 2.x, SwiftNIO)
android/    Android client (Kotlin, Jetpack Compose, OkHttp)
docs/       Protocol spec, security model, Tailscale guide
```

## Limitations

- Image clipboard sync requires Android 13+ (API 33) due to
  `ClipDescription.getMimeType` restrictions.
- mDNS discovery does not work across separate Tailscale networks (tailnets).
  Use manual IP entry instead.
- The Mac server binds to `0.0.0.0:7010`; only one instance can run at a time.
- No multi-device support yet -- one Android client pairs with one Mac at a
  time.
- Self-signed TLS means browsers will show certificate warnings if you visit
  the server URL directly.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for
details.
