# ClipSync General Improvements — Launch Prompt

> Copy everything below the line into a fresh Claude Code chat.
> Make sure you're in `/Users/2cristo7/Documents/personal-proyects/clip-sync` before pasting.

---

Eres el **Orquestador** de un pipeline multi-agente que va a implementar mejoras generales en ClipSync: seguridad, rendimiento, build config, UX, y calidad de código. Tu trabajo es lanzar workers Sonnet, validar su output, y avanzar fase por fase.

## Setup inicial (hazlo ANTES de lanzar nada)

1. Lee `docs/development/general-improvements-plan.md` — plan completo con ~40 issues en 5 fases.
2. Lee `CLAUDE.md` — convenciones de commits, estructura del proyecto, reglas de build.

3. Crea el archivo de estado:

Escribe `docs/development/IMPROVEMENTS_STATE.md`:
```markdown
# General Improvements — State
## Status: NOT_STARTED
## Current Phase: 1
## Completed Phases: []
## Branch: chore/general-improvements
## Last Commit: (none)
## Notes: Plan in docs/development/general-improvements-plan.md
```

4. Crea la rama:
```bash
git checkout -b chore/general-improvements
```

## Tu rol como Orquestador

Eres extremadamente eficiente en tokens. **NUNCA** lees código fuente completo ni escribes código. Solo:

1. Lees `docs/development/IMPROVEMENTS_STATE.md` para saber dónde estás
2. Lanzas workers Sonnet (Agent tool, model: "sonnet")
3. Verificas con `grep`, `git diff`, y build commands
4. Si un worker produce código que no compila, lanzas uno nuevo con el error exacto
5. Actualizas el state file después de cada tarea
6. Si te quedas sin contexto, escribes CONTEXT_LIMIT con notas de handoff

## Fases y Workers

---

### Phase 1: Security Hardening

**Worker 1A — Mac Rate Limiting + Input Validation** (Mac)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Añadir rate limiting y validación de input al servidor Mac.

Lee estos archivos:
- `mac/ClipSync/Server/ClipServer.swift`
- `mac/ClipSync/Server/AuthMiddleware.swift`
- `mac/ClipSync/Clipboard/ClipPayload.swift`
- `mac/ClipSync/Security/HMACValidator.swift`

CAMBIOS:

1. Crea `mac/ClipSync/Server/RateLimiter.swift`:
```swift
import Foundation

actor RateLimiter {
    private var requests: [String: [Date]] = [:]
    
    func allow(key: String, maxRequests: Int, windowSeconds: TimeInterval) -> Bool {
        let now = Date()
        let cutoff = now.addingTimeInterval(-windowSeconds)
        var timestamps = requests[key, default: []].filter { $0 > cutoff }
        guard timestamps.count < maxRequests else { return false }
        timestamps.append(now)
        requests[key] = timestamps
        return true
    }
    
    func reset(key: String) {
        requests.removeValue(forKey: key)
    }
}
```

2. Añádelo al pbxproj (rango IDs: A000000000000000000025xx, siguiente libre después de los usados).

3. En `ClipServer.swift`:
   - Añade propiedad `let rateLimiter = RateLimiter()`
   - En `/pair` endpoint, antes de procesar:
     ```swift
     let clientIP = request.headers.first(name: "X-Forwarded-For") ?? context.remoteAddress?.description ?? "unknown"
     guard await rateLimiter.allow(key: "pair:\(clientIP)", maxRequests: 5, windowSeconds: 60) else {
         throw HTTPError(.tooManyRequests)
     }
     ```
   - En `/inject` endpoint, antes de procesar:
     ```swift
     guard await rateLimiter.allow(key: "inject:\(clientIP)", maxRequests: 10, windowSeconds: 1) else {
         throw HTTPError(.tooManyRequests)
     }
     ```
   - Antes del JSONDecoder en /inject, añade size check:
     ```swift
     let estimatedSize = buffer.readableBytes * 3 / 4
     guard estimatedSize <= 20 * 1024 * 1024 else {
         throw HTTPError(.payloadTooLarge)
     }
     ```

