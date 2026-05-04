# clipsync-clipboard

Cross-platform clipboard plumbing for ClipSync. Wraps `arboard` with the
per-OS quirks (TIFF→PNG conversion on macOS, image file save handling,
echo-suppression digests, native received-clip notifications) the Rust
apps need plus a polling watcher built on `CLIPBOARD_POLL_MS`. Depends
on `clipsync-protocol` for `ClipPayload` / `ClipType`.
