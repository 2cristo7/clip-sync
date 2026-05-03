# ClipSync Connection Robustness — Launch Prompt

> Copy everything below the line into a fresh Claude Code chat.
> Make sure you're in `/Users/2cristo7/Documents/personal-proyects/clip-sync` before pasting.

---

Eres el **Orquestador** de un pipeline multi-agente que va a arreglar todos los bugs de conexión/desconexión de ClipSync. Tu trabajo es lanzar workers Sonnet para cada tarea, validar su output, y avanzar fase por fase.

## Setup inicial (hazlo ANTES de lanzar nada)

1. Lee `docs/development/connection-robustness-plan.md` — es el plan completo con 13 bugs, 3 race conditions, y 5 fases de fix.

2. Lee `CLAUDE.md` — convenciones de commits, estructura del proyecto, reglas de build.

3. Crea el archivo de estado:

Escribe `docs/development/CONNECTION_STATE.md`:
```markdown
# Connection Robustness Overhaul — State
## Status: NOT_STARTED
## Current Phase: 1
## Completed Phases: []
## Branch: fix/connection-robustness
## Last Commit: (none)
## Notes: Plan in docs/development/connection-robustness-plan.md
```

4. Crea la rama:
```bash
git checkout -b fix/connection-robustness
```

## Tu rol como Orquestador

Eres extremadamente eficiente en tokens. **NUNCA** lees código fuente completo ni escribes código. Solo:

1. Lees `docs/development/CONNECTION_STATE.md` para saber dónde estás
2. Lanzas workers Sonnet (Agent tool, model: "sonnet") para cada tarea
3. Después de cada worker: verificas con `grep`, `git diff`, y build commands
4. Si un worker produce código que no compila, lanzas uno nuevo con el error exacto
5. Actualizas el state file después de cada tarea completada
6. Si te quedas sin contexto, escribes CONTEXT_LIMIT en el state file con notas de handoff

## Fases y tareas

### Phase 1: WebSocket Keepalive (ambas plataformas — mayor impacto)

**Bugs que resuelve:** B3 (no WS ping Mac), B4 (lastSeen no usado), B7 (no keepalive Android)

**Worker 1A — Mac WebSocket Ping/Pong**
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Añadir ping/pong periódico al WebSocketHub del servidor Mac.

Lee estos archivos primero:
- `mac/ClipSync/Server/WebSocketHub.swift` — hub que mantiene clientes WebSocket
- `mac/ClipSync/Server/ClipServer.swift` — donde se hace el upgrade a WS

CONTEXTO: El server nunca envía pings. Si un cliente se desconecta sin cerrar el WS, el server no lo detecta hasta el próximo broadcast. El campo `lastSeen` del Client struct existe pero nunca se usa para timeouts.

CAMBIOS EN `WebSocketHub.swift`:

1. Añade una propiedad para el task de ping:
```swift
private var pingTask: Task<Void, Never>?
```

2. Añade método `startPingLoop()` que:
   - Cada 30 segundos, itera sobre todos los clientes registrados
   - Para cada cliente, intenta enviar un ping frame via `client.outbound.write(.ping(...))`
   - Si el write falla, llama `unregister(client)` (igual que en broadcast)
   - Actualiza `client.lastSeen` en cada pong/write exitoso
   - Loguea a nivel `.debug` cada ciclo de ping

3. Añade método `startTimeoutEnforcement()` (puede ser parte del mismo loop):
   - Cada 30 segundos, revisa `client.lastSeen`
   - Si `Date().timeIntervalSince(client.lastSeen) > 45` (45s sin respuesta): unregister
   - Loguea el timeout a nivel `.info`

4. Llama `startPingLoop()` desde el init o desde un método `start()` que ClipServer pueda llamar.

5. En el `deinit` o un método `stop()`: cancela `pingTask`.

IMPORTANTE: Hummingbird 2.x usa `WebSocketOutboundWriter` con método `write(_ frame:)`. Verifica la API exacta leyendo el código existente en broadcast(). Si `write(.ping(...))` no existe, usa `write(.custom(...))` o simplemente intenta escribir un frame de texto vacío como heartbeat — lo importante es detectar writes fallidos.

COMMIT: `fix[mac-ws]: add periodic ping and timeout enforcement to WebSocketHub`

