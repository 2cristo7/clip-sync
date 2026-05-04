//! Integration tests for `clipsync_server::startup`.
//!
//! Phase 1.6: when ClipSync starts and its configured port is already taken
//! by another process, `try_bind` must surface a specific `PortInUse` error
//! (rather than a generic Tokio bind panic) so `main.rs` can log a clean
//! message and exit with status 2.
//!
//! These tests grab a free port via `TcpListener::bind("127.0.0.1:0")`,
//! keep the listener alive, then call `try_bind` against the same port and
//! assert on the error variant — no subprocess, no flakiness.

use std::net::SocketAddr;

use clipsync_server::startup::{try_bind, StartupError};

/// Bind a kernel-assigned port and return both the live listener (kept
/// alive by the caller for the duration of the test) and its `SocketAddr`.
async fn occupy_free_port() -> (tokio::net::TcpListener, SocketAddr) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("kernel should always be able to assign a free port");
    let addr = listener.local_addr().expect("listener has a local addr");
    (listener, addr)
}

#[tokio::test]
async fn try_bind_reports_port_in_use_when_port_already_taken() {
    let (_held, addr) = occupy_free_port().await;

    let result = try_bind(addr).await;

    match result {
        Err(StartupError::PortInUse(port)) => {
            assert_eq!(
                port,
                addr.port(),
                "PortInUse should carry the conflicting port"
            );
        }
        Err(StartupError::Io(e)) => panic!(
            "expected PortInUse, got generic IO error: {e} (kind={:?})",
            e.kind()
        ),
        Ok(_) => panic!("expected bind failure on already-occupied port {addr}, got Ok"),
    }
}

#[tokio::test]
async fn try_bind_port_in_use_display_matches_log_message() {
    // The exact wording is asserted because `main.rs` logs it for grep-ability
    // and downstream tooling (and the master plan) keys on it.
    let err = StartupError::PortInUse(54321);
    assert_eq!(
        err.to_string(),
        "port 54321 is already in use; another ClipSync instance may be running"
    );
}

#[tokio::test]
async fn try_bind_succeeds_on_free_port() {
    // Happy-path regression check: binding to a fresh kernel-assigned port
    // (port 0) must still return Ok.
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = try_bind(addr).await.expect("free port should bind");
    let bound = listener.local_addr().expect("listener has a local addr");
    assert_ne!(bound.port(), 0, "kernel should have assigned a real port");
}
