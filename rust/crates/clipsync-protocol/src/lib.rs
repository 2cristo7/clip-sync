//! Wire-format types and protocol-level constants for ClipSync.
//!
//! This crate is the single source of truth for everything that
//! crosses the wire between Mac, Android and the Rust apps:
//! [`ClipPayload`], pairing state machine, and HMAC/payload skew
//! constants. Anything in here is shared by every transport; nothing
//! in here knows about WebSockets, mDNS or clipboards.
//!
//! See `docs/plans/master-plan-rust-fork.md` Phase 1.9 for the
//! workspace split rationale.

pub mod config;
pub mod handshake;
pub mod pairing;
pub mod protocol;