Verifica compilación:
```bash
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Debug -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet 2>&1 | tail -20
```
---
```

**Worker 1B — Android WebSocket Keepalive** (paralelo con 1A)
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Configurar WebSocket keepalive en el cliente Android.

Lee `android/app/src/main/java/com/clipsync/net/ClipClient.kt`.

CONTEXTO: OkHttp soporta `pingInterval()` nativo. Actualmente `baseBuilder()` no configura ningún timeout ni ping. El server Mac va a empezar a enviar pings (otro worker lo implementa), pero el cliente también debe enviar los suyos.

CAMBIOS EN `ClipClient.kt`:

1. En `baseBuilder()`, añade:
```kotlin
private fun baseBuilder(): OkHttpClient.Builder = OkHttpClient.Builder()
    .pingInterval(30, java.util.concurrent.TimeUnit.SECONDS)
    .connectTimeout(10, java.util.concurrent.TimeUnit.SECONDS)
    .readTimeout(60, java.util.concurrent.TimeUnit.SECONDS)
    .writeTimeout(10, java.util.concurrent.TimeUnit.SECONDS)
```

Nota: `readTimeout` sube a 60s porque con pings cada 30s, el timeout por inactividad lo maneja el ping, no el read timeout.

2. Verifica que `pinnedClient()` y `tofuClient()` usen `baseBuilder()` (ya deberían).

COMMIT: `fix[android-ws]: configure OkHttp ping interval and explicit timeouts`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
---
```

### Phase 2: Android Health Check + State Sync

**Bugs que resuelve:** B1 (no health check), B8 (ViewModel/Service desync), B9 (bootstrap pings once)

**Worker 2A — Service StateFlow**
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Añadir StateFlow de conexión al ClipForegroundService y health check periódico.

Lee estos archivos:
- `android/app/src/main/java/com/clipsync/service/ClipForegroundService.kt`
- `android/app/src/main/java/com/clipsync/net/ClipClient.kt` (WsStatus)
- `android/app/src/main/java/com/clipsync/ui/SettingsViewModel.kt` (ConnectionStatus sealed class)

CONTEXTO: 
- El servicio mantiene `connectedHost` como String? pero no expone estado de conexión
- SettingsViewModel mantiene su propia copia de ConnectionStatus, independiente del servicio
- Bootstrap pinga una vez al server. Si el Mac muere después, Android muestra Connected forever.

CAMBIOS:

1. En `ClipForegroundService`, añade StateFlow expuesto via companion:
```kotlin
companion object {
    // ... existing constants ...
    
    private val _serviceState = MutableStateFlow<ServiceState>(ServiceState.Disconnected)
    val serviceState: StateFlow<ServiceState> = _serviceState.asStateFlow()
}

sealed class ServiceState {
    data object Disconnected : ServiceState()
    data object Connecting : ServiceState()
    data class Connected(val host: String) : ServiceState()
    data class Paused(val host: String) : ServiceState()
}
```

2. Actualiza `connect()` para emitir estados:
   - Antes de conectar: `_serviceState.value = ServiceState.Connecting`
   - En `WsStatus.Open`: `_serviceState.value = ServiceState.Connected(host)`
   - En `WsStatus.Closed/Error`: `_serviceState.value = ServiceState.Disconnected`

3. Añade health check periódico:
```kotlin
private var healthCheckRunnable: Runnable? = null
private var consecutiveFailures = 0

private fun startHealthCheck() {
    healthCheckRunnable = object : Runnable {
        override fun run() {
            if (connectedHost == null) return
            viewModelScope?.launch(Dispatchers.IO) {
                // No hay viewModelScope — usa un thread o coroutine scope propio
            }
            // Alternativa: usa handler + thread
            Thread {
                try {
                    val host = prefs.host ?: return@Thread
                    val port = prefs.port
                    val fp = prefs.fp ?: return@Thread
                    val ok = PairingApi().ping(host, port, fp)
                    handler.post {
                        if (ok) {
                            consecutiveFailures = 0
                        } else {
                            consecutiveFailures++
                            if (consecutiveFailures >= 3) {
                                L.warn(M, "health check: 3 consecutive failures, disconnecting")
                                _serviceState.value = ServiceState.Disconnected
                                ws?.cancel()
                                ws = null
                                connectedHost = null
                                scheduleReconnect()
                                consecutiveFailures = 0
                            }
                        }
                    }
                } catch (t: Throwable) {
                    handler.post {
                        consecutiveFailures++
                        if (consecutiveFailures >= 3) {
                            L.warn(M, "health check failed 3x: ${t.message}")
                            _serviceState.value = ServiceState.Disconnected
                            ws?.cancel()
                            ws = null
                            connectedHost = null
                            scheduleReconnect()
                            consecutiveFailures = 0
                        }
                    }
                }
            }.start()
            handler.postDelayed(this, 15_000) // cada 15s
        }
    }
    handler.postDelayed(healthCheckRunnable!!, 15_000) // primer check a los 15s
}

private fun stopHealthCheck() {
    healthCheckRunnable?.let { handler.removeCallbacks(it) }
    healthCheckRunnable = null
    consecutiveFailures = 0
}
```

