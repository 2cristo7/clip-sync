//! Cryptographic primitives for ClipSync: HMAC signing/verification,
//! self-signed TLS identity (cert + key generation/persistence),
//! SPKI-SHA256 fingerprints and the on-disk hashed token store.
//!
//! Pure crypto/identity primitives — nothing here knows about the wire
//! format or any specific transport. Depends on `clipsync-protocol`
//! only for non-tunable constants like `TLS_CERT_VALIDITY_DAYS`.

pub mod fingerprint;
pub mod hmac;
pub mod tls;
pub mod token_store;