4. En `ClipPayload.swift`, añade validación post-decode:
```swift
func validate() throws {
    guard ["text", "image", "file"].contains(type.rawValue) else {
        throw ValidationError.invalidType(type.rawValue)
    }
    guard !mime.isEmpty, mime.count <= 256 else {
        throw ValidationError.invalidMime
    }
    guard !nonce.isEmpty else {
        throw ValidationError.missingNonce
    }
    if let name = name, name.count > 1024 {
        throw ValidationError.nameTooLong
    }
    let now = Int64(Date().timeIntervalSince1970 * 1000)
    guard abs(now - ts) < 5 * 60 * 1000 else {
        throw ValidationError.timestampOutOfRange
    }
}

enum ValidationError: Error {
    case invalidType(String)
    case invalidMime
    case missingNonce
    case nameTooLong
    case timestampOutOfRange
}
```
Llama `try payload.validate()` en el endpoint `/inject` después del decode.

5. En `AuthMiddleware.swift` `extractBearer()`:
   - Añade: `guard parts[1].count <= 512 else { return nil }`

6. En `HMACValidator.swift`:
   - Cambia `skewSeconds: TimeInterval = 60` → `skewSeconds: TimeInterval = 30`
   - CUIDADO: revisa si hay tests que dependen de 60s. Si los hay, actualiza los tests también.

COMMIT: `fix[mac-security]: add rate limiting, payload validation, and reduce HMAC skew`

Verifica compilación:
```bash
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Debug -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet 2>&1 | tail -20
```
Si hay tests, ejecútalos:
```bash
cd mac && xcodebuild test -project ClipSync.xcodeproj -scheme ClipSync -destination 'platform=macOS' -quiet 2>&1 | tail -30
```
```

**Worker 1B — Android Payload Validation** (Android, paralelo con 1A)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Añadir validación de payload en Android antes de procesar frames.

Lee:
- `android/app/src/main/java/com/clipsync/model/ClipPayload.kt`
- `android/app/src/main/java/com/clipsync/model/ClipPayloadBuilder.kt` (para MAX_IMAGE_BYTES)
- `android/app/src/main/java/com/clipsync/service/ClipForegroundService.kt` (busca `onFrame`)

CAMBIOS:

1. En `ClipPayload.kt`, añade método de validación:
```kotlin
fun validate() {
    require(type in listOf("text", "image", "file")) { "Invalid type: $type" }
    require(mime.isNotEmpty() && mime.length <= 256) { "Invalid MIME: $mime" }
    require(nonce.isNotEmpty()) { "Empty nonce" }
    name?.let { require(it.length <= 1024) { "Name too long: ${it.length}" } }
    val nowMs = System.currentTimeMillis()
    require(kotlin.math.abs(nowMs - ts) < 5 * 60 * 1000) { "Timestamp out of range" }
}
```

2. En `ClipPayload.fromJson()`, llama `validate()` al final antes de retornar.

3. En `ClipForegroundService.onFrame()`, ANTES de cualquier base64 decode, añade:
```kotlin
private fun onFrame(payload: ClipPayload) {
    // Size guard: estimate decoded size from base64 length
    val estimatedBytes = payload.data.length * 3 / 4
    if (estimatedBytes > ClipPayloadBuilder.MAX_IMAGE_BYTES) {
        L.warn(M, "payload too large: ~${estimatedBytes / 1024}KB, dropping")
        return
    }
    // ... rest of existing onFrame code
}
```

4. Wrappea el `ClipPayload.fromJson(text)` en ClipClient.connectWebSocket onMessage con try-catch que loguea validation errors:
```kotlin
override fun onMessage(webSocket: WebSocket, text: String) {
    val payload = try {
        ClipPayload.fromJson(text)
    } catch (t: Throwable) {
        onStatus(WsStatus.Error("bad frame: ${t.message}"))
        return
    }
    onFrame(payload)
}
```
(Esto ya debería existir — solo verifica que el catch captura los nuevos require failures.)

COMMIT: `fix[android-security]: add payload validation and size guard before decode`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
```

**Worker 1C — Keychain Hardening** (Mac, paralelo con 1A y 1B)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Endurecer el nivel de accesibilidad del Keychain.

Lee `mac/ClipSync/Storage/Keychain.swift`.

CAMBIO:
En el método `save()`, cambia:
```swift
kSecAttrAccessibleAfterFirstUnlock
```
por:
```swift
kSecAttrAccessibleWhenUnlockedThisDeviceOnly
```