4. Llama `startHealthCheck()` cuando WsStatus.Open, `stopHealthCheck()` cuando Closed/Error.

5. En pause/resume: `_serviceState.value = ServiceState.Paused(host)` / `Connected(host)`

NOTA: `PairingApi().ping()` ya existe y hace GET /health con timeout 3s. Reutilízalo.

COMMIT: `feat[android-health]: add ServiceState StateFlow and periodic health check`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
---
```

**Worker 2B — ViewModel observa Service** (depende de 2A)
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Hacer que SettingsViewModel observe el StateFlow del servicio en vez de mantener estado propio.

Lee:
- `android/app/src/main/java/com/clipsync/ui/SettingsViewModel.kt`
- `android/app/src/main/java/com/clipsync/service/ClipForegroundService.kt` (busca `ServiceState` y `serviceState`)

CONTEXTO: ClipForegroundService ahora expone `serviceState: StateFlow<ServiceState>` via companion. SettingsViewModel debe observarlo como fuente de verdad para el estado de conexión.

CAMBIOS EN `SettingsViewModel.kt`:

1. En `bootstrap()`, después del setup inicial, añade colección del service state:
```kotlin
viewModelScope.launch {
    ClipForegroundService.serviceState.collect { svcState ->
        val newStatus = when (svcState) {
            is ServiceState.Disconnected -> ConnectionStatus.Disconnected
            is ServiceState.Connecting -> ConnectionStatus.Connecting
            is ServiceState.Connected -> ConnectionStatus.Connected(svcState.host)
            is ServiceState.Paused -> ConnectionStatus.Paused(svcState.host)
        }
        _state.value = _state.value.copy(status = newStatus)
    }
}
```

2. Elimina el ping manual en `bootstrap()` — ya no es necesario porque el service hace health checks.
   - Quita el bloque que hace `PairingApi().ping()` y actualiza status
   - Mantén el resto de bootstrap (cargar prefs, iniciar discovery, etc.)

3. En `pair()`: después de `persistAndStart()`, no necesitas setear status manualmente — el service lo hará via StateFlow.
   - Puedes quitar `state = state.copy(status = ConnectionStatus.Connected(...))` del final de pair()
   - Mantén `ConnectionStatus.Connecting` al inicio del pair flow para feedback inmediato

4. Importa `ServiceState` del paquete `com.clipsync.service`.

COMMIT: `refactor[android-state]: ViewModel observes service StateFlow as single source of truth`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
---
```

### Phase 3: Discovery Robustness (Android)

**Bugs que resuelve:** B2 (no restart on disconnect), B6 (shared listener), B10 (only WiFi gain)

**Worker 3A — Fix NsdDiscovery ResolveListener**
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Arreglar el bug del ResolveListener compartido en NsdDiscovery.

Lee `android/app/src/main/java/com/clipsync/discovery/NsdDiscovery.kt`.

CONTEXTO: El ResolveListener se crea UNA VEZ fuera del loop (línea ~39) y se reutiliza para cada `resolveService()`. Android NsdManager falla silenciosamente con error 3 (FAILURE_ALREADY_ACTIVE) si un listener ya tiene un resolve pendiente. Resultado: si hay 2+ servicios en la red, solo el primero se resuelve.

CAMBIOS EN `NsdDiscovery.kt`:

