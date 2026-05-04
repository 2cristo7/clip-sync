# ClipSync Enterprise — Installation Guide

This guide covers installing the ClipSync Enterprise server, the admin dashboard, and desktop clients on all supported platforms.

---

## Prerequisites

- Network connectivity between all machines (LAN or Tailscale)
- Admin/root privileges on the server machine
- Port **7010** open for HTTP/WebSocket traffic (or your configured port)

---

## 1. Server Installation

The ClipSync Enterprise server is the central hub that brokers clipboard sync, enforces policies, and stores audit logs.

### Linux (Ubuntu / Debian)

1. Download the `.deb` package from the [Releases](https://github.com/AstidProjects/clip-sync/releases) page:

   ```bash
   curl -LO https://github.com/AstidProjects/clip-sync/releases/latest/download/clipsync-enterprise-server.deb
   ```

2. Install the package:

   ```bash
   sudo dpkg -i clipsync-enterprise-server.deb
   ```

3. Enable and start the service:

   ```bash
   sudo systemctl enable --now clipsync-enterprise
   ```

4. Verify the server is running:

   ```bash
   curl http://localhost:7010/health
   ```

   Expected response:

   ```json
   { "status": "ok" }
   ```

> **Tip:** The default configuration file is installed at `/etc/clipsync/config.toml`. See the [Admin Guide](admin-guide.md) for configuration options.

### Windows

1. Download `clipsync-enterprise-server.msi` from the [Releases](https://github.com/AstidProjects/clip-sync/releases) page.

2. Run the MSI installer and follow the wizard. The installer registers ClipSync Enterprise as a Windows service.

3. The service starts automatically after installation. Verify by opening PowerShell:

   ```powershell
   Invoke-RestMethod http://localhost:7010/health
   ```

   Expected response:

   ```json
   { "status": "ok" }
   ```

> **Note:** The configuration file is located at `C:\ProgramData\ClipSync\config.toml`.

### macOS

1. Download `clipsync-enterprise-server.dmg` from the [Releases](https://github.com/AstidProjects/clip-sync/releases) page.

2. Open the DMG and drag **ClipSync Enterprise Server** to the Applications folder.

3. Start the server using `launchctl`:

   ```bash
   sudo launchctl load /Library/LaunchDaemons/com.clipsync.enterprise.plist
   ```

   Or start it manually from the terminal:

   ```bash
   /Applications/ClipSync\ Enterprise\ Server.app/Contents/MacOS/clipsync-server
   ```

4. Verify the server is running:

   ```bash
   curl http://localhost:7010/health
   ```

   Expected response:

   ```json
   { "status": "ok" }
   ```

> **Note:** The configuration file is located at `/etc/clipsync/config.toml` (system-wide) or `~/Library/Application Support/ClipSync/config.toml` (per-user).

---

## 2. Admin Dashboard Installation

The admin dashboard provides a graphical interface for managing devices, policies, broadcast, and audit logs.

### macOS

1. Download `clipsync-dashboard.dmg` from the [Releases](https://github.com/AstidProjects/clip-sync/releases) page.
2. Open the DMG and drag **ClipSync Dashboard** to the Applications folder.
3. Launch the app. On first launch you will be prompted to enter:
   - **Server URL** — the address of your ClipSync Enterprise server (e.g. `http://192.168.1.100:7010`)
   - **Admin token** — generated during server first-start and printed to the server log

### Windows

1. Download `clipsync-dashboard.msi` from the [Releases](https://github.com/AstidProjects/clip-sync/releases) page.
2. Run the MSI installer and follow the wizard.
3. Launch **ClipSync Dashboard** from the Start Menu.
4. Enter the **Server URL** and **Admin token** on first launch.

### Linux

1. Download `clipsync-dashboard.AppImage` from the [Releases](https://github.com/AstidProjects/clip-sync/releases) page.
2. Make it executable and run:

   ```bash
   chmod +x clipsync-dashboard.AppImage
   ./clipsync-dashboard.AppImage
   ```

3. Enter the **Server URL** and **Admin token** on first launch.

---

## 3. Client Installation

Clients are the end-user applications that sync clipboard content through the enterprise server.

### macOS

1. Download `clipsync-client.dmg` from the [Releases](https://github.com/AstidProjects/clip-sync/releases) page.
2. Open the DMG and drag **ClipSync** to the Applications folder.
3. Launch the app. Enter the **Server URL** provided by your administrator.
4. The pairing process completes automatically once the server accepts the device.

### Windows

1. Download `clipsync-client.msi` from the [Releases](https://github.com/AstidProjects/clip-sync/releases) page.
2. Run the MSI installer and follow the wizard.
3. Launch **ClipSync** from the Start Menu.
4. Enter the **Server URL** and complete pairing.

### Linux

1. Download `clipsync-client.AppImage` from the [Releases](https://github.com/AstidProjects/clip-sync/releases) page.
2. Make it executable and run:

   ```bash
   chmod +x clipsync-client.AppImage
   ./clipsync-client.AppImage
   ```

3. Enter the **Server URL** and complete pairing.

---

## Verifying the Installation

After installing the server, dashboard, and at least one client:

1. Open the admin dashboard and confirm the server status shows **Connected**.
2. Check the **Devices** tab — the paired client should appear in the device list.
3. Copy text on the client machine and verify it appears on other paired devices.

---

## Troubleshooting

| Symptom | Likely Cause | Fix |
|---|---|---|
| `curl` to `/health` times out | Server not running or port blocked | Check `systemctl status clipsync-enterprise` (Linux) or firewall rules |
| Dashboard cannot connect | Wrong server URL or admin token | Re-check the URL and token from the server log |
| Client pairing fails | Server unreachable from client network | Verify network connectivity and that port 7010 is open |
| Service fails to start on Linux | Missing dependency | Run `sudo apt-get install -f` to resolve dependencies |

For further help, see the [Admin Guide](admin-guide.md) or file an issue on GitHub.