IMPORTANTE: El cambio de accessibility attribute puede hacer que items existentes NO se encuentren con el nuevo query si fueron guardados con el viejo atributo. Para migrar:
- En `load()`, si el item se encuentra con el query actual, bien.
- Si no, intenta load con el viejo atributo, y si existe, delete + re-save con el nuevo.
- O simplemente: la app regenerará los certs/secrets si no los encuentra. Verifica que `loadOrCreate()` en Keychain y `loadOrCreate()` en TLSManager manejan el caso de "not found" generando nuevos valores.

Si confirmas que loadOrCreate regenera correctamente, el cambio simple basta — items viejos se regeneran en el próximo arranque.

COMMIT: `fix[mac-security]: upgrade Keychain accessibility to WhenUnlockedThisDeviceOnly`

Verifica compilación y tests:
```bash
cd mac && xcodebuild test -project ClipSync.xcodeproj -scheme ClipSync -destination 'platform=macOS' -quiet 2>&1 | tail -30
```
```

---

### Phase 2: Performance & Battery

**Worker 2A — Android Battery Optimization** (Android)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Reducir consumo de batería en Android.

Lee `android/app/src/main/java/com/clipsync/service/ClipForegroundService.kt`.

CAMBIOS:

1. Cambia Shizuku poll interval:
   - Busca `SHIZUKU_POLL_MS = 500L` → cámbialo a `SHIZUKU_POLL_MS = 1000L`

2. Añade polling adaptativo en `pollViaShizuku()`:
   - Añade propiedad: `private var unchangedPolls = 0`
   - Si hash no cambió: `unchangedPolls++`
   - Si hash cambió: `unchangedPolls = 0`
   - Calcula delay adaptativo:
     ```kotlin
     val delay = when {
         unchangedPolls > 20 -> 2000L  // idle: poll every 2s
         unchangedPolls > 10 -> 1500L  // slowing down
         else -> SHIZUKU_POLL_MS       // active: 1s
     }
     handler.postDelayed(this, delay)
     ```
   - Reemplaza el `handler.postDelayed(this, SHIZUKU_POLL_MS)` fijo con el delay calculado.

3. Reemplaza Thread en health check con coroutine:
   - Busca el `Thread { ... }.start()` en el healthCheckRunnable
   - Reemplaza con un CoroutineScope:
     ```kotlin
     private val healthScope = CoroutineScope(Dispatchers.IO + SupervisorJob())
     ```
   - En el runnable:
     ```kotlin
     healthScope.launch {
         try {
             val ok = PairingApi().ping(host, port, fp)
             handler.post { /* handle result */ }
         } catch (t: Throwable) {
             handler.post { /* handle failure */ }
         }
     }
     ```
   - Cancela scope en onDestroy: `healthScope.cancel()`

4. En `SendClipActivity.kt`, busca `onDestroy()` y añade `scope.cancel()`:
   ```kotlin
   override fun onDestroy() {
       handler.removeCallbacks(fallbackRunnable)
       scope.cancel()
       super.onDestroy()
   }
   ```

COMMIT: `fix[android-perf]: adaptive Shizuku polling, coroutine health check, scope cleanup`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
```

**Worker 2B — Android Memory Safety** (Android, secuencial después de 2A)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Limitar uso de memoria en caché de imágenes y notificaciones.

Lee:
- `android/app/src/main/java/com/clipsync/images/ImageCache.kt`
- `android/app/src/main/java/com/clipsync/notifications/IncomingClipNotifier.kt`

CAMBIOS EN `ImageCache.kt`:

1. Añade límite de tamaño al cache:
```kotlin
companion object {
    private const val MAX_CACHE_SIZE_BYTES = 200L * 1024 * 1024 // 200MB
    // ... existing constants
}
```

2. Añade método de eviction:
```kotlin
fun enforceMaxSize() {
    val dir = cacheDir() ?: return
    val files = dir.listFiles()?.sortedBy { it.lastModified() } ?: return
    var totalSize = files.sumOf { it.length() }
    var evicted = 0
    for (file in files) {
        if (totalSize <= MAX_CACHE_SIZE_BYTES) break
        totalSize -= file.length()
        file.delete()
        evicted++
    }
    if (evicted > 0) L.event("ImageCache", "evicted $evicted files to stay under ${MAX_CACHE_SIZE_BYTES / 1024 / 1024}MB")
}
```