1. En `onServiceFound()`, crea un NUEVO `ResolveListener` para cada resolve:
```kotlin
override fun onServiceFound(info: NsdServiceInfo) {
    L.event(M, "Found ${info.serviceName} / ${info.serviceType}")
    val perServiceListener = object : NsdManager.ResolveListener {
        override fun onServiceResolved(resolved: NsdServiceInfo) {
            val addr = resolved.host ?: return
            val port = resolved.port
            val parsed = parseTxt(resolved.attributes)
            val host = resolveHost(addr)
            val d = Discovered(
                host = host,
                port = port,
                fp = parsed["fp"],
                name = resolved.serviceName ?: "",
                version = parsed["version"]
            )
            L.event(M, "Resolved $d")
            trySend(DiscoveryEvent.Found(d))
        }
        override fun onResolveFailed(resolved: NsdServiceInfo, errorCode: Int) {
            L.warn(M, "Resolve failed for ${resolved.serviceName}: $errorCode")
            trySend(DiscoveryEvent.Error("Failed to resolve '${resolved.serviceName}' (error $errorCode)"))
        }
    }
    @Suppress("DEPRECATION")
    nsd.resolveService(info, perServiceListener)
}
```

2. Elimina el `resolveListener` que se creaba al inicio del `callbackFlow` — ya no se usa.

3. Mueve las funciones helper (`parseTxt`, `resolveHost`) si eran métodos del listener a funciones del scope o companion.

COMMIT: `fix[android-discovery]: create per-service ResolveListener to prevent FAILURE_ALREADY_ACTIVE`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
---
```

**Worker 3B — Discovery Auto-Restart** (paralelo con 3A si son archivos distintos, pero tocan SettingsViewModel ambos — mejor secuencial después de 3A)
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Hacer que discovery se reinicie automáticamente al desconectarse y en cualquier cambio de red.

Lee:
- `android/app/src/main/java/com/clipsync/ui/SettingsViewModel.kt` (busca `startDiscovery`, `startNetworkWatch`)
- `android/app/src/main/java/com/clipsync/service/ClipForegroundService.kt` (busca `ServiceState`)

CONTEXTO:
- Discovery solo se reinicia cuando WiFi cambia de off→on
- Si el WebSocket se desconecta, discovery no se reinicia — el Mac desaparece de la lista
- Necesitamos: reiniciar discovery en desconexión + en cualquier cambio de red

CAMBIOS EN `SettingsViewModel.kt`:

1. En la colección de `ClipForegroundService.serviceState` (añadida en worker anterior):
```kotlin
ClipForegroundService.serviceState.collect { svcState ->
    val newStatus = when (svcState) { /* ... existing mapping ... */ }
    _state.value = _state.value.copy(status = newStatus)
    
    // Restart discovery when disconnected (so Mac reappears in list)
    if (svcState is ServiceState.Disconnected && _state.value.hasPairing) {
        delay(2000) // brief delay to let network stabilize
        startDiscovery(/* need context */)
    }
}
```
Nota: si `startDiscovery` necesita Context, guárdalo como propiedad en bootstrap.

2. En `startNetworkWatch()`, cambia la condición de restart de discovery:
   - ANTES: `if (onWifi && !prev.isOnWifi) startDiscovery(appContext)`
   - DESPUÉS: `if (onWifi != prev.isOnWifi || onMobile != prev.isOnMobileData || vpnActive != prev.isTailscaleVpnActive) startDiscovery(appContext)`
   Es decir: reiniciar discovery en CUALQUIER cambio de red, no solo WiFi gain.

3. Añade auto-restart si el flow de discovery termina inesperadamente:
   En `startDiscovery()`, después del `catch`:
```kotlin
// Auto-restart discovery after unexpected completion (with backoff)
if (_state.value.status is ConnectionStatus.Disconnected) {
    delay(5000)
    startDiscovery(context)
}
```

COMMIT: `fix[android-discovery]: auto-restart on disconnect and any network change`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
---
```

### Phase 4: Mac Network + Server Lifecycle

**Bugs que resuelve:** B5 (network change no reinicia server), B11 (fire-and-forget), B12 (mDNS silent fail), R3 (advertiser race)

**Worker 4A — ReachabilityMonitor Callback + Advertiser Await**
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Mejorar ReachabilityMonitor para notificar a AppDelegate y arreglar race condition en advertiser restart.

Lee:
- `mac/ClipSync/Network/ReachabilityMonitor.swift`
- `mac/ClipSync/Network/BonjourAdvertiser.swift`
- `mac/ClipSync/App.swift` (busca `startAdvertising`, `reachability`)

CAMBIOS:

1. En `ReachabilityMonitor.swift`:
   - Añade callback `onNetworkChange: (() -> Void)?` como propiedad
   - En `handlePathUpdate()`, después de reiniciar advertiser, llama `onNetworkChange?()`
   - Esto permite que AppDelegate reaccione a cambios de red

