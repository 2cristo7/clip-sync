//! Server bootstrap helpers.
//!
//! This module factors out the parts of startup that benefit from being
//! testable without spawning a subprocess — currently the TCP bind step.
//!
//! Background: `tokio::net::TcpListener::bind` returns a generic `io::Error`
//! on failure. We want a specific, user-actionable message when the failure
//! mode is "port already in use" (typically: another ClipSync instance is
//! already running). The rest of the bootstrap (TLS, token store, etc.) is
//! still handled inline in `main.rs`.
//!
//! See `docs/plans/master-plan-rust-fork.md` Phase 1.6.

use std::io;
use std::net::SocketAddr;

use tokio::net::TcpListener;

/// Errors that can occur during the early bootstrap phase of the server.
///
/// Intentionally narrow: today this only models bind-time failures. Other
/// startup paths (TLS load, token store load, …) `process::exit(1)` inline
/// in `main.rs` because they're already handled with specific messages.
#[derive(Debug)]
pub enum StartupError {
    /// The configured port is already bound by another process.
    ///
    /// This is reported separately from generic IO errors so the binary can
    /// emit a helpful, grep-able message and exit with a distinct status
    /// code (see `main.rs`).
    PortInUse(u16),
    /// Any other IO error returned by `TcpListener::bind`.
    Io(io::Error),
}

impl std::fmt::Display for StartupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PortInUse(port) => write!(
                f,
                "port {port} is already in use; another ClipSync instance may be running"
            ),
            Self::Io(e) => write!(f, "bind failed: {e}"),
        }
    }
}

impl std::error::Error for StartupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PortInUse(_) => None,
            Self::Io(e) => Some(e),
        }
    }
}

/// Attempt to bind a `TcpListener` to `addr`.
///
/// Returns:
/// - `Ok(listener)` on success.
/// - `Err(StartupError::PortInUse(port))` when the OS reports
///   `io::ErrorKind::AddrInUse` for the requested address.
/// - `Err(StartupError::Io(e))` for any other bind failure.
///
/// This wrapper exists so `main.rs` can map `PortInUse` to a clean message
/// and `process::exit(2)` instead of bubbling up a generic Tokio error,
/// and so integration tests can assert on the error variant without
/// shelling out to the binary.
pub async fn try_bind(addr: SocketAddr) -> Result<TcpListener, StartupError> {
    match TcpListener::bind(addr).await {
        Ok(listener) => Ok(listener),
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => Err(StartupError::PortInUse(addr.port())),
        Err(e) => Err(StartupError::Io(e)),
    }
}