3. Llama `enforceMaxSize()` al final de `writeImage()` (después de guardar el archivo).

CAMBIOS EN `IncomingClipNotifier.kt`:

1. Busca donde se hace `BitmapFactory.decodeByteArray(bytes, 0, bytes.size)`.
2. Para imágenes grandes, usa inSampleSize:
```kotlin
val bitmap: Bitmap? = try {
    if (bytes.size > 5 * 1024 * 1024) {
        // Large image: subsample for notification preview
        val opts = BitmapFactory.Options().apply {
            inJustDecodeBounds = true
        }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
        opts.inSampleSize = calculateInSampleSize(opts, 512, 512)
        opts.inJustDecodeBounds = false
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
    } else {
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
    }
} catch (t: Throwable) {
    L.warn(M, "decodeByteArray failed: ${t.message}")
    null
}
```

3. Añade helper:
```kotlin
private fun calculateInSampleSize(options: BitmapFactory.Options, reqWidth: Int, reqHeight: Int): Int {
    val (height, width) = options.outHeight to options.outWidth
    var inSampleSize = 1
    if (height > reqHeight || width > reqWidth) {
        val halfHeight = height / 2
        val halfWidth = width / 2
        while (halfHeight / inSampleSize >= reqHeight && halfWidth / inSampleSize >= reqWidth) {
            inSampleSize *= 2
        }
    }
    return inSampleSize
}
```

4. Añade bitmap recycling con try-finally donde se usa bitmap para notification:
```kotlin
try {
    if (bitmap != null) {
        builder.setLargeIcon(bitmap)
        builder.setStyle(NotificationCompat.BigPictureStyle().bigPicture(bitmap))
    }
    // ... build notification
} finally {
    bitmap?.recycle()
}
```

COMMIT: `fix[android-memory]: add cache size limit, subsample large images, recycle bitmaps`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
```

**Worker 2C — Mac Polling** (Mac, paralelo con 2A/2B)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Mejorar polling del pasteboard en Mac.

Lee `mac/ClipSync/Clipboard/PasteboardWatcher.swift`.

CAMBIOS:

1. Cambia el default interval de 500ms a 750ms:
   - Busca `intervalMillis: Int = 500` → `intervalMillis: Int = 750`

2. Aumenta el echo suppression window:
   - Busca el límite de `suppressedDigests` (actualmente 8 entries)
   - Cámbialo a 16: `if suppressedDigests.count > 16 { suppressedDigests.removeFirst() }`

3. Extrae magic numbers a constantes (si no existen ya):
```swift
private enum Defaults {
    static let pollIntervalMs = 750
    static let maxSuppressedDigests = 16
    static let maxFileBytes = 20 * 1024 * 1024
}
```

COMMIT: `fix[mac-perf]: increase poll interval to 750ms and echo suppression window to 16`

Verifica compilación y tests:
```bash
cd mac && xcodebuild test -project ClipSync.xcodeproj -scheme ClipSync -destination 'platform=macOS' -quiet 2>&1 | tail -30
```
```

---

### Phase 3: Build & Config

**Worker 3A — Android Build Fixes** (Android)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Arreglar configuración de build Android.

Lee:
- `android/app/build.gradle.kts`
- `android/gradle/libs.versions.toml`
- `android/gradle.properties`

CAMBIOS:

1. En `build.gradle.kts`:
   - `targetSdk = 34` → `targetSdk = 35`
   - En el bloque release: `isMinifyEnabled = false` → `isMinifyEnabled = true`
   - Añade `isShrinkResources = true` junto a isMinifyEnabled

2. En `libs.versions.toml`:
   - `securityCrypto = "1.1.0-alpha06"` → `securityCrypto = "1.0.0"`
   (La versión 1.0.0 es estable y soporta EncryptedSharedPreferences.)
   NOTA: Si 1.0.0 causa errores de API, prueba con "1.1.0-alpha06" pero documenta por qué.

3. En `gradle.properties`:
   - `org.gradle.configuration-cache=false` → `org.gradle.configuration-cache=true`

4. Verifica que ProGuard rules (`android/app/proguard-rules.pro`) cubren:
   - OkHttp
   - Kotlin serialization
   - Compose
   - Shizuku
   Si falta algo, añádelo.

COMMIT: `chore[android-build]: bump targetSdk 35, enable R8 minification, stable securityCrypto`

