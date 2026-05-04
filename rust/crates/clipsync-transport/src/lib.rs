//! Transport-layer building blocks for ClipSync: mDNS service
//! advertisement and discovery, plus the WebSocket/healthcheck tuning
//! constants (`WS_PING_INTERVAL`, `WS_READ_TIMEOUT`,
//! `HEALTHCHECK_POLL_INTERVAL`, `CONSECUTIVE_FAILURE_THRESHOLD`) that
//! both the server hub and the client connector must agree on. Future
//! work: lift the WebSocket hub itself in here once apps are split.

pub mod config;
pub mod mdns;
