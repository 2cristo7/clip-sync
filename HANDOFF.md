# ClipSync Pipeline Handoff

**Fecha**: 2026-04-15
**Última fase completada**: 7 — Android Share Target + Pairing-Secret Distribution (merge `9ba2ecb`)
**Siguiente fase**: 8 — Integración Tailscale + Pruebas Remotas
**Tests actuales**:
- macOS (`xcodebuild test -project mac/ClipSync.xcodeproj -scheme ClipSync`): 33 passed, 0 failed (primer run intermitente "test runner hung"; retry verde — flaky harness, no el código).
- Android (`cd android && ./gradlew :app:testDebugUnitTest :app:assembleDebug :app:lintDebug`): 27 unit tests passed, BUILD SUCCESSFUL, lint clean.

**Rama actual**: `main` (limpia).

## Estado de merges en `main` (first-parent)

```
9ba2ecb Merge branch 'feature/android-share-target' into main              ← Phase 7
6150f8f docs[pipeline]: add phase 6 summary
12968fc Merge branch 'feature/android-notifications-clipboard' into main   ← Phase 6
6729f96 chore[pipeline]: add phase 5 summary and handoff
e1b2d16 Merge branch 'feature/android-client-core' into main               ← Phase 5
eaf5710 docs[pipeline]: add phase 4 summary
85d957d Merge branch 'feature/mac-security' into main                      ← Phase 4
49e334e chore[pipeline]: document phased sub-agent delegation protocol
a4eba91 docs[pipeline]: add master plan and phase 1-3 summaries
f448eeb Merge branch 'feature/mac-discovery-pairing' into main             ← Phase 3
8243f01 Merge branch 'feature/mac-clipboard-core' into main                ← Phase 2
fb0fd2f Merge branch 'feature/mac-server-core' into main                   ← Phase 1
efb69b3 Merge branch 'chore/bootstrap' into main                           ← Phase 0
```

## Notas en caliente para el próximo master

### Decisión resuelta (cerrada en Fase 7)
La distribución del `pairing-secret` se hace dentro del JSON de `/pair` (TLS). No hay endpoint separado. `PairingResponse = {token, sig, secret}`. Android lo persiste en `Prefs.pairingSecret` (EncryptedSharedPreferences). `HmacSigner` lo usa sin TODOs pendientes. Formato HMAC usado: header `X-ClipSync-Signature: t=<ts>, v1=<hex>` sobre `<ts>.<body>`.

### Estado funcional end-to-end tras Fase 7
- Mac → Pixel: implementado (Fase 5 WebSocket + Fase 6 notificaciones + trampolín ApplyClipActivity).
- Pixel → Mac (Share Target): implementado (Fase 7 ShareReceiverActivity + ShareSender firmando HMAC).
- Cubierto por tests unitarios (MockWebServer en Android, 33 tests en Mac). **Validación en device real pendiente** — sugerirlo tras Fase 8.

### Puntos de extensión para Fase 8 (Tailscale)
- `Prefs.pairingSecret`, `Prefs.token`, `Prefs.fp` existen. Tailscale añade sólo host/port distintos (MagicDNS o IP Tailscale). `SettingsViewModel.persistAndStart` debe seguir recibiendo el secret en cualquier re-pair.
- Modo "Manual" en UI Compose (TOFU) ya permite introducir IP+puerto; probablemente suficiente para Tailscale sin cambios en UI.
- mDNS sobre Tailscale NO suele funcionar (no hay multicast). Documéntalo: "Auto" en LAN, "Manual" en Tailscale.
- TLS self-signed con cert pinning sobrevive a Tailscale sin cambios (pinning por SPKI fingerprint, no por hostname).
- `master_plan.md` Fase 8 (líneas 470-513) tiene el spec; entregable principal es `docs/tailscale-setup.md`.

### Deuda y warnings (no bloqueantes)
- macOS: `HummingbirdTesting` sigue fuera del target de tests (link NIOHTTP1). Cobertura vía unit tests.
- macOS: `xcodebuild test` a veces falla con "test runner hung before establishing connection" en el primer run — retry verde. Posible saturación del simulator/DerivedData; si ocurre, retry una vez antes de bloquear.
- Android: AGP 8.5.2 warning "compileSdk 35 tested up to 34" — no bloquea.
- Menú "Clients → Revoke" en macOS sin cablear (polish Fase 9).
- `Package.resolved` en `.gitignore` (recordar).
- pbxproj: `objectVersion 56`, IDs hex 24 chars — rangos libres continuando desde los consecutivos usados en Fase 4.

### Contexto consumido por handoff
- Sesión completó Fases 6 y 7 (2 fases).
- Fase 7 sub-agente: 81 tool_uses y ~94k tokens (output grande). Por protocolo (b) — `>50 KB output` aproximado — handoff tras 2 fases.
- Sin reintentos de sub-agente en esta sesión.

## Cómo continuar

Abre un chat nuevo de Claude Code en `/Users/2cristo7/Documents/personal-proyects/shared-clipboard` y pega el prompt de arranque original. El nuevo master leerá este `HANDOFF.md`, verificará `git log --oneline --first-parent main` y retomará en Fase 8.

**Tip al próximo master**: Fase 8 (Tailscale) es principalmente validación manual + documentación (`docs/tailscale-setup.md`). Respeta la regla "el master no implementa código" y delega aunque el sub-agente sea pequeño. Usa este bloque al inicio del prompt del sub-agente:

> ## Tu rol (léelo primero)
> Eres un sub-agente implementador. Implementas directamente. NO eres el master. NO delegues, NO lances otros Agent, NO mergees a main, NO hagas push.
