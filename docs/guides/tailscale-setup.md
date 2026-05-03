# ClipSync over Tailscale -- End-to-End Setup Guide

ClipSync uses mDNS (Bonjour / NSD) for automatic device discovery on a
local network. mDNS relies on multicast UDP, which **does not work across
Tailscale's WireGuard tunnels** -- multicast packets are not forwarded
between tailnet peers. This means automatic discovery will not find
devices connected only via Tailscale.

The solution is straightforward: use **Manual IP entry** on the Android
client, pointing it at the Mac's Tailscale IP (`100.x.x.x`). TLS
self-signed + SPKI pinning and Bearer token auth work identically over
Tailscale since they are transport-layer agnostic.

---

## Prerequisites

| Item | Requirement |
|------|-------------|
| Mac  | macOS 14.0+, ClipSync built and running |
| Android | Android 13 (API 33)+, ClipSync installed |
| Tailscale account | Free tier is sufficient |

---

## 1. Install Tailscale

### macOS

**Option A -- Homebrew (CLI only)**

```bash
brew install tailscale
sudo tailscaled install-system-daemon   # installs the launchd service
tailscale up
```

**Option B -- Mac App Store / official app**

Download from <https://tailscale.com/download/mac> or search "Tailscale"
in the Mac App Store. Sign in and toggle "Connected".

### Android

Install **Tailscale** from the Google Play Store:
<https://play.google.com/store/apps/details?id=com.tailscale.ipn>

Open the app, sign in with the same account (or the same tailnet).

---

## 2. Verify tailnet connectivity

On the Mac, find your Tailscale IP:

```bash
tailscale ip -4
# Example output: 100.64.0.2
```

On Android, open the Tailscale app and note the device's IP (e.g.
`100.64.0.3`).

Verify reachability from the Mac:

```bash
tailscale ping <android-tailscale-ip>
# Should show "pong from <device> (...) via DERP(...)" or "... via <direct>"
```

If `tailscale ping` succeeds, the WireGuard tunnel is up and both
devices are authorized on the tailnet.

---

## 3. Configure ClipSync

### 3.1 Start ClipSync on Mac

Launch ClipSync as usual. The HTTP/WebSocket server listens on
`0.0.0.0:<port>` (default 7010), so it is reachable on all interfaces
including the Tailscale `utun` interface.

### 3.2 Connect from Android -- Manual IP mode

1. Open ClipSync on Android.
2. Go to **Settings**.
3. Switch discovery mode to **Manual**.
4. Enter the Mac's Tailscale IP (e.g. `100.64.0.2`) and port (`7010`).
5. Tap **Pair** and enter the 6-digit pairing code displayed on the Mac.

Pairing will complete over TLS. The SPKI fingerprint is pinned on first
connect (TOFU), and the bearer token + HMAC secret are exchanged inside
the encrypted `/pair` response.

### 3.3 Verify the connection

- The Android foreground-service notification should show
  **"Connected (100.64.0.2)"**.
- Copy text on the Mac -- a notification should appear on Android within
  1-2 seconds.
- Share content from Android (e.g. via Chrome Share) -- it should land in
  the Mac's pasteboard.

---

## 4. MagicDNS (optional)

If MagicDNS is enabled on your tailnet (Tailscale admin console >
DNS > MagicDNS), you can use the device's MagicDNS name instead of a
raw IP:

```
<hostname>.tail<tailnet-name>.ts.net
```

For example: `macbook.tailabcdef.ts.net`. Use this as the manual host in
the Android ClipSync settings. Note that the TLS certificate's CN will
not match this hostname (it is self-signed for `ClipSync`), but SPKI
pinning bypasses hostname validation, so this works without changes.

---

## 5. Troubleshooting

### mDNS does not discover the Mac

**Expected behavior.** Tailscale tunnels do not forward multicast
traffic. Use Manual IP entry as described in section 3.2.

On a true LAN (same Wi-Fi), mDNS works normally. If it does not:
- Ensure both devices are on the same subnet.
- Check that `_clipsync._tcp` is being advertised (`dns-sd -B _clipsync._tcp`).

