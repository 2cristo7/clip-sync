# Phase 7 Summary — Android Share Target + Pairing-Secret Distribution

**Branch**: `feature/android-share-target` (merged into `main` via `9ba2ecb`, `--no-ff`).

## Decisión clave
El pairing-secret se distribuye ahora dentro del JSON de `/pair` (TLS preserva confidencialidad). `PairingResponse` lleva `{token, sig, secret}`. No hay endpoint separado de key-exchange. Android persiste el secret en `EncryptedSharedPreferences` y `HmacSigner` lo usa para firmar `POST /inject`.

## What shipped
### macOS
- `PairingResponse` en `mac/ClipSync/Pairing/PairingManager.swift` añade campo `secret: String` (base64).
- `PairingManagerTests.swift` actualizado.

### Android
- `ShareReceiverActivity` — intent-filters para `ACTION_SEND` (text/plain, image/*) y `ACTION_SEND_MULTIPLE` (image/*). Parsea `EXTRA_TEXT` / `EXTRA_STREAM`, límite 20 MB, MIME real vía `ContentResolver.getType` + `MimeTypeMap`, Toast de feedback, trampolín a `MainActivity` si no hay pairing.
- `ShareSender` — construye `ClipPayload`, firma HMAC con el secret (`HmacSigner`: formato `t=<ts>, v1=<hex>` sobre `<ts>.<body>`), POST `https://<host>:<port>/inject` con `Authorization: Bearer <token>` y `X-ClipSync-Signature: <hex>`. Reutiliza el OkHttpClient con cert pinning de `ClipClient`.
- `PairingApi.kt` — parsea `secret` del JSON y lo guarda.
- `Prefs.kt` — nuevo campo `pairingSecret` (base64) en EncryptedSharedPreferences; `clearPairing()` lo borra.
- `HmacSigner.kt` — eliminado TODO, usa secret real de Prefs.
- `AndroidManifest.xml` — `ShareReceiverActivity` registrada con intent-filters, exported=true, theme translucent, noHistory=true.

## Commits
```
9ec8f58 feat[mac-pairing]: return pairing-secret in pair response
588f1a3 feat[android-pairing]: persist pairing-secret from /pair response
a7a087e feat[android-share]: register share target for text and images
0057300 feat[android-share]: post shared content to mac via /inject
bcb70e3 test[android-share]: integration test with mock server
```

## Validation
- `xcodebuild test -project mac/ClipSync.xcodeproj -scheme ClipSync` → **33 passed, 0 failed** (primer run con "test runner hung" intermitente, retry verde).
- `./gradlew :app:testDebugUnitTest` → **27 passed, 0 failed** (23 previos + 4 nuevos `ShareSenderTest` con MockWebServer validando body, Bearer, HMAC, 200/401).
- `./gradlew :app:assembleDebug` → BUILD SUCCESSFUL.
- `./gradlew :app:lintDebug` → clean.

## Deviations from plan
- `shortcuts.xml` (Direct Share Targets Android 11+) no se añadió — es opcional y de menor valor.
- Formato HMAC: firma sobre `<ts>.<body>` con header compuesto `t=<ts>, v1=<hex>` (consistente con `HMACValidator.swift`).

## Out of scope / Follow-ups (para Fase 8)
- Validación end-to-end en device real (Chrome share → Mac pbpaste / Galería share → Preview paste) es manual.
- `TofuPairingResponse` ahora lleva `secret` como 3er campo — cualquier flow de pairing nuevo (Tailscale manual, re-pair) debe propagarlo a `Prefs` vía `SettingsViewModel.persistAndStart`.
- Warning AGP 8.5.2 vs compileSdk=35 persiste (no bloqueante).
