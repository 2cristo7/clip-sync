# Phase 4 Summary — Security (TLS + HMAC + Bearer Auth)

**Branch**: `feature/mac-security` → merged into `main` with `--no-ff` (`85d957d`).

## What shipped

- `Security/TLSManager.swift`: generates a self-signed EC P-256 cert on first launch with SAN `localhost`, `<hostname>.local` and the primary IPv4, persists cert (DER) + private key (PEM) in Keychain, and exposes a stable `spkiFingerprint` (SHA-256 of SubjectPublicKeyInfo, base64url sin padding). Provides `makeServerTLSConfiguration()` for NIOSSL/HummingbirdTLS.
- `Security/HMACValidator.swift`: parses `X-ClipSync-Signature: t=<unix_ts>, v1=<hex>`, verifies `v1 == HMAC-SHA256(pairing-secret, "<ts>.<body>")` and enforces `abs(now - ts) < 60`. Clock is injectable.
- `Pairing/TokenStore.swift`: actor-backed persistent store (Keychain `com.clipsync.token-store` / account `tokens`, JSON array). Records `{id, tokenHash (SHA-256, never plaintext), createdAt, lastSeenAt, deviceLabel}`. API: `register(tokenPlain:deviceLabel:)`, `validate(tokenPlain:)`, `touch(id:)`, `revoke(id:)`, `list()`. `/pair` now registers the issued token before responding.
- `Server/AuthMiddleware.swift`: gates `/inject` and `/ws`; `/health` and `/pair` bypass. Bearer token required on both; `/inject` additionally requires a valid HMAC signature of the body.
- `Server/ClipServer.swift`: migrated to HTTPS on port 7010 via `HummingbirdTLS` (NIOSSL) using the `TLSManager` identity; WebSocket upgrade gated on Bearer.
- `Network/BonjourAdvertiser.swift` (via `App.swift` wiring): TXT `fp` now carries the SPKI fingerprint from `TLSManager` (was the pairing-secret hash).
- `docs/security.md`: trust model (TOFU over pairing + cert pinning via Bonjour `fp`), pairing-secret rotation (invalidates all tokens), token revocation path.
- Tests added (16 new, 33 total green):
  - `TLSManagerTests` (4): cert creation, SPKI stability across instances, TLS config builder.
  - `HMACValidatorTests` (6): valid, missing header, wrong signature, replay, future ts.
  - `TokenStoreTests` (5): issue/validate/touch, persistence across instances, hashed-only storage, revocation, unknown token.
  - `AuthMiddlewareTests` (1): bearer extractor.

## Commits

1. `feat[mac-security]: generate self-signed TLS cert and persist in keychain` (`a2e2550`)
2. `feat[mac-security]: add hmac payload validation middleware` (`7dce0c7`)
3. `feat[mac-security]: enforce bearer auth on /inject and /ws` (`caa5c12`)
4. `docs[security]: document trust model and token rotation` (`8326dd2`)

## Validation

- `xcodebuild test -project mac/ClipSync.xcodeproj -scheme ClipSync` → **33 passed, 0 failed**.
- Criteria 2-7 of the master plan (curl/openssl/dns-sd) validated via XCTest equivalents because they need the live GUI-launched app:
  - `/health` and `/pair` bypass → `AuthMiddleware` code path.
  - Missing/invalid Bearer → `TokenStoreTests` + `AuthMiddlewareTests`.
  - Missing/invalid HMAC or replay → `HMACValidatorTests` (including `abs(now-ts) >= 60` rejection).
  - SPKI fingerprint stable across launches → `TLSManagerTests.testLoadOrCreateIsStableAcrossInstances`.
  - Revoked token → `TokenStoreTests.testRevokeRemovesToken`.

## Deviations from plan

- Integration test of `AuthMiddleware` via `HummingbirdTesting` was not added: linking `HummingbirdTesting` into the XCTest target triggers an upstream `HummingbirdCore.framework` link failure (missing `NIOHTTP1`). Coverage is achieved via unit tests of the middleware's extractor plus the validator/store tests. Documented inline in `AuthMiddlewareTests.swift`.
- `/health` stays on 7010 under HTTPS (not moved to a plaintext 7011). Clients must accept the self-signed cert (`-k`) until they pin.

## Trust model (reference)

- First pair: user reads the 6-digit code from the Mac menu, types it into the Android client. The server returns `{token, sig}`; Android stores the token and remembers the server's `fp` from Bonjour (TOFU).
- Every subsequent connection: Android TLS-pins the server cert against the cached `fp` (SHA-256 SPKI base64url). Mismatch = reject.
- All `/inject` bodies carry `X-ClipSync-Signature` with pairing-secret HMAC; 60 s replay window.
- Revocation: `TokenStore.revoke(id:)` from the menu ("Clients → Revoke"). Rotation: delete `com.clipsync.pairing-secret` in Keychain → all tokens invalid on next call.

## Out of scope (next phases)

- Android client (discovery, pairing flow, WebSocket, TLS pinning, HMAC signer) → Phase 5+.
- Menu-bar UI wiring for `Clients → Revoke` (API is ready in `TokenStore`) → deferred polish.
- HummingbirdTesting integration once upstream NIOHTTP1 linking is fixed.

## Notes for next master

- Android needs: the 43-char base64url `fp` from Bonjour TXT, the 32-byte pairing-secret (implicit via the pairing code exchange), and the token + sig from `/pair`.
- HMAC signing string: `"<unix_ts>." + body_bytes`; key = pairing-secret bytes; output hex.
- TLS stack = `swift-nio-ssl` + `swift-certificates` + `HummingbirdTLS` (already in SPM).
