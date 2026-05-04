# payload_v1_ms.json

Golden vector: ClipPayload v1 wire shape with `ts` in **milliseconds**.

- Source of truth: Mac Swift `ClipPayload.swift` post-fix commit `f615624e`
  (timestamps stored and serialized as milliseconds since epoch).
- Mirror of `rust/tests/compat/payload_v1.json` (kept for backward compatibility);
  this file is the explicitly-named canonical name introduced by Phase 1.8.
- Bytes are produced by `JSONEncoder` on Mac with default key ordering
  (struct-declaration order on Swift; serde mirrors it on Rust).

## Invariants

- `ts` is `Int64` Unix milliseconds (>10^12). Never seconds.
- `data` is **standard** base64 (alphabet `+/`, padding `=`). Not base64url.
- `nonce` is a UUID string.
- `name` may be `null` (text/image) or a string (file).

See `CLAUDE.md` §"Wire Protocol Invariants" and `docs/architecture/protocol.md`.
