//! Clipboard-polling tuning.

/// Cadence (milliseconds) at which the clipboard watcher polls
/// `arboard` for a change. 500 ms balances responsiveness against
/// CPU/IO cost; mirrors the Mac/Android cadence.
pub const CLIPBOARD_POLL_MS: u64 = 500;
