# clipsync-crypto

Cryptographic primitives and identity material for ClipSync: HMAC-SHA256
request signing/verification, self-signed EC P-256 TLS identity
(generate / persist / load + rustls `ServerConfig` and `ClientConfig`),
SPKI-SHA256 certificate fingerprints (used for TOFU pairing) and the
on-disk hashed token store. Transport-agnostic — depends on
`clipsync-protocol` for shared constants only.