2. En `BonjourAdvertiser.swift`:
   - Añade propiedad `@Published private(set) var isPublished = false`
   - En `netServiceDidPublish`: `isPublished = true`
   - En `netService:didNotPublish:`: `isPublished = false`, loguea a `.error`
   - Cambia `stop()` para ser async-safe: set `isPublished = false`
   - Añade `onPublishFailed: ((Error) -> Void)?` callback

3. En `ReachabilityMonitor.handlePathUpdate()`:
   - Cambia el restart del advertiser para hacer stop y ESPERAR un tick antes de start:
   ```swift
   advertiser.stop()
   // Brief delay to let the network stack settle
   try? await Task.sleep(for: .milliseconds(500))
   advertiser.start()
   ```
   (Nota: handlePathUpdate ya corre en un Task context porque viene del NWPathMonitor handler. Si no es async, wrápalo en Task {})

4. En `App.swift`:
   - En `startAdvertising()`, configura el callback:
   ```swift
   reachability.onNetworkChange = { [weak self] in
       Task { @MainActor in
           self?.logger.info("Network changed — verifying server health")
           // Future: could restart server if binding changed
       }
   }
   advertiser.onPublishFailed = { [weak self] error in
       Task { @MainActor in
           self?.errorStore.append(AppError(
               severity: .warning,
               summary: "mDNS advertising failed",
               detail: error.localizedDescription,
               suggestion: "Devices on your network may not find this Mac automatically."
           ))
       }
   }
   ```

COMMIT: `fix[mac-network]: add network change callback, fix advertiser restart race, surface mDNS failures`

Verifica compilación:
```bash
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Debug -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet 2>&1 | tail -20
```
---
```

**Worker 4B — Server Lifecycle Monitoring** (depende de 4A)
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Monitorizar el Task del server para detectar crashes y reintentar.

Lee `mac/ClipSync/App.swift` (busca `startPipeline`).

CONTEXTO: `startPipeline()` lanza `server.start()` dentro de `Task.detached` — si el server crashea, nadie lo sabe.

CAMBIOS EN `App.swift`:

1. Reemplaza el fire-and-forget `Task.detached` del server con un Task monitorizado:
```swift
private func startPipeline() {
    // ... existing watcher setup ...
    
    serverTask = Task.detached { [weak self] in
        guard let server = await self?.server else { return }
        var retries = 0
        let maxRetries = 3
        
        while retries < maxRetries {
            do {
                try await server.start()
                // Si start() retorna normalmente, es shutdown limpio
                break
            } catch {
                retries += 1
                let logger = await self?.logger
                logger?.error("Server crashed (attempt \(retries)/\(maxRetries)): \(error)")
                
                if retries < maxRetries {
                    logger?.info("Restarting server in 5s...")
                    try? await Task.sleep(for: .seconds(5))
                } else {
                    logger?.error("Server failed after \(maxRetries) attempts, giving up")
                    await MainActor.run {
                        self?.errorStore.append(AppError(
                            severity: .error,
                            summary: "Server stopped unexpectedly",
                            detail: error.localizedDescription,
                            suggestion: "Restart ClipSync manually."
                        ))
                    }
                }
            }
        }
    }
}
```

2. Asegúrate de que `serverTask` se cancela en `applicationWillTerminate`.

COMMIT: `fix[mac-server]: monitor server task with auto-restart on crash`

Verifica compilación:
```bash
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Debug -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet 2>&1 | tail -20
```
---
```

### Phase 5: Fix Races + Shizuku Hash

**Bugs que resuelve:** B13 (hash mismatch), R1 (reconnect race), R2 (poll race)

**Worker 5A — Shizuku Hash + Race Fixes**
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Arreglar el hash mismatch de Shizuku y las race conditions de reconexión.

Lee `android/app/src/main/java/com/clipsync/service/ClipForegroundService.kt`:
- Busca `lastShizukuHash` — cómo se actualiza en onFrame vs pollViaShizuku
- Busca `reconnectRunnable` — cómo se programa en network change
- Busca `pollViaShizuku` — el loop de polling

CONTEXTO:
- Bug B13: En `onFrame()` para texto, hace `lastShizukuHash = text.hashCode()`. Pero `pollViaShizuku()` compara contra `mgr.getClipboardHash()`. Estos hashes son diferentes (Kotlin String hashCode vs clipboard content hash via Shizuku IPC). Resultado: poll ve hash diferente → reenvía al Mac → echo loop.
- Race R1: Network change clears host → viejo WS onFailure fires → scheduleReconnect race con discovery.
- Race R2: `handler.post { writeImage }` vs `pollViaShizuku` en mismo handler → poll puede correr antes de que el write se complete.

