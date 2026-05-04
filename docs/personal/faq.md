# Frequently Asked Questions

## "No devices found"

Make sure both devices are on the same Wi-Fi network. ClipSync uses local network discovery, which requires devices to be able to see each other.

If you're behind a firewall, allow port **5195/tcp** (ClipSync's default).

Still stuck? Use **Add by IP** on the home screen and type the other device's local IP address.

## How do I use ClipSync over Tailscale?

Tailscale creates a private network between your devices, even across the internet. Since Tailscale doesn't support local discovery (no multicast), you'll need to add peers manually:

1. Install Tailscale on both devices
2. In ClipSync, tap **Add device** → **Add by IP**
3. Enter the other device's Tailscale IP (find it in the Tailscale app)

## How do I uninstall?

- **Mac**: Drag ClipSync from Applications to the Trash
- **Windows**: Settings → Apps → ClipSync → Uninstall
- **Linux AppImage**: Delete the AppImage file
- **Linux deb**: `sudo dpkg -r clipsync`

## Is my clipboard data encrypted?

Yes. All clipboard data is authenticated with HMAC and never leaves your local network unless you explicitly set up Tailscale. No data ever goes through our servers — there are no servers.

## Can I use it with the Mac native app?

Yes! ClipSync Personal and the native Mac app speak the same protocol. They can pair with each other, so you can run whichever you prefer on Mac while using ClipSync Personal on Windows/Linux.

## How many devices can I connect?

ClipSync works best with 2–5 devices. It's been tested with up to 10 peers on the same network. Beyond that, you may notice slightly higher CPU use.