Verifica compilación COMPLETA (release):
```bash
cd android && ./gradlew assembleRelease 2>&1 | tail -30
```
Si falla por ProGuard, lee el error y añade las reglas necesarias. Después:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
```

**Worker 3B — Cross-Platform Config** (ambos, paralelo con 3A)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Centralizar puerto y mejorar gitignore.

CAMBIOS:

1. En `.gitignore`, añade al final:
```
# Certificates and secrets
*.pem
*.key
*.p12
secrets/
```

2. Crear archivo `VERSION` en la raíz del repo:
```
0.1.0
```

3. En `mac/ClipSync/Server/ServerConfig.swift`:
   - Busca donde se define `port: 7010`
   - Añade lectura de env var:
   ```swift
   static var defaultPort: Int {
       if let envPort = ProcessInfo.processInfo.environment["CLIPSYNC_PORT"],
          let port = Int(envPort) {
           return port
       }
       return 7010
   }
   ```
   - Actualiza `ServerConfig.default` para usar `defaultPort`

4. En Android, busca los "7010" hardcodeados en:
   - `SettingsScreen.kt` (campo manual port)
   - `SettingsViewModel.kt` o `SettingsState`
   - `Prefs.kt`
   Reemplaza con constante:
   ```kotlin
   // En Prefs.kt o un nuevo Constants.kt:
   const val DEFAULT_PORT = 7010
   ```

COMMIT: `chore[config]: centralize port constant, add VERSION file, expand gitignore`

Verifica compilación ambas plataformas:
```bash
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Debug -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet 2>&1 | tail -10
cd ../android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
```

**Worker 3C — CI Improvements** (CI)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Mejorar CI workflow.

Lee `.github/workflows/ci.yml`.

CAMBIOS en `ci.yml`:

1. Pin macOS runner: `runs-on: macos-latest` → `runs-on: macos-14`

2. Añade cache de Gradle:
```yaml
# En el job de Android, antes del build step:
- name: Cache Gradle
  uses: actions/cache@v4
  with:
    path: |
      ~/.gradle/caches
      ~/.gradle/wrapper
    key: gradle-${{ hashFiles('**/*.gradle*', '**/gradle-wrapper.properties') }}
    restore-keys: gradle-
```

3. Añade upload de test artifacts:
```yaml
# En ambos jobs, después del test step:
- name: Upload test results
  if: always()
  uses: actions/upload-artifact@v4
  with:
    name: test-results-${{ matrix.platform || 'android' }}
    path: |
      mac/build/DerivedData/Logs/Test/
      android/app/build/reports/tests/
```

4. Asegúrate de que los steps de test tienen `if: always()` para que artifacts se suban aunque el test falle.

COMMIT: `chore[ci]: pin macOS runner, add Gradle cache, upload test artifacts`

No hay compilación que verificar — solo YAML. Valida sintaxis:
```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo "YAML valid"
```
```

---

### Phase 4: UX Polish

**Worker 4A — Android ErrorAction + Accessibility** (Android)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Implementar los handlers de ErrorAction y añadir accessibility labels.

Lee `android/app/src/main/java/com/clipsync/ui/SettingsScreen.kt` (busca TODO, ErrorAction, StatusDot).

CAMBIOS:

1. Busca los placeholders de ErrorAction y reemplaza:
   - `ErrorAction.Retry`:
     ```kotlin
     is ErrorAction.Retry -> {
         vm.dismissError(error.id)
         vm.bootstrap(context)
     }
     ```
   - `ErrorAction.Repair`:
     ```kotlin
     is ErrorAction.Repair -> {
         vm.dismissError(error.id)
         vm.unpair(context)
         // After unpair, discovery should auto-start
     }
     ```
   - `ErrorAction.OpenUrl`:
     ```kotlin
     is ErrorAction.OpenUrl -> {
         val intent = Intent(Intent.ACTION_VIEW, Uri.parse(action.url))
         context.startActivity(intent)
     }
     ```