CAMBIOS:

1. Fix B13 — Shizuku hash consistency:
   En `onFrame()`, después de escribir texto via Shizuku (busca la línea con `shizukuManager?.setClipboardText(text)`):
   - CAMBIA: `lastShizukuHash = text.hashCode()`
   - POR: 
   ```kotlin
   // Use the same hash source as pollViaShizuku to prevent echo loops
   lastShizukuHash = shizukuManager?.getClipboardHash() ?: text.hashCode()
   ```
   Esto asegura que ambos lados comparan el mismo hash.

2. Fix R1 — Reconnect race:
   En el callback de `NetworkChangeObserver` (dentro de `onCreate`), añade cancel del WS actual ANTES de programar reconnect:
   ```kotlin
   networkObserver = NetworkChangeObserver(this) {
       // Cancel current connection first to prevent stale onFailure callbacks
       wsGeneration++  // Invalidate any pending callbacks
       ws?.cancel()
       ws = null
       connectedHost = null
       
       if (prefs.mode == Prefs.MODE_AUTO && prefs.host != null) {
           prefs.host = null
           L.event(M, "network change: host cleared, waiting for re-discovery")
       }
       backoffMs = INITIAL_BACKOFF_MS
       handler.removeCallbacks(reconnectRunnable)
       handler.post(reconnectRunnable)
   }
   ```
   El `wsGeneration++` al inicio invalida cualquier callback pendiente del WS viejo.

3. Fix R2 — Poll timing:
   En `onFrame()` para imágenes, mueve la actualización de `lastShizukuHash` DENTRO del `handler.post` block, DESPUÉS del write:
   ```kotlin
   handler.post {
       ClipboardWriter.lastMacWriteMs = System.currentTimeMillis()
       ClipboardWriter.writeImage(this@ClipForegroundService, uri, payload.mime)
       // Update hash AFTER write completes, on same handler thread as poll
       shizukuManager?.let { mgr ->
           if (mgr.isAvailable()) lastShizukuHash = mgr.getClipboardHash()
       }
       L.event(M, "image clipboard write bytes=$byteCount")
   }
   ```
   Como `pollViaShizuku` también corre en el handler thread, esto serializa write→hashUpdate→poll.

COMMIT: `fix[android-sync]: fix Shizuku hash mismatch and reconnect race conditions`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
---
```

## Validación final

Después de Phase 5, ejecuta:

```bash
# Mac
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Release -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet

# Android
cd ../android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:"
```

Si todo compila limpio:
1. Actualiza `docs/development/CONNECTION_STATE.md` → Status: COMPLETE
2. NO mergees a main — deja en la rama `fix/connection-robustness` para que el usuario revise

## Reglas

- Tú NO escribes código — solo los workers escriben código
- Workers son Sonnet (Agent tool, model: "sonnet") — más barato en tokens
- Validas TODO el output: `git diff`, build commands, grep para verificar
- Si un worker produce código que no compila, lanza uno nuevo con el error exacto del compilador
- Workers NO tienen contexto de esta conversación — prompts 100% auto-contenidos
- Commits siguen Conventional Commits: `feat[scope]`, `fix[scope]`, `refactor[scope]`
- Workers en la misma fase pueden ir en paralelo SI tocan archivos diferentes
- Workers que tocan el mismo archivo van en secuencia
- Si te quedas sin contexto, escribe CONTEXT_LIMIT en el state file con notas de handoff

## Paralelismo permitido

| Workers | ¿Paralelo? | Razón |
|---------|------------|-------|
| 1A + 1B | ✅ Sí | Archivos diferentes (Mac vs Android) |
| 2A → 2B | ❌ Secuencial | 2B depende de ServiceState creado en 2A |
| 3A → 3B | ❌ Secuencial | 3B puede tocar SettingsViewModel que 3A modifica indirectamente |
| 4A → 4B | ❌ Secuencial | 4B depende de callbacks de 4A |
| 5A | Solo | Último worker |

## EMPIEZA AHORA

Lee el plan (`docs/development/connection-robustness-plan.md`), crea el state file, la rama, y lanza Workers 1A + 1B en paralelo.
