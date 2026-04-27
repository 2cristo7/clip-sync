# Phase 5 Summary — Android Base Client

**Branch**: `feature/android-client-core` → merged into `main` with `--no-ff` (`e1b2d16`).

## What shipped

Standalone Android app under `android/` (Kotlin 2.0.21, AGP 8.5.2, Compose BOM 2024.10.01).

- **Discovery** — `discovery/NsdDiscovery.kt`: `NsdManager` listener for `_clipsync._tcp`. Resolves to `Discovered(host, port, fp)` (parses TXT `fp`).
- **Pairing** — `net/PairingApi.kt`: `GET https://<host>:<port>/pair?code=XXXXXX` via OkHttp, returns `{token, sig}`. Uses pinned client when `fp` is known (mDNS), TOFU client when manual IP.
- **WebSocket client** — `net/ClipClient.kt`: `OkHttpClient` with dynamic `CertificatePinner` (`sha256/<base64>` derived from `fp` base64url → standard base64 via `Fingerprint.okHttpPin`). TOFU mode captures `X509Certificate.publicKey.encoded` SPKI-SHA256 in `checkServerTrusted` and persists it. `connectWebSocket(host, port, token)` sends `Authorization: Bearer <token>` on the upgrade.
- **Foreground service** — `service/ClipForegroundService.kt`: `foregroundServiceType="dataSync"`, persistent notification "ClipSync connected (host)", exponential backoff capped at 30 s, reconnects on `NetworkCallback` events. Currently only logs incoming frames (`Log.i("ClipSync", "frame type=… mime=… bytes=…")`).
- **UI** — `ui/SettingsScreen.kt` + `SettingsViewModel.kt`: Compose Material3, mode toggle Auto mDNS / Manual IP, host+port inputs (default 7010), "Pair" dialog with 6-digit code input, status chip `Disconnected/Connecting/Connected/Error(reason)`.
- **Storage** — `storage/Prefs.kt`: `EncryptedSharedPreferences` for `token`, `fp`, `host`, `port`, `mode`.
- **Crypto helpers (already wired for Phase 7)** — `crypto/Fingerprint.kt` (base64url ↔ standard base64), `crypto/HmacSigner.kt` (`signatureHeader(secret, ts, body) → "t=<ts>, v1=<hex>"`).
- **Model** — `model/ClipPayload.kt` data class + JSON roundtrip.
- **Manifest** permissions: `INTERNET`, `ACCESS_NETWORK_STATE`, `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_DATA_SYNC`, `POST_NOTIFICATIONS`.
- **Gradle Wrapper** committed (`gradlew`, `gradlew.bat`, `gradle-wrapper.jar`, `gradle-wrapper.properties`). `.gitignore` extended with `android/.gradle/`, `android/app/build/`, `android/build/`, `android/local.properties`.

## Commits

1. `chore[android]: bootstrap gradle kotlin compose project` (`ae99ee3`)
2. `feat[android-discovery]: add nsdmanager mdns discovery` (`f8601f7`)
3. `feat[android-client]: okhttp websocket client with cert pinning` (`dcf45d5`)
4. `feat[android-service]: foreground service to keep websocket alive` (`3579993`)
5. `feat[android-ui]: settings screen with auto/manual modes and pairing` (`ccb1fb5`)

## Validation

- `cd android && ./gradlew :app:assembleDebug` → `BUILD SUCCESSFUL`.
- `cd android && ./gradlew :app:lintDebug` → no critical errors (warnings OK).
- `cd android && ./gradlew :app:testDebugUnitTest` → 11 passed, 0 failed:
  - `ClipPayloadTest` × 3 (text JSON, image JSON, malformed input).
  - `NsdDiscoveryTest` × 2 (TXT parsing).
  - `FingerprintTest` × 3 (base64url → OkHttp pin conversion, padding, edge cases).
  - `HmacSignerTest` × 3 (vector against known HMAC, header format, ts type).
- Manual validation (NOT executed — needs Pixel + running Mac server): `adb install`, pair via 6-digit code, copy on Mac, `adb logcat -s ClipSync:I` shows incoming frame; swipe MainActivity from recents → service survives; toggle Wi-Fi → reconnects within ~1-2 s.

## Deviations from plan

- `compileSdk = 35` (only SDK platforms 35/36 installed on host); `targetSdk = 34` as specified. AGP 8.5.2 prints a "tested up to 34" warning but build/tests/lint are all green.
- Hilt skipped (optional per spec); manual injection.
- `org.json:json:20240303` added as `testImplementation` because the Android stub `org.json` throws in JVM tests.

## Out of scope (next phases)

- `ClipboardManager` writes / `ApplyClipActivity` trampoline → Phase 6.
- Notifications for incoming clips → Phase 6.
- `POST /inject` from Android (Share Target) → Phase 7. `HmacSigner` is already implemented and tested for that.

## Notes for next master / Phase 6+ author

- Single integration point for clipboard writes is `ClipForegroundService.onFrame(payload)` (currently only `Log.i`). Keep the foreground service as the receiver; route into a notifier that schedules `ApplyClipActivity`.
- TOFU vs pinned: Auto mode (with mDNS `fp`) starts pinned. Manual IP falls back to TOFU on first connect, persists SPKI hash in `Prefs.fp`, then pins on subsequent runs.
- **Pairing-secret distribution for Phase 7** (HMAC signing of `/inject`): unresolved. The current QR (`clipsync://pair?host=…&port=…&code=…`) does NOT carry the pairing-secret and `/pair` returns only `{token, sig}`. **Recommended**: extend `/pair` to return a third field `secret` (base64) — confidentiality preserved by TLS, sig becomes verifiable as proof-of-server-knowledge. Alternative QR-only path leaks the secret to anyone who sees the QR. TODOs left in `HmacSigner.kt` and `PairingApi.kt`.