2. Añade `contentDescription` a `StatusDot` composable:
```kotlin
@Composable
private fun StatusDot(status: ConnectionStatus) {
    val color = /* existing color logic */
    val description = when (status) {
        is ConnectionStatus.Disconnected -> "Disconnected"
        is ConnectionStatus.Connecting -> "Connecting"
        is ConnectionStatus.Connected -> "Connected to ${status.host}"
        is ConnectionStatus.Paused -> "Sync paused"
        is ConnectionStatus.Error -> "Error: ${status.reason}"
    }
    Box(
        modifier = Modifier
            .size(12.dp)
            .clip(CircleShape)
            .background(color)
            .semantics { contentDescription = description }
    )
}
```

3. Añade timeout a `PairingCodeDialog`:
   - Añade `LaunchedEffect(Unit) { delay(120_000); onDismiss() }` dentro del Dialog
   - Muestra countdown: `val remainingSeconds by produceState(120) { while (value > 0) { delay(1000); value-- } }`
   - Muestra texto "Code expires in {remainingSeconds}s"

COMMIT: `feat[android-ux]: implement ErrorAction handlers, accessibility labels, pairing timeout`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
```

**Worker 4B — Mac Sync Toggle** (Mac, paralelo con 4A)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Añadir toggle de pausa/resume sync en el menú bar del Mac.

Lee:
- `mac/ClipSync/UI/MenuBarController.swift`
- `mac/ClipSync/Clipboard/PasteboardWatcher.swift` (busca start/stop)
- `mac/ClipSync/App.swift` (busca cómo se conectan los componentes)

CAMBIOS:

1. En AppDelegate (App.swift), añade propiedad:
```swift
@Published var isSyncPaused = false
```

2. En MenuBarController, añade item de menú:
   - Después del item de "Connected devices" y antes de "Quit":
   ```swift
   let syncItem = NSMenuItem(
       title: isSyncPaused ? "Resume Sync" : "Pause Sync",
       action: #selector(handleToggleSync),
       keyEquivalent: ""
   )
   syncItem.target = self
   menu.addItem(syncItem)
   ```
   - Donde `isSyncPaused` se obtiene de AppDelegate (pásalo como parámetro o observa)

3. Añade handler:
```swift
@objc private func handleToggleSync() {
    // Toggle via callback to AppDelegate
    onToggleSync?()
}
var onToggleSync: (() -> Void)?
```

4. En AppDelegate, conecta el callback:
```swift
menuBar.onToggleSync = { [weak self] in
    guard let self else { return }
    self.isSyncPaused.toggle()
    if self.isSyncPaused {
        self.watcher?.stop()
        self.logger.info("Sync paused by user")
    } else {
        self.watcher?.start()
        self.logger.info("Sync resumed by user")
    }
}
```

5. En MenuBarController, cuando sync está pausado, muestra icono diferente:
   - Usa el mismo icono pero con un badge gris (o simplemente cambia el tooltip)

COMMIT: `feat[mac-ux]: add pause/resume sync toggle to menu bar`

Verifica compilación:
```bash
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Debug -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet 2>&1 | tail -20
```
```

**Worker 4C — Android ClipSender Retry** (Android, secuencial después de 4A)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Añadir retry con backoff a ClipSender para errores transitorios.

Lee `android/app/src/main/java/com/clipsync/overlay/ClipSender.kt`.

CAMBIOS:

1. Clasifica errores:
```kotlin
private fun isTransient(t: Throwable): Boolean = when {
    t is java.net.SocketTimeoutException -> true
    t is java.net.ConnectException -> true
    t is java.io.IOException && t.message?.contains("timeout") == true -> true
    t is okhttp3.internal.http2.StreamResetException -> true
    else -> false
}

private fun isTransientHttp(code: Int): Boolean = code in 500..599 || code == 429
```

2. Añade retry al método `post()`:
```kotlin
suspend fun post(/* existing params */): Result {
    var lastError: Throwable? = null
    val maxRetries = 3
    val backoffMs = longArrayOf(1000, 2000, 4000)
    
    repeat(maxRetries) { attempt ->
        try {
            val resp = client.newCall(request).execute()
            if (resp.isSuccessful) {
                return Result.Ok
            }
            if (!isTransientHttp(resp.code)) {
                return Result.Failed("HTTP ${resp.code}: ${resp.body?.string()?.take(200) ?: "no body"}")
            }
            L.warn(M, "transient HTTP ${resp.code}, retry ${attempt + 1}/$maxRetries")
        } catch (t: Throwable) {
            if (!isTransient(t)) {
                return Result.Failed(t.message ?: "network error")
            }
            lastError = t
            L.warn(M, "transient error: ${t.message}, retry ${attempt + 1}/$maxRetries")
        }
        if (attempt < maxRetries - 1) {
            delay(backoffMs[attempt])
        }
    }
    return Result.Failed("Failed after $maxRetries attempts: ${lastError?.message}")
}
```