### macOS firewall blocks connections

macOS has two firewall layers:

**Application Firewall (System Settings)**

System Settings > Network > Firewall. Either:
- Disable it entirely (not recommended), or
- Add ClipSync to the allowed list: Firewall Options > "+" > select
  ClipSync.app.

**Packet Filter (`pf`)**

Less common, but if active:

```bash
# Check if pf is enabled
sudo pfctl -s info | head -5

# If rules block port 7010, add an allow rule:
echo "pass in proto tcp from any to any port 7010" | sudo pfctl -a clipsync -f -
```

### `tailscale ping` fails

- Verify both devices are signed in to the same tailnet.
- Check the Tailscale admin console (<https://login.tailscale.com/admin/machines>)
  and ensure both devices are **authorized** (not expired / pending).
- If using an ACL policy, ensure it allows traffic between the two
  devices on the ClipSync port (default 7010).
- Try `tailscale netcheck` on both devices to diagnose DERP/relay
  connectivity.

### WebSocket disconnects frequently

- Tailscale's WireGuard tunnel can briefly drop when switching between
  Wi-Fi and cellular. ClipSync's `NetworkChangeObserver` (Android) and
  `ReachabilityMonitor` (Mac) detect these transitions and trigger
  automatic reconnection with exponential backoff.
- If reconnection takes longer than 30 seconds, check `tailscale status`
  to confirm the tunnel is up.

### Android loses connection when switching to mobile data

- This is expected during the network transition. The foreground service
  detects the network change via `ConnectivityManager.NetworkCallback`
  and triggers an immediate reconnect (backoff resets to 1 second).
- Ensure Tailscale has the "Run as VPN" permission on Android and is not
  being battery-optimized (Settings > Battery > ClipSync and Tailscale >
  Unrestricted).

---

## 6. Tested scenarios

> **Note**: The scenarios below require two physical devices and cannot be
> fully automated. They are documented here as a manual QA checklist.

| # | Scenario | Expected result | Status |
|---|----------|-----------------|--------|
| 1 | Mac on Wi-Fi, Android on same Wi-Fi via Tailscale IP | Pair + clipboard sync works | Placeholder |
| 2 | Mac on Wi-Fi, Android on mobile data (5G), both on Tailscale | Pair via manual IP, clipboard sync works | Placeholder |
| 3 | Android switches from Wi-Fi to mobile data mid-session | WebSocket reconnects within ~5s, sync resumes | Placeholder |
| 4 | Mac's Tailscale is stopped and restarted | Android shows "Disconnected", then reconnects | Placeholder |
| 5 | Share from Android browser to Mac over Tailscale | Content appears in Mac pasteboard | Placeholder |
| 6 | Mac copies image, Android receives notification over Tailscale | Image notification with "Apply" action works | Placeholder |

---

## 7. Known limitations

1. **No mDNS across Tailscale.** Multicast is not routed through
   WireGuard tunnels. Manual IP entry is required. If Tailscale adds
   multicast support in the future, automatic discovery would work
   without changes.

2. **MagicDNS hostname vs TLS CN mismatch.** The self-signed TLS
   certificate uses a generic CN. This is not a problem because ClipSync
   uses SPKI fingerprint pinning (not hostname-based validation), but it
   means standard browser-based HTTPS requests to the MagicDNS name will
   show a certificate warning.

3. **DERP relay latency.** If a direct WireGuard connection cannot be
   established (e.g. symmetric NATs on both sides), Tailscale falls back
   to DERP relays. Clipboard sync still works but latency may increase
   to 100-300ms depending on the relay location. Run `tailscale netcheck`
   to see preferred DERP region.

4. **Battery impact.** Running both Tailscale VPN and ClipSync foreground
   service on Android increases battery usage. Consider stopping ClipSync
   when not needed.

5. **Tailscale ACLs.** If your tailnet uses Access Control Lists, ensure
   port 7010 (or your configured ClipSync port) is allowed between the
   Mac and Android devices.
