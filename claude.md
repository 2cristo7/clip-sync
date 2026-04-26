# Git Conventions

## Commits

All commit messages must be in English and follow the Conventional Commits format:

```
type[scope]: message
```

Types allowed: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`.

Example: `feat[mac-network]: add mDNS broadcasting`

## Branches

All branches must follow kebab-case and use the following prefixes:

`feature/`, `fix/`, `hotfix/`, `release/`, `chore/`

Example: `feature/auth-token`

---

# Pipeline de Fases con Sub-agentes (a partir de Fase 4)

Este proyecto se ejecuta por fases definidas en `master_plan.md`. A partir de
la Fase 4, el agente principal actúa como **master** y delega cada fase en
**sub-agentes** con contexto limpio mediante la tool `Agent`.

## Fuentes de verdad

- `master_plan.md` — definición oficial de todas las fases (objetivos,
  archivos, commits esperados, criterios de éxito).
- `docs/phase-N-summary.md` — estado acumulado tras completar cada fase.
  Al terminar una fase, el master crea o actualiza este archivo siguiendo
  el patrón de `docs/phase-1-summary.md`, `docs/phase-2-summary.md` y
  `docs/phase-3-summary.md`.
- `claude.md` (este archivo) — convenciones de Git + reglas del pipeline.

## Cuándo activar el pipeline

Activa el comportamiento de master cuando el usuario pida:
- "ejecuta la Fase N", "arranca la Fase N", "continúa el pipeline", o
- cualquier instrucción que implique avanzar una o más fases del
  `master_plan.md`.

Para cambios puntuales fuera de fase (un bugfix, un refactor, una pregunta)
NO actives el pipeline: trabaja directamente.

## Comportamiento del agente master

### 1. Lee las fases

Lee `master_plan.md` y para cada fase solicitada extrae:
- Número y nombre de la fase.
- Objetivo y archivos a crear/modificar.
- Rama y commits esperados.
- Criterios de éxito (sección "Criterios de Éxito").
- Dependencias con fases anteriores (implícitas por el orden).

### 2. Ejecuta cada fase de forma secuencial

Para cada fase, sigue este ciclo:

**A) Prepara el contexto acumulado**

Construye un resumen auto-contenido que el sub-agente pueda leer sin ver
esta conversación:
- Qué implementó cada fase anterior (resumen desde
  `docs/phase-N-summary.md`).
- Tests que pasan actualmente y cobertura.
- Archivos clave y su propósito.
- Decisiones técnicas tomadas (ej. Hummingbird 2.x requiere macOS 14;
  `NetService` programado en `.main/.common`; `PairingClock` inyectable).
- Deuda técnica o notas (ej. `remoteAddress` pendiente, `fp` pasa a SPKI
  real en Fase 4).

**B) Lanza sub-agente(s) con contexto limpio**

Usa la tool `Agent` con `subagent_type: "general-purpose"` para
implementación. Cada llamada arranca con contexto cero — el prompt debe
ser completamente self-contained.

Plantilla obligatoria del prompt:

```
## Contexto del proyecto
Ruta: /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee primero: claude.md, master_plan.md (Fase N) y docs/phase-*-summary.md.

[resumen del estado actual del código y del stack ya disponible]

## Lo que se ha hecho hasta ahora
[resumen de fases anteriores: qué se implementó, qué tests pasan,
decisiones técnicas relevantes]

## Tu tarea — Fase N: [nombre]
[descripción exacta adaptada del master_plan.md, incluyendo rama
feature/…, archivos a tocar y commits atómicos esperados]

## Criterios de éxito
[condiciones ejecutables en terminal, copiadas del master_plan.md]

## Instrucciones de verificación
Al terminar tu tarea debes:
1. Ejecutar `xcodebuild test -project mac/ClipSync.xcodeproj -scheme ClipSync`
   (y cualquier test adicional relevante) y mostrar el output completo.
2. Confirmar los criterios de éxito uno a uno, con el comando que
   los valida.
3. Listar los archivos que creaste o modificaste.
4. Hacer los commits atómicos definidos en el master_plan.md siguiendo
   las convenciones de claude.md.
5. Reportar con EXACTAMENTE este formato al final de tu respuesta:

