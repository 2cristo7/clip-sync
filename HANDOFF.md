# ClipSync Pipeline Handoff

**Fecha**: 2026-04-15
**Última fase completada**: 5 — Android Base Client (merge `e1b2d16`)
**Siguiente fase**: 6 — Android: Notificaciones + Inyección en ClipboardManager
**Tests actuales**:
- macOS (`xcodebuild test -project mac/ClipSync.xcodeproj -scheme ClipSync`): 33 passed, 0 failed.
- Android (`cd android && ./gradlew :app:testDebugUnitTest :app:assembleDebug`): 11 unit tests passed, `BUILD SUCCESSFUL`.
**Rama actual**: `main` (limpia, sin trabajo en curso).

## Estado de merges en `main` (first-parent)

```
e1b2d16 Merge branch 'feature/android-client-core' into main   ← Phase 5
eaf5710 docs[pipeline]: add phase 4 summary
85d957d Merge branch 'feature/mac-security' into main          ← Phase 4
49e334e chore[pipeline]: document phased sub-agent delegation protocol
a4eba91 docs[pipeline]: add master plan and phase 1-3 summaries
f448eeb Merge branch 'feature/mac-discovery-pairing' into main ← Phase 3
8243f01 Merge branch 'feature/mac-clipboard-core' into main    ← Phase 2
fb0fd2f Merge branch 'feature/mac-server-core' into main       ← Phase 1
efb69b3 Merge branch 'chore/bootstrap' into main               ← Phase 0
102ba72 chore[init]: initialize repository and add git conventions
```

## Notas en caliente para el próximo master

### Decisión pendiente que afecta Fase 7 (CRÍTICA)
**Distribución del `pairing-secret` al cliente Android para firmar HMAC en `POST /inject`.**

Estado actual:
- `mac/ClipSync/Pairing/PairingManager.swift` genera `{token, sig}` donde `sig = HMAC-SHA256(token, pairing-secret)`. El secret NO sale del Mac.
- El QR (`clipsync://pair?host=…&port=…&code=…`) NO incluye el secret.
- Android tiene `crypto/HmacSigner.kt` listo y testeado, pero no tiene el secret para firmar.

Recomendación del sub-agente de Fase 5: **extender `/pair` para devolver un tercer campo `secret` (base64)** además de `token`+`sig`. La TLS preserva la confidencialidad. El sub-agente de Fase 7 debe decidir y, si toma esta opción, actualizar también el sub-agente de Fase 6 si necesita firmar algo (probablemente no — Fase 6 solo recibe).

TODOs vivos en: `android/app/src/main/java/com/clipsync/crypto/HmacSigner.kt`, `android/app/src/main/java/com/clipsync/net/PairingApi.kt`.

### Punto de extensión para Fase 6
- `android/app/src/main/java/com/clipsync/service/ClipForegroundService.kt` → método `onFrame(payload)` actualmente solo hace `Log.i("ClipSync", …)`. Ahí se enchufa el `IncomingClipNotifier` + `ApplyClipActivity` trampoline descritos en Fase 6 del `master_plan.md`.
- `model/ClipPayload.kt` ya tiene `fromJson/toJson` testeados — reutilizar.
- `Fingerprint.okHttpPin` y la conversión TOFU funcionan; no tocar la capa de red salvo necesidad.

### Deuda y warnings (no bloqueantes)
- Android `compileSdk = 35` con `targetSdk = 34`. AGP 8.5.2 advierte "tested up to 34"; build OK.
- macOS: `HummingbirdTesting` no está en el target de tests por un fallo de link de `HummingbirdCore.framework` (NIOHTTP1 missing). Cobertura del middleware vía unit tests del extractor + `TokenStoreTests` + `HMACValidatorTests`. Reintentar cuando upstream lo arregle.
- Menú "Clients → Revoke" en macOS no está cableado; `TokenStore.list/revoke` listos para conectarlo (polish, no crítico).
- Bonjour TXT `fp` ya es SPKI-SHA256 base64url sin padding; cliente Android ya lo lee correctamente.
- `Package.resolved` en `.gitignore` (recordar para futuras fases macOS).
- pbxproj: `objectVersion 56`, IDs hex 24 chars libres desde `A00000000000000000001800` (revisar en pbxproj antes de añadir archivos para no chocar; los rangos usados en Fase 4 fueron consecutivos).

### Sesión cerrada por protocolo de handoff
- 2 fases completadas en esta sesión (4 y 5).
- Fase 4 requirió 1 reintento del sub-agente (el primer Agent confundió su rol con el del master). Por eso el handoff dispara tras 2 fases en lugar de 3.
- Tool calls usadas en esta sesión: ~35.

## Cómo continuar

Abre un chat nuevo de Claude Code en esta misma carpeta (`/Users/2cristo7/Documents/personal-proyects/shared-clipboard`) y pega el prompt de arranque original. El nuevo master leerá este `HANDOFF.md`, verificará que `git log --oneline --first-parent main` coincide, y retomará en Fase 6.

**Tip al próximo master**: cuando lances el `Agent` para Fase 6 (y sucesivas), incluye este bloque al inicio del prompt para evitar el problema de Fase 4:

> ## Tu rol (léelo primero)
> Eres un sub-agente implementador. Implementas directamente. NO eres el master. NO delegues, NO lances otros Agent, NO mergees a main, NO hagas push. La regla "el master no implementa código" del claude.md se aplica al master, no a ti.
