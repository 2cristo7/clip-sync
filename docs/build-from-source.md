# ClipSync — Installation Guide

Complete setup guide for running ClipSync on macOS and Android, including
code signing, Shizuku (for automatic clipboard reading), and Tailscale.

---

## Table of contents

1. [Requirements](#1-requirements)
2. [macOS — build and install](#2-macos--build-and-install)
   - 2.1 [Clone the repository](#21-clone-the-repository)
   - 2.2 [Code-signing setup (`setup-signing.sh`)](#22-code-signing-setup-setup-signingsh)
   - 2.3 [Build the app](#23-build-the-app)
   - 2.4 [First launch and Gatekeeper](#24-first-launch-and-gatekeeper)
3. [Android — build and sideload](#3-android--build-and-sideload)
   - 3.1 [Build the APK](#31-build-the-apk)
   - 3.2 [Enable Developer Options and sideload](#32-enable-developer-options-and-sideload)
   - 3.3 [Required permissions](#33-required-permissions)
4. [Shizuku — automatic clipboard reading](#4-shizuku--automatic-clipboard-reading)
   - 4.1 [What Shizuku is and why ClipSync needs it](#41-what-shizuku-is-and-why-clipsync-needs-it)
   - 4.2 [Install Shizuku](#42-install-shizuku)
   - 4.3 [Activate Shizuku via ADB (one-time)](#43-activate-shizuku-via-adb-one-time)
   - 4.4 [Activate Shizuku via wireless debugging (no USB)](#44-activate-shizuku-via-wireless-debugging-no-usb)
   - 4.5 [Grant ClipSync access](#45-grant-clipsync-access)
5. [Tailscale — remote access outside LAN](#5-tailscale--remote-access-outside-lan)
   - 5.1 [Install Tailscale on macOS](#51-install-tailscale-on-macos)
   - 5.2 [Install Tailscale on Android](#52-install-tailscale-on-android)
   - 5.3 [Configure ClipSync for Tailscale](#53-configure-clipsync-for-tailscale)
6. [First pairing](#6-first-pairing)
7. [Troubleshooting](#7-troubleshooting)

---

## 1. Requirements

| | Minimum | Notes |
|-|---------|-------|
| macOS | 14.0 (Sonoma) | Hummingbird 2.x requires macOS 14 |
| Xcode | 15+ | Tested with Xcode 26 |
| Android | 13 (API 33) | Clipboard access requires Android 13+ |
| Java | JDK 17 | For Gradle builds |
| Tailscale account | Free tier | Only needed for remote use |

Both devices must be on the same Wi-Fi network for LAN use. For remote use, both must be connected to the same Tailscale tailnet.

---

## 2. macOS — build and install

### 2.1 Clone the repository

```bash
git clone https://github.com/2cristo7/clip-sync.git
cd clip-sync
```

### 2.2 Code-signing setup (`setup-signing.sh`)

macOS requires every app to be code-signed before it can run.
The helper script at `mac/scripts/setup-signing.sh` automates this.

```bash
bash mac/scripts/setup-signing.sh
```

**What the script does — three possible outcomes:**

**A) Apple Developer certificate already in Keychain**
The script detects it and exits. Nothing to do — `build-release.sh` will pick it up automatically.

**B) You have an Apple Developer account ($99/year)**
The script offers to open Xcode so you can sign in and create a development certificate. After signing in:
1. Xcode → Settings → Accounts → add your Apple ID
2. Select your team → Manage Certificates → `+` → Apple Development
3. Close Xcode and re-run the script.

An Apple Developer certificate allows you to distribute the app to other Macs and notarize it (removes Gatekeeper warnings for everyone).

**C) No Apple Developer account (personal use)**
The script creates a **local self-signed certificate** using OpenSSL:
- Generates a 2048-bit RSA key + X.509 certificate (valid 10 years)
- Imports it into your login Keychain with code-signing trust
- The app will run on **your Mac only**
- macOS may show a Gatekeeper warning the first time (see section 2.4)

Certificate files are saved to `~/.clipsync-signing/` and can be deleted after import — they are not needed again.

**Non-interactive mode** (for CI or scripting):

```bash
bash mac/scripts/setup-signing.sh --ci
```

This always takes path C (self-signed) without prompting.

### 2.3 Build the app

```bash
xcodebuild \
  -project mac/ClipSync.xcodeproj \
  -scheme ClipSync \
  -configuration Debug \
  -derivedDataPath mac/build \
  build
```

The app bundle is written to `mac/build/Build/Products/Debug/ClipSync.app`.

For a release build:

```bash
bash mac/scripts/build-release.sh
```

### 2.4 First launch and Gatekeeper

If you used a self-signed certificate (path C), macOS Gatekeeper will block the app the first time.

1. Double-click `ClipSync.app` — Gatekeeper shows a warning.
2. Open **System Settings → Privacy & Security**.
3. Scroll to the bottom — you will see `"ClipSync" was blocked…`
4. Click **Open Anyway**.

This is a one-time step. On subsequent launches the app opens normally.

After launch, a clipboard icon appears in the **menu bar**. The embedded HTTP/WebSocket server starts automatically on port `7010`.

Verify it is running:

```bash
curl -s http://127.0.0.1:7010/health | python3 -m json.tool
# { "ok": true, "version": "0.1.0", "platform": "macos" }
```

---

## 3. Android — build and sideload

### 3.1 Build the APK

```bash
cd android
./gradlew assembleDebug
```

The APK is written to:

```
android/app/build/outputs/apk/debug/app-debug.apk
```

### 3.2 Enable Developer Options and sideload

**Enable Developer Options on the phone:**

1. Settings → About phone → tap **Build number** seven times.
2. Go back to Settings → **Developer options**.
3. Enable **USB debugging** (needed for ADB).
4. Also enable **Install via USB** or **Allow install from unknown sources**.

**Install via ADB:**

```bash
adb install android/app/build/outputs/apk/debug/app-debug.apk
```

**Install manually:**
Copy the APK to the phone, open it from Files, and accept the installation prompt.

### 3.3 Required permissions

ClipSync requests several permissions on first launch:

| Permission | Why |
|-----------|-----|
| `FOREGROUND_SERVICE` | Keeps the connection alive when the app is in the background |
| `SYSTEM_ALERT_WINDOW` | Not used in the current release — reserved for future features |
| `POST_NOTIFICATIONS` | Shows incoming clipboard notifications |
| Battery optimisation exemption | Prevents Android from killing the foreground service |


For the **battery exemption**:
Settings → Battery → App battery usage → ClipSync → **Unrestricted**.

---

## 4. Shizuku — automatic clipboard reading

### 4.1 What Shizuku is and why ClipSync needs it

Android 10+ restricts background clipboard access: an app in the background calls `getPrimaryClip()` and gets `null`. This means ClipSync cannot automatically detect when you copy something unless it is the foreground app — which would be useless.

**Shizuku** grants ClipSync a system-level user service that can read the clipboard from the background, enabling 100% automatic sync: copy anything in Chrome, WhatsApp, or any other app, and ClipSync sends it to the Mac without any manual tap.

Without Shizuku, clipboard sync is Mac→Android only; Android→Mac requires Shizuku.

### 4.2 Install Shizuku

Install from the **Google Play Store**:
[Shizuku — Play Store](https://play.google.com/store/apps/details?id=moe.shizuku.privileged.api)

Or from **IzzyOnDroid** (F-Droid):
[Shizuku — IzzyOnDroid](https://apt.izzysoft.de/fdroid/index/apk/moe.shizuku.privileged.api)

### 4.3 Activate Shizuku via ADB (one-time)

Shizuku needs to be started once after each phone reboot. The first time, it must be started via ADB.

1. Enable Developer Options (see section 3.2).
2. Connect the phone to your computer via USB.
3. Verify ADB detects the device:
   ```bash
   adb devices
   # Should list your device as "device", not "unauthorized"
   ```
4. Open the Shizuku app on the phone and tap **"Pairing (Use Wireless debugging)"** or **"Start via ADB"**.
5. Run the command shown in the Shizuku app:
   ```bash
   adb shell sh /sdcard/Android/data/moe.shizuku.privileged.api/start.sh
   ```
6. The Shizuku app shows **"Shizuku is running"** with a green indicator.

> **After every reboot** you need to run the ADB command again (or use wireless debugging, see 4.4). Shizuku does not survive reboots unless you have a rooted device.

### 4.4 Activate Shizuku via wireless debugging (no USB)

Android 11+ supports wireless ADB debugging, so you can start Shizuku without a cable.

1. Settings → Developer options → **Wireless debugging** → Enable.
2. Tap **Pair device with pairing code** — note the IP, port, and code shown.
3. On your Mac:
   ```bash
   adb pair <ip>:<port>
   # Enter the pairing code when prompted
   ```
4. Then connect:
   ```bash
   adb connect <ip>:<port shown in "Wireless debugging" screen>
   ```
5. Run the Shizuku start command:
   ```bash
   adb shell sh /sdcard/Android/data/moe.shizuku.privileged.api/start.sh
   ```

After the initial pairing, subsequent reconnections only need steps 4 and 5.

### 4.5 Grant ClipSync access

Once Shizuku is running:

1. Open ClipSync on Android.
2. Go to **Settings → Clipboard mode**.
3. Tap **Enable Shizuku**.
4. A Shizuku permission dialog appears — tap **Allow**.

ClipSync will now automatically detect clipboard changes and sync them to the Mac.

---

## 5. Tailscale — remote access outside LAN

Tailscale creates a private encrypted mesh network (WireGuard-based) between your devices, letting ClipSync work when the Mac and the phone are on different networks.

> **Note:** mDNS auto-discovery does not work over Tailscale because WireGuard tunnels do not forward multicast traffic. You must use manual IP entry (section 5.3).

### 5.1 Install Tailscale on macOS

**Option A — Homebrew (recommended for developers)**

```bash
brew install tailscale
sudo tailscaled install-system-daemon   # installs as a launchd service
tailscale up
```

**Option B — Mac App Store**

Search "Tailscale" in the Mac App Store or visit [tailscale.com/download/mac](https://tailscale.com/download/mac).

Sign in with your Tailscale account. The status bar icon shows when you are connected.

Find your Mac's Tailscale IP:

```bash
tailscale ip -4
# Example: 100.64.0.2
```

### 5.2 Install Tailscale on Android

Install **Tailscale** from the Google Play Store.

Open the app and sign in with the **same Tailscale account** (or an account on the same tailnet). Toggle **Connected**.

Verify the tunnel from the Mac:

```bash
tailscale ping <android-tailscale-ip>
# Should show: pong from <device> via ...
```

### 5.3 Configure ClipSync for Tailscale

1. On the Mac, make sure ClipSync is running (clipboard icon in the menu bar).
2. On Android, open ClipSync → **Settings**.
3. Set **Connection mode** to **Tailscale / Manual IP**.
4. Enter the Mac's Tailscale IP (e.g. `100.64.0.2`) and port `7010`.
5. Tap **Connect**.

Pair as usual (section 6). TLS and HMAC work identically over Tailscale.

**MagicDNS (optional):** If MagicDNS is enabled on your tailnet, you can use the Mac's hostname (`macbook.tailabcdef.ts.net`) instead of a raw IP. The TLS certificate CN will not match this hostname, but SPKI fingerprint pinning bypasses hostname validation, so it works without changes.

**Firewall:** If the macOS Application Firewall is enabled, allow ClipSync:
System Settings → Network → Firewall → Firewall Options → `+` → select ClipSync.

---

## 6. First pairing

Pairing links the Android app to the Mac using a one-time 6-digit code (TOFU — trust on first use).

1. **On the Mac** — click the ClipSync menu bar icon → **Pair new device**.
   A window shows a 6-digit code and a QR code (valid for 5 minutes).

2. **On Android** — open ClipSync → **Pair**.
   Either scan the QR code or type the 6-digit code manually.

3. The app exchanges a bearer token and HMAC secret over TLS.
   The Android foreground-service notification changes to **"Connected"**.

4. **Test it** — copy any text on the Mac. A notification should appear on Android within 1–2 seconds.

> Pairing only needs to be done once. The token is stored in the Mac Keychain and Android EncryptedSharedPreferences and persists across restarts.

---

## 7. Troubleshooting

### Android cannot discover the Mac

- Make sure both devices are on **the same Wi-Fi network**.
- Verify the Mac's server is running: `curl http://127.0.0.1:7010/health`
- Check that mDNS is broadcasting: `dns-sd -B _clipsync._tcp`
- If using Tailscale, mDNS will not work — use manual IP (section 5.3).

### Pairing code expires before I can enter it

The code is valid for 5 minutes. If it expires, close and reopen the Pair window to generate a new one.

### Clipboard does not sync automatically on Android

- Without Shizuku: clipboard sync is Mac→Android only (copy on Mac, paste on Android). Android→Mac requires Shizuku.
- With Shizuku: verify Shizuku is running (green indicator in the Shizuku app). After a reboot, re-run the ADB start command (section 4.3 step 5).
- Check that ClipSync has battery optimisation disabled (section 3.3).

### macOS firewall blocks the connection

System Settings → Network → Firewall → Firewall Options → add ClipSync. If using `pf`:

```bash
echo "pass in proto tcp from any to any port 7010" | sudo pfctl -a clipsync -f -
```

### WebSocket disconnects frequently over Tailscale

ClipSync reconnects automatically via `NetworkChangeObserver` (Android) and `ReachabilityMonitor` (Mac). If reconnection takes more than 30 seconds, run `tailscale status` to confirm the tunnel is up.

### Shizuku stops after reboot

This is expected — Shizuku does not survive reboots on non-rooted devices. Re-run the ADB start command:

```bash
adb shell sh /sdcard/Android/data/moe.shizuku.privileged.api/start.sh
```

To automate this, you can add the command to a startup script on your Mac that runs when the phone connects via ADB.