---FASE_COMPLETADA: [sí/no]
Tests: [N passed, M failed]
Archivos modificados: [lista]
Commits: [lista de hashes cortos + subject]
Resumen: [2-3 líneas de qué se hizo]
Notas para la siguiente fase: [lo que el siguiente agente debe saber]
---

NO hagas merge a main. El master se encarga del merge tras validar.
```

**C) Si una fase necesita múltiples sub-agentes**

Divide sólo si las subtareas son independientes o estrictamente
secuenciales (ej. "TLS cert + trust model doc" tiene dos subtareas
independientes). Cada subtarea = un `Agent` separado con contexto limpio.
El bloque `---FASE_COMPLETADA---` del anterior se incluye textualmente
en el prompt del siguiente cuando hay dependencia. Todos deben reportar
`FASE_COMPLETADA: sí` para avanzar.

Cuando las subtareas son independientes, lanza los `Agent` en paralelo
(múltiples tool calls en un solo mensaje).

**D) Valida el resultado**

Tras cada sub-agente, el master:
1. Parsea el bloque `---FASE_COMPLETADA---` del output.
2. Verifica `FASE_COMPLETADA: sí`, tests en verde y criterios cumplidos.
3. Hace `git log --oneline main..HEAD` para confirmar que los commits
   existen con los subjects esperados.
4. Si todo pasa → hace el merge a `main` con `--no-ff` y borra la rama
   local. Al ser una acción con blast radius, pide confirmación al
   usuario la primera vez en la sesión; si el usuario autoriza "auto-merge
   durante el pipeline", procede sin volver a preguntar.

Si algo falla:
- Analiza el error.
- Crea un **nuevo** `Agent` (contexto limpio) incluyendo en el prompt el
  output completo del sub-agente anterior + el diagnóstico del master.
- **Máximo 2 reintentos por fase** antes de pausar y preguntar al
  usuario.

**E) Cierra la fase**

Antes de avanzar a la siguiente fase, el master:
1. Crea `docs/phase-N-summary.md` siguiendo el patrón de los anteriores
   (Branch/What shipped/Commits/Validation/Deviations/Out of scope).
2. Añade ese resumen al contexto acumulado para el siguiente
   sub-agente.
3. Reporta al usuario con el formato de la sección 4.

### 3. Reglas estrictas

- **Nunca reutilices un sub-agente entre fases.** Cada fase arranca con
  un `Agent` fresco.
- **No avances si los tests fallan.** Bloquea y pregunta.
- **El contexto se pasa explícitamente en el prompt.** Los sub-agentes
  no ven esta conversación ni los prompts de otros sub-agentes.
- **El master NO implementa código directamente durante el pipeline.**
  Sólo lee, resume, lanza `Agent`, valida y hace el merge. La única
  excepción es crear `docs/phase-N-summary.md` tras validar la fase.
- **Los sub-agentes NO hacen merge a main.** El master se encarga tras
  validar.
- **Reporta progreso al usuario** después de cada fase (ver formato
  abajo).
- **Respeta siempre las convenciones de Git** de la parte superior de
  este archivo (Conventional Commits en inglés, ramas kebab-case con
  prefijo).

### 4. Formato de reporte al usuario entre fases

Tras completar una fase:

```
✅ Fase N completada: [nombre]
   Tests: X passed, Y failed
   Commits: [N commits atómicos]
   Cambios: [resumen en 1 línea]
   Resumen: docs/phase-N-summary.md
   Siguiente: Fase N+1 — [nombre]
```

Si hay un fallo:

```
❌ Fase N bloqueada: [nombre]
   Error: [descripción corta]
   Intentos: [1/2 | 2/2]
   Opciones: [a] reintentar con más contexto  [b] saltar  [c] revisar manualmente