3. Si `post()` no es `suspend`, hazla suspend o usa `Thread.sleep` para el backoff.

4. También parsea el response body en errores para dar mejor feedback:
   - Antes: `Result.Failed("HTTP ${resp.code}")`
   - Después: `Result.Failed("HTTP ${resp.code}: ${resp.body?.string()?.take(200)}")`

COMMIT: `feat[android-network]: add retry with backoff for transient send errors`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
```

---

### Phase 5: Code Quality (Refactors)

**Worker 5A — Split SettingsScreen** (Android)
```
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Dividir SettingsScreen.kt (953 líneas) en sub-composables por sección.

Lee `android/app/src/main/java/com/clipsync/ui/SettingsScreen.kt` completo.

CAMBIOS:

1. Crea `android/app/src/main/java/com/clipsync/ui/sections/ConnectionSection.kt`:
   - Extrae la sección de modo de conexión (Auto/Manual/Tailscale toggle + discovery list)
   - Composable: `@Composable fun ConnectionSection(state: SettingsState, vm: SettingsViewModel, context: Context)`

2. Crea `android/app/src/main/java/com/clipsync/ui/sections/PermissionsSection.kt`:
   - Extrae todas las PermissionRow calls + Shizuku section
   - Composable: `@Composable fun PermissionsSection(state: SettingsState, vm: SettingsViewModel, context: Context)`

3. Crea `android/app/src/main/java/com/clipsync/ui/sections/TailscaleSection.kt`:
   - Extrae la sección de Tailscale
   - Composable: `@Composable fun TailscaleSection(state: SettingsState, vm: SettingsViewModel, context: Context)`

4. `SettingsScreen.kt` queda como orchestrador que llama a las secciones. Debe quedar en ~300 líneas máximo.

5. Mueve `DiscoveredServerCard`, `PairingCodeDialog`, `PermissionRow`, `StatusDot` a sus respectivos archivos de sección o a un archivo `components/` compartido.

6. Elimina Clay* aliases de NeuComponents.kt (líneas ~405-436) — son dead code.

REGLA: No cambies lógica ni UI — solo reorganiza. El resultado visual debe ser idéntico.

COMMIT: `refactor[android-ui]: split SettingsScreen into section composables`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
```

---

## Validación final

Después de Phase 5:

```bash
# Mac — build + tests
cd mac && xcodebuild test -project ClipSync.xcodeproj -scheme ClipSync -destination 'platform=macOS' -quiet 2>&1 | tail -30

# Android — build
cd ../android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:"

# Android — release build (R8 enabled)
cd android && ./gradlew assembleRelease 2>&1 | tail -20
```

Si todo pasa:
1. Actualiza `docs/development/IMPROVEMENTS_STATE.md` → Status: COMPLETE
2. NO mergees a main — deja en rama `chore/general-improvements` para review

## Reglas

- Tú NO escribes código — solo workers Sonnet
- Validas TODO: `git diff`, build commands, grep
- Si un worker falla, lanza uno nuevo con el error exacto del compilador
- Prompts 100% auto-contenidos
- Commits: `feat[scope]`, `fix[scope]`, `chore[scope]`, `refactor[scope]`
- CONTEXT_LIMIT → state file con handoff notes

## Paralelismo

| Workers | ¿Paralelo? | Razón |
|---------|------------|-------|
| 1A + 1B + 1C | ✅ Sí | Mac vs Android vs Keychain — archivos distintos |
| 2A → 2B | ❌ Secuencial | 2B puede tocar archivos modificados por 2A |
| 2C | ✅ Paralelo con 2A/2B | Mac vs Android |
| 3A + 3B + 3C | ✅ Sí | Build, config, CI — archivos distintos |
| 4A → 4C | ❌ Secuencial | Ambos Android |
| 4B | ✅ Paralelo con 4A | Mac vs Android |
| 5A | Solo | Refactor grande, debe ir último |

## EMPIEZA AHORA

Lee el plan, crea state file, crea rama, y lanza Workers 1A + 1B + 1C en paralelo.
