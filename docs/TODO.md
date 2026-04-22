# ClipSync — Tareas Pendientes

Estado: Pipeline completo (Fases 0-9 mergeadas, tag `v0.1.0`).
Fecha: 2026-04-18

---

## Validación manual (requiere dispositivos físicos)

- [ ] **Tailscale E2E testing** — Completar la tabla "Tested scenarios" en `docs/tailscale-setup.md`:
  - Mac en Wi-Fi, Pixel en datos móviles (5G), ambos en Tailscale
  - Pairing manual con IP 100.x.x.x
  - Copiar en Mac → notificación en Pixel (latencia < 2s)
  - FAB overlay → clipboard del Mac
  - Reconexión tras cambio de red (Wi-Fi → 5G) en < 30s
  - Desconexión/reconexión de Tailscale en Mac
- [ ] **Test en device real (LAN)** — Validar flujo completo sin Tailscale:
  - mDNS discovery automático
  - Pairing con código
  - Texto e imagen en ambas direcciones

## UI / Polish

- [ ] **Menú "Clients → Revoke"** en barra de estado macOS — `TokenStore.revoke()` existe pero no está cableado al menú de la app
- [ ] **Screenshots para README** — Capturar pantallas reales para documentación (pairing, notificación, share sheet)

## Build y distribución

- [ ] **Code signing macOS** — Configurar identidad de firma en `mac/scripts/build-release.sh` (actualmente genera build sin firmar)
- [ ] **Notarización macOS** — Opcional: integrar `notarytool` en el script de release
- [ ] **Firma APK Android** — Configurar keystore para `assembleRelease` (actualmente solo debug)
- [ ] **Push a GitHub** — Subir repositorio para activar GitHub Actions CI
- [ ] **Verificar CI** — Confirmar que `.github/workflows/ci.yml` pasa en GitHub Actions (mac + android jobs)

## Seguridad (checklist de `master_plan.md` Apéndice B)

- [x] TLS obligatorio en producción
- [x] HMAC con timestamp ±60s en cada payload
- [ ] Token revocable desde el menú bar (backend listo, UI pendiente)
- [ ] Pairing-secret rotable regenerando en el Mac (invalida todos los devices) — lógica no implementada
- [x] Tamaño máximo de payload (20 MB en ClipPayloadBuilder)
- [ ] Auditar que logs no filtran contenido del clipboard (solo longitud + tipo)
- [x] Keychain para secretos en Mac; EncryptedSharedPreferences en Android
- [x] Imágenes se borran del cache tras 24h (`ImageCache.cleanupOlderThan`)

## Deuda técnica (no bloqueante)

- [ ] **AGP warning** — `compileSdk 35 tested up to 34` (AGP 8.5.2). Resolver al actualizar AGP
- [ ] **xcodebuild test flaky** — Primer run a veces falla con "test runner hung before establishing connection". Retry siempre verde. Posible saturación de DerivedData
- [ ] **`HummingbirdTesting` fuera del target de tests** — Link error con NIOHTTP1; cobertura cubierta por unit tests actuales
- [ ] **mDNS sobre Tailscale** — No funciona (sin multicast). Documentado en `docs/tailscale-setup.md`. Si se quiere soporte futuro: servidor de rendezvous o Tailscale Funnel