```

### 5. Notas específicas del proyecto

#### Estado del pipeline
- **Todas las fases 0–9 están mergeadas en `main`. Tag `v0.1.0` creado.**
  El pipeline original está completo. Cualquier trabajo nuevo empieza
  desde `main` directamente o abre un nuevo ciclo de fases.

#### Stack y targets
- Target macOS: **14.0** (Hummingbird 2.x requiere macOS 14).
- Target Android: **API 33** (Android 13+).
- `Package.resolved` está en `.gitignore` — no lo commitees.

#### Estructura de archivos fuente (estado tras v0.1.0 + parches del 22 abr 2026)

**Mac — 17 archivos Swift** (`mac/ClipSync/`):
`App`, `Clipboard/{ClipPayload,PasteboardInjector,PasteboardWatcher}`,
`Network/{BonjourAdvertiser,ReachabilityMonitor}`,
`Pairing/{PairingManager,TokenStore}`,
`Security/{HMACValidator,TLSManager}`,
`Server/{AuthMiddleware,ClipServer,ServerConfig,WebSocketHub}`,
`Storage/Keychain`, `UI/{MenuBarController,PairingWindow}`

**Android — 22 archivos Kotlin** (`android/app/src/main/java/com/clipsync/`):
`MainActivity`, `clipboard/ClipboardWriter`, `crypto/{Fingerprint,HmacSigner}`,
`discovery/NsdDiscovery`, `images/ImageCache`,
`model/{ClipPayload,ClipPayloadBuilder}`,
`net/{ClipClient,NetworkChangeObserver,PairingApi}`,
`notifications/{ApplyClipActivity,IncomingClipNotifier}`,
`overlay/{ClipOverlayManager,ClipSender,SendClipActivity}`,
`service/ClipForegroundService`, `storage/Prefs`,
`ui/{SettingsScreen,SettingsViewModel,theme/ClipSyncTheme,theme/NeuComponents}`

#### Características implementadas
- Sync en tiempo real por WebSocket (LAN + Tailscale).
- TOFU pairing con HMAC (timestamp ±60s); pairing-secret en Keychain
  (`com.clipsync.pairing-secret`). Un nuevo secreto invalida todos los tokens.
- **FAB overlay persistente** en Android (`ClipOverlayManager`): burbuja
  flotante sobre cualquier app; toca para enviar clipboard al Mac vía
  `POST /inject`. Permiso `SYSTEM_ALERT_WINDOW` requerido.
- **Pausa/reanudación de sync** desde la burbuja FAB o el menú de settings.
- UI **neumórfica** en Android (`ClipSyncTheme` + `NeuComponents`).
- `NetworkChangeObserver` (Android) y `ReachabilityMonitor` (Mac): reconexión
  automática al cambiar de red.
- Share target **eliminado** (rama `chore/remove-share-target`, 22 abr 2026).
- CI en `.github/workflows/ci.yml` (mac + android jobs, push a main y PRs).

#### pbxproj
- Patrón: `objectVersion 56`, IDs hex de 24 chars.
- Último rango usado: `A000000000000000000024xx`.
- Próximo rango libre: `A000000000000000000025xx`.
- Proyecto migrado a **Xcode 26** (`LastUpgradeCheck = 2620`, scheme version `1.3`).
- `DEVELOPMENT_TEAM = 8FNGRJHHV4` configurado en ambas configs (Debug/Release).
- `DEAD_CODE_STRIPPING = YES` y `STRING_CATALOG_GENERATE_SYMBOLS = YES` activos.

#### Deuda técnica conocida
- `TokenStore.revoke()` existe pero no está cableado al menú bar (UI pendiente).
- No hay rotación de pairing-secret desde la UI.
- Code signing macOS: team ID `8FNGRJHHV4` configurado pero sin certificado/provisioning profile; builds sin firmar funcionan (`CODE_SIGN_IDENTITY = "-"`). Firma APK Android sin configurar.
- mDNS no funciona sobre Tailscale (sin multicast); documentado en
  `docs/tailscale-setup.md`.
- `NSBonjourServices` en `Info.plist` incluye `_clipsync._tcp`.

#### Android — reglas de compilación

- **No compilar para instalar en dispositivo** (`assembleDebug` + `adb install` están prohibidos).
- Para verificar errores de compilación usar exclusivamente:
  ```
  ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:"
  ```
  Es el formato más ligero: solo compila Kotlin, sin dexing ni packaging, y filtra la salida a errores/warnings únicamente.
- `assembleDebug` solo está permitido para verificar que el build completo no rompe (sin instalar).

#### Notas de UI
- `PairingWindow` tiene tamaño fijo **380×460** pt (no redimensionable).
- El QR se genera una sola vez en `onAppear` y se cachea en `@State` —
  no recomputar en cada render.
