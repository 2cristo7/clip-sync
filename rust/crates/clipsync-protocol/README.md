# clipsync-protocol

Wire-format types and protocol-level constants shared by every ClipSync
transport. Holds [`ClipPayload`], the pairing state machine, HMAC/payload
clock-skew bounds and protocol identifiers (`PORT`, `VERSION`,
`MAX_PAYLOAD_BYTES`, `MDNS_SERVICE_TYPE`, etc.). This crate is
transport-agnostic: it knows nothing about WebSockets, TLS or
clipboards. Mac, Android and Rust apps must agree on its types or the
bytes on the wire diverge.
