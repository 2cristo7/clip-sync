# ClipSync Enterprise — Admin Guide

This guide walks administrators through day-to-day management of a ClipSync Enterprise deployment using the admin dashboard and server configuration.

---

## Getting Started

After completing the steps in the [Installation Guide](installation.md), open the admin dashboard and sign in with your server URL and admin token.

---

## Pairing Your First Device

1. Install the ClipSync Enterprise server on your chosen host and verify it is running (see [Installation Guide](installation.md)).
2. Install the ClipSync client on another machine.
3. Launch the client and enter the **Server URL** (e.g. `http://192.168.1.100:7010`).
4. The client sends a pairing request to the server. The server accepts it and issues a device token.
5. Open the admin dashboard — the new device appears under **Devices** with a status of **Paired**.

> **Note:** In deployments with auto-approve disabled, the admin must manually approve pairing requests from the dashboard before the device can sync.

---

## Managing Device Policies

Policies control what a device is allowed to sync (text, images, files) and in which direction.

1. Open the admin dashboard.
2. Navigate to **Devices**.
3. Click the device you want to configure.
4. Use the **Policy** dropdown to select a policy:
   - **Full Sync** — bidirectional sync of all content types.
   - **Text Only** — sync text content only; images and files are blocked.
   - **Receive Only** — device can receive clipboard content but cannot push.
   - **Send Only** — device can push clipboard content but does not receive.
   - **Disabled** — sync is suspended for this device.
5. Click **Save**. The policy takes effect immediately.

> **Tip:** You can select multiple devices and apply a policy in bulk using the **Bulk Actions** menu.

---

## Broadcasting Content

Broadcast lets an admin push a file or clipboard payload to one or more devices simultaneously.

1. Open the admin dashboard.
2. Navigate to **Broadcast**.
3. Drop a file into the upload area, or paste text into the content field.
4. Select the target devices or device groups.
5. Click **Send**.
6. A confirmation dialog shows the number of targets. Confirm to broadcast.

The broadcast status appears in the activity feed. Failed deliveries are retried automatically up to three times.

---

## Viewing the Audit Log

The audit log records all sync events, pairing actions, policy changes, and admin operations.

1. Open the admin dashboard.
2. Navigate to **Audit**.
3. Use the filters to narrow results:
   - **Date range** — select start and end dates.
   - **Device** — filter by a specific device name or ID.
   - **Event type** — filter by event category (e.g. `sync`, `pair`, `policy_change`, `broadcast`).
4. Click **Apply Filters** to refresh the log.
5. To export, click **Export CSV**. The exported file includes all rows matching the current filters.

> **Privacy:** Audit entries record metadata (timestamps, device IDs, content types, byte sizes) but never store raw clipboard text. See the [Security Guide](security.md) for details.

---

## Server Configuration

The server reads its configuration from a TOML file. The default location depends on the platform:

| Platform | Default Path |
|---|---|
| Linux | `/etc/clipsync/config.toml` |
| macOS | `/etc/clipsync/config.toml` |
| Windows | `C:\ProgramData\ClipSync\config.toml` |

You can override the path with the `--config` flag:

```bash
clipsync-server --config /path/to/custom-config.toml
```

### Key Configuration Options

```toml
[server]
bind_address = "0.0.0.0"
port = 7010

[tls]
enabled = false
cert_path = "/etc/clipsync/certs/server.crt"
key_path = "/etc/clipsync/certs/server.key"

[auth]
auto_approve_pairing = true
token_expiry_days = 90

[audit]
audit_retention_days = 30

[storage]
data_dir = "/var/lib/clipsync"
```

### Configuration Reference

| Key | Type | Default | Description |
|---|---|---|---|
| `server.bind_address` | string | `"0.0.0.0"` | IP address to bind the server to |
| `server.port` | integer | `7010` | Port for HTTP and WebSocket traffic |
| `tls.enabled` | boolean | `false` | Enable TLS encryption |
| `tls.cert_path` | string | — | Path to the TLS certificate file |
| `tls.key_path` | string | — | Path to the TLS private key file |
| `auth.auto_approve_pairing` | boolean | `true` | Automatically approve new device pairing requests |
| `auth.token_expiry_days` | integer | `90` | Days before a device token expires |
| `audit.audit_retention_days` | integer | `30` | Days to retain audit log entries before pruning |
| `storage.data_dir` | string | platform-dependent | Directory for server data files |

After editing the config file, restart the server for changes to take effect:

```bash
# Linux
sudo systemctl restart clipsync-enterprise

# macOS
sudo launchctl unload /Library/LaunchDaemons/com.clipsync.enterprise.plist
sudo launchctl load /Library/LaunchDaemons/com.clipsync.enterprise.plist
```

---

## User and Device Management

### Revoking a Device

1. Navigate to **Devices** in the dashboard.
2. Click the device to revoke.
3. Click **Revoke**. The device token is invalidated immediately.
4. The device must re-pair to regain access.

### Viewing Device Details

Each device entry in the dashboard shows:

- **Name** — hostname or user-assigned label.
- **Platform** — operating system (macOS, Windows, Linux).
- **Last Seen** — timestamp of last activity.
- **Policy** — current sync policy.
- **Status** — Paired, Pending, or Revoked.

---

## Monitoring

The server exposes a health endpoint for monitoring integrations:

```
GET /health
```

Returns `200 OK` with `{ "status": "ok" }` when the server is operational.

This endpoint can be polled by uptime monitors, load balancers, or orchestration tools.
