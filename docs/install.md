# Install ClipSync

Pre-built binaries for macOS and Android are attached to every [GitHub Release](https://github.com/2cristo7/clip-sync/releases).

---

## One-liner (macOS)

Open **Terminal** and run:

```bash
curl -fsSL https://raw.githubusercontent.com/2cristo7/clip-sync/main/install.sh | bash
```

The script downloads the latest `.dmg`, mounts it, copies `ClipSync.app` to `/Applications`, and cleans up. No build tools required.

---

## Manual install

### macOS

1. Go to the [latest release](https://github.com/2cristo7/clip-sync/releases/latest) and download **`ClipSync-x.x.x.dmg`**.
2. Open the DMG — a window appears with `ClipSync.app` and an `Applications` shortcut.
3. Drag `ClipSync.app` into `Applications`.
4. On first launch, macOS will block the app (unsigned binary). Open **System Settings → Privacy & Security**, scroll down, and click **Open Anyway**.
5. ClipSync appears as a menu-bar icon. Click it to start pairing.

### Android

1. Go to the [latest release](https://github.com/2cristo7/clip-sync/releases/latest) and download **`ClipSync-x.x.x.apk`**.
2. Transfer the APK to your phone (AirDrop, cable, email, etc.).
3. On your Android: open the APK file. If prompted, enable **Install unknown apps** for the app you used to open it (Files, Chrome, etc.).
4. Tap **Install**.

> **Android 13+:** You may need to go to **Settings → Apps → Special app access → Install unknown apps** and allow the source app.

#### Install via ADB (optional, faster)

If you have ADB set up on your computer and your phone connected via USB or wireless debugging:

```bash
adb install ClipSync-x.x.x.apk
```

---

## After installing

1. **Mac** — click the ClipSync icon in the menu bar → **Pair Android device**.
2. **Android** — open ClipSync, tap **Pair with Mac**, scan the QR code shown on the Mac (or enter the 6-digit code manually).
3. Both apps confirm the pairing. Clipboard sync starts immediately.

For Shizuku setup (required for Android→Mac clipboard reading), see the [full installation guide](installation.md#4-shizuku--automatic-clipboard-reading).  
For Tailscale (sync outside your home network), see [guides/tailscale-setup.md](guides/tailscale-setup.md).

---

## Requirements

| Platform | Minimum version |
|----------|----------------|
| macOS    | 14.0 (Sonoma)  |
| Android  | 13 (API 33)    |

---

## Uninstall

**macOS:** drag `/Applications/ClipSync.app` to Trash. To also remove stored secrets:

```bash
security delete-generic-password -s "com.clipsync.auth-token"
security delete-generic-password -s "com.clipsync.pairing-secret"
security delete-certificate -c "ClipSync"
```

**Android:** long-press the ClipSync icon → **Uninstall**.
