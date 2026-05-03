# ClipSync Error Handling UX — Launch Prompt

> Copy everything below the line into a fresh Claude Code chat.
> Make sure you're in `/Users/2cristo7/Documents/personal-proyects/clip-sync` before pasting.

---

Eres el **Orquestador** de un pipeline multi-agente que va a implementar el overhaul de error handling de ClipSync. Tu trabajo es lanzar workers Sonnet para cada tarea, validar su output, y avanzar fase por fase.

## Setup inicial (hazlo ANTES de lanzar nada)

1. Lee `docs/development/error-handling-plan.md` — es el plan completo con 5 fases, diseño de UI, y clasificación de errores.

2. Lee `CLAUDE.md` — convenciones de commits, estructura del proyecto, reglas de build.

3. Crea el archivo de estado:

```bash
mkdir -p docs/development
```

Escribe `docs/development/ERROR_HANDLING_STATE.md`:
```markdown
# Error Handling Overhaul — State
## Status: NOT_STARTED
## Current Phase: 1
## Completed Phases: []
## Branch: feature/error-handling-ux
## Last Commit: (none)
## Notes: Plan in docs/development/error-handling-plan.md
```

4. Crea la rama:
```bash
git checkout -b feature/error-handling-ux
```

## Tu rol como Orquestador

Eres extremadamente eficiente en tokens. **NUNCA** lees código fuente completo ni escribes código. Solo:

1. Lees `docs/development/ERROR_HANDLING_STATE.md` para saber dónde estás
2. Lanzas workers Sonnet (Agent tool, model: "sonnet") para cada tarea
3. Después de cada worker: verificas con `grep`, `git diff`, y build commands
4. Si un worker produce código malo, lanzas uno nuevo para arreglar
5. Actualizas el state file después de cada tarea completada
6. Si te quedas sin contexto, escribes CONTEXT_LIMIT en el state file con notas de handoff

## Fases y tareas

### Phase 1: Error Model + Mac TLS Warning
**Prioridad máxima — el usuario no sabe si TLS falló**

**Worker 1A — Error Model (Mac)**
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Crear el modelo de errores para la app Mac.

1. Crea `mac/ClipSync/UI/ErrorState.swift`:

```swift
import Foundation

enum ErrorSeverity {
    case warning, error
}

struct AppError: Identifiable {
    let id = UUID()
    let severity: ErrorSeverity
    let summary: String
    let detail: String
    let suggestion: String?
    let timestamp = Date()
}

@MainActor
final class ErrorStore: ObservableObject {
    @Published private(set) var errors: [AppError] = []

    func append(_ error: AppError) {
        errors.append(error)
    }

    func dismiss(_ id: UUID) {
        errors.removeAll { $0.id == id }
    }

    func dismissAll() {
        errors.removeAll()
    }

    var hasErrors: Bool { errors.contains { $0.severity == .error } }
    var hasWarnings: Bool { errors.contains { $0.severity == .warning } }
}
```

2. Añade `ErrorState.swift` al proyecto Xcode (`mac/ClipSync.xcodeproj/project.pbxproj`):
   - Lee el pbxproj, busca el patrón de cómo están añadidos los otros archivos en UI/ (ej: MenuBarController.swift, PairingWindow.swift)
   - Usa IDs en rango A000000000000000000025xx (siguiente rango libre según CLAUDE.md)
   - Necesitas: PBXBuildFile, PBXFileReference, PBXGroup (dentro del grupo UI), PBXSourcesBuildPhase

3. Añade `LocalizedError` conformance a `TLSManagerError` en `mac/ClipSync/Security/TLSManager.swift`:
```swift
extension TLSManagerError: LocalizedError {
    var errorDescription: String? {
        switch self {
        case .serializationFailed: return "Failed to serialize TLS certificate"
        case .storageFailed: return "Failed to store TLS certificate in Keychain"
        }
    }
    var recoverySuggestion: String? {
        switch self {
        case .serializationFailed: return "Restart ClipSync. If the problem persists, delete ClipSync items in Keychain Access."
        case .storageFailed: return "Check Keychain Access permissions. Restart your Mac if error -34018 appears."
        }
    }
}
```

4. Añade `LocalizedError` conformance a `KeychainError` en `mac/ClipSync/Storage/Keychain.swift`:
```swift
extension KeychainError: LocalizedError {
    var errorDescription: String? {
        switch self {
        case .unexpectedStatus(let s): return "Keychain error (OSStatus \(s))"
        case .notFound: return "Item not found in Keychain"
        case .dataConversionFailure: return "Failed to decode Keychain data"
        case .randomGenerationFailed(let s): return "Secure random generation failed (OSStatus \(s))"
        }
    }
}
```

COMMIT: `feat[mac-errors]: add AppError model, ErrorStore, and LocalizedError conformances`

Verifica compilación:
```bash
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Debug -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet 2>&1 | tail -5
```
---
```

**Worker 1B — Surface TLS Fallback (Mac)**
Depende de 1A. Lanzar después.
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

CONTEXTO: Ya existe `mac/ClipSync/UI/ErrorState.swift` con `AppError` y `ErrorStore`.

TAREA: Hacer que App.swift use ErrorStore y surfacee TLS failures.

1. En `mac/ClipSync/App.swift` (clase AppDelegate):
   - Añade propiedad: `let errorStore = ErrorStore()`
   - Pasa `errorStore` a `MenuBarController` (añade parámetro en su init o como propiedad)
   - En `startPipeline()`, donde se hace catch del TLS error y se cae a HTTP:
     - ANTES del fallback, añade:
     ```swift
     errorStore.append(AppError(
         severity: .warning,
         summary: "Running without TLS encryption",
         detail: "TLS setup failed: \(error.localizedDescription)",
         suggestion: "Restart ClipSync. Clipboard data will be sent unencrypted on your network."
     ))
     ```
   - En el catch de server start failure, añade AppError similar con severity .error

2. En `mac/ClipSync/UI/MenuBarController.swift`:
   - Añade `let errorStore: ErrorStore` (recibido de AppDelegate)
   - En `rebuildMenu()`:
     - Si `errorStore.hasErrors` o `errorStore.hasWarnings`, añade al INICIO del menú:
       - Un item deshabilitado con título "⚠ {count} issue(s)" si hay warnings
       - Un item deshabilitado con título "🔴 {count} error(s)" si hay errors
       - Para cada error en `errorStore.errors`: un item con `summary` como título
       - Un separator después de los errores
     - Cambia el icono del menu bar: si `hasErrors` → pinta badge rojo, si `hasWarnings` → badge naranja
   - En `startObservingHub()` o similar, observa `errorStore.$errors` para llamar `rebuildMenu()` cuando cambien

COMMIT: `feat[mac-errors]: surface TLS fallback and server errors in menu bar`

Verifica compilación:
```bash
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Debug -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet 2>&1 | tail -5
```
---
```

### Phase 2: Android Error Banner + Silent Catch Fixes

**Worker 2A — Error Model + Banner (Android)**
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Crear modelo de errores y componente ErrorBanner para Android.

1. Crea `android/app/src/main/java/com/clipsync/model/AppError.kt`:
```kotlin
package com.clipsync.model

import java.util.UUID

enum class ErrorSeverity { WARNING, ERROR }

sealed class ErrorAction {
    data object Retry : ErrorAction()
    data object Repair : ErrorAction()
    data class OpenUrl(val url: String) : ErrorAction()
}

data class AppError(
    val id: String = UUID.randomUUID().toString(),
    val severity: ErrorSeverity,
    val summary: String,
    val detail: String? = null,
    val suggestion: String? = null,
    val action: ErrorAction? = null
)
```

2. Crea `android/app/src/main/java/com/clipsync/ui/components/ErrorBanner.kt`:
```kotlin
package com.clipsync.ui.components

// Composable ErrorBanner:
// - NeuCard con borde rojo (ERROR) o naranja (WARNING)
// - Muestra summary siempre
// - Botón "▸ Details" que expande con animateContentSize para mostrar detail + suggestion
// - Botón "Dismiss" (icono X) que llama onDismiss
// - Si action != null, botón con label contextual ("Retry", "Re-pair", etc.)
// - Botón "Copy" (icono) que copia detail al clipboard
// - Usa colores del theme existente (NeuColors)
```
Lee `android/app/src/main/java/com/clipsync/ui/theme/NeuComponents.kt` para los colores y estilos existentes.

3. En `android/app/src/main/java/com/clipsync/ui/SettingsViewModel.kt`:
   - En `SettingsState`: reemplaza `val error: String? = null` con `val errors: List<AppError> = emptyList()`
   - Añade helper methods al ViewModel:
     ```kotlin
     private fun addError(error: AppError) {
         state = state.copy(errors = state.errors + error)
     }
     fun dismissError(id: String) {
         state = state.copy(errors = state.errors.filter { it.id != id })
     }
     ```
   - Donde antes hacía `state = state.copy(error = ...)`, usa `addError(AppError(...))` en su lugar

4. En `android/app/src/main/java/com/clipsync/ui/SettingsScreen.kt`:
   - Reemplaza el bloque `state.error?.let { ... NeuCard ... }` (aprox línea 726-735) con:
     ```kotlin
     state.errors.forEach { error ->
         ErrorBanner(
             error = error,
             onDismiss = { vm.dismissError(error.id) },
             onAction = { /* handle based on error.action type */ }
         )
     }
     ```

COMMIT: `feat[android-errors]: add AppError model, ErrorBanner composable, migrate SettingsState`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
---
```

**Worker 2B — Fix Silent Catches (Android)**
Depende de 2A. Lanzar después.
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

CONTEXTO: Ya existe `AppError` en `com.clipsync.model` y `addError()` en `SettingsViewModel`.

TAREA: Arreglar los silent catches en SettingsViewModel para que surfaceen errores al usuario.

1. `SettingsViewModel.bootstrap()` (~línea 87-128):
   - El catch actual solo loguea. Añade:
   ```kotlin
   addError(AppError(
       severity = ErrorSeverity.ERROR,
       summary = "Startup failed",
       detail = e.stackTraceToString().take(500),
       suggestion = "Restart the app. If the problem persists, try clearing app data.",
       action = ErrorAction.Retry
   ))
   ```

2. `SettingsViewModel.requestShizukuPermission()` (~línea 452-473):
   - El catch actual solo loguea. Añade:
   ```kotlin
   addError(AppError(
       severity = ErrorSeverity.WARNING,
       summary = "Shizuku permission request failed",
       detail = e.message,
       suggestion = "Make sure Shizuku is running. Open Shizuku app and start the service."
   ))
   ```

3. `SettingsViewModel.pair()` (~línea 215-268):
   - El catch actual ya pone error en state. Mejóralo clasificando excepciones:
   ```kotlin
   catch (t: Throwable) {
       val appError = when {
           t is javax.net.ssl.SSLHandshakeException ->
               AppError(severity = ErrorSeverity.ERROR, summary = "Certificate mismatch",
                   detail = t.message, suggestion = "The Mac app regenerated its certificate. Re-pair to fix.",
                   action = ErrorAction.Repair)
           t is java.net.ConnectException ->
               AppError(severity = ErrorSeverity.ERROR, summary = "Server unreachable",
                   detail = t.message, suggestion = "Check that both devices are on the same network.",
                   action = ErrorAction.Retry)
           t is com.clipsync.net.PairingException ->
               AppError(severity = ErrorSeverity.ERROR, summary = "Pairing failed",
                   detail = t.message, suggestion = "Check the pairing code and try again.")
           else ->
               AppError(severity = ErrorSeverity.ERROR, summary = "Connection failed",
                   detail = t.message ?: "Unknown error", suggestion = "Try again.")
       }
       addError(appError)
       state = state.copy(status = ConnectionStatus.Error(appError.summary))
   }
   ```

4. `SettingsViewModel.startNetworkWatch()` (~línea 475-513):
   - El catch en el flow collection solo loguea. Añade:
   ```kotlin
   addError(AppError(
       severity = ErrorSeverity.WARNING,
       summary = "Network discovery interrupted",
       detail = e.message,
       suggestion = "Discovery will restart on next network change."
   ))
   ```

COMMIT: `fix[android-errors]: surface all silent catches as user-visible AppErrors`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
---
```

### Phase 3: Mac Server + WebSocket Error Propagation

**Worker 3A — Server Typed Errors + WebSocket Notifications (Mac)**
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

CONTEXTO: Ya existe `ErrorStore` en `mac/ClipSync/UI/ErrorState.swift`, accesible desde AppDelegate.

TAREA: Mejorar error propagation en ClipServer y WebSocketHub.

1. En `mac/ClipSync/Server/ClipServer.swift`:
   - Añade propiedad `let errorStore: ErrorStore` (recibirlo desde AppDelegate)
   - En el catch de startup donde detecta "address in use" por string matching:
     - Reemplaza string match con detección más robusta (si el error contiene "address already in use" O es `IOError` con errno EADDRINUSE)
     - Añade: `errorStore.append(AppError(severity: .error, summary: "Port \(port) already in use", detail: error.localizedDescription, suggestion: "Close other ClipSync instances or change the port."))`
   - Para otros errores de startup:
     - Añade: `errorStore.append(AppError(severity: .error, summary: "Server failed to start", detail: error.localizedDescription, suggestion: "Check the logs and restart ClipSync."))`

2. En `mac/ClipSync/Server/WebSocketHub.swift`:
   - Añade propiedad `let errorStore: ErrorStore` (recibirlo desde ClipServer o AppDelegate)
   - Cuando un client se dropea por write failure (actualmente solo log debug):
     - Añade: `errorStore.append(AppError(severity: .warning, summary: "Device disconnected", detail: "WebSocket write failed for client", suggestion: "The device will reconnect automatically."))`
   - Asegúrate de no spammear: solo añade si no hay ya un error con mismo summary en los últimos 5 segundos

3. Pasa `errorStore` por la cadena: AppDelegate → ClipServer → WebSocketHub

COMMIT: `feat[mac-errors]: propagate server and WebSocket errors to ErrorStore`

Verifica compilación:
```bash
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Debug -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet 2>&1 | tail -5
```
---
```

### Phase 4: Android Discovery + Network Errors

**Worker 4A — NsdDiscovery Error Propagation (Android)**
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Mejorar NsdDiscovery para emitir errores, y PairingApi.ping() para devolver más contexto.

1. En `android/app/src/main/java/com/clipsync/discovery/NsdDiscovery.kt`:
   - Crea sealed class para eventos:
     ```kotlin
     sealed class DiscoveryEvent {
         data class Found(val info: NsdServiceInfo) : DiscoveryEvent()
         data class Lost(val name: String) : DiscoveryEvent()
         data class Error(val message: String) : DiscoveryEvent()
     }
     ```
   - Cambia el flow para emitir `DiscoveryEvent` en vez de solo `NsdServiceInfo`
   - En `onStartDiscoveryFailed`: emit `DiscoveryEvent.Error("Discovery failed to start (error $errorCode)")` antes de cerrar el flow
   - En `onResolveFailed`: emit `DiscoveryEvent.Error("Failed to resolve service (error $errorCode)")`

2. En `android/app/src/main/java/com/clipsync/net/PairingApi.kt`:
   - Cambia `ping()` para devolver `Result<Boolean>` en vez de `Boolean`:
     ```kotlin
     suspend fun ping(): Result<Boolean> = runCatching {
         // existing logic, but let exceptions propagate
     }
     ```
   - Los callers que usen `ping()` ahora pueden inspeccionar `result.exceptionOrNull()` para saber POR QUÉ falló

3. En `SettingsViewModel`:
   - Actualiza `startDiscovery()` para manejar `DiscoveryEvent.Error`:
     ```kotlin
     is DiscoveryEvent.Error -> {
         addError(AppError(
             severity = ErrorSeverity.WARNING,
             summary = "Can't find servers on network",
             detail = event.message,
             suggestion = "Check that both devices are on the same Wi-Fi network."
         ))
     }
     ```
   - Actualiza callers de `ping()` para usar `Result`

COMMIT: `feat[android-errors]: propagate discovery and ping errors with context`

Verifica compilación:
```bash
cd android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
```
---
```

### Phase 5: Polish — macOS Notification + Copy Error

**Worker 5A — Final Polish (ambas plataformas)**
```
Prompt para worker:
---
REPO: /Users/2cristo7/Documents/personal-proyects/clip-sync
Lee CLAUDE.md para convenciones.

TAREA: Polish final del error handling.

1. Mac — Notificación nativa para errores críticos:
   - En `mac/ClipSync/UI/ErrorState.swift`, añade método a `ErrorStore`:
     ```swift
     func appendAndNotify(_ error: AppError) {
         append(error)
         if error.severity == .error {
             let notification = NSUserNotification()
             notification.title = "ClipSync"
             notification.informativeText = error.summary
             NSUserNotificationCenter.default.deliver(notification)
         }
     }
     ```
   - O usa `UNUserNotificationCenter` si el target es macOS 14+. Revisa qué usa el proyecto.
   - Reemplaza `errorStore.append()` por `errorStore.appendAndNotify()` en App.swift y ClipServer para errores de severity .error

2. Android — Botón "Copy Error" en ErrorBanner:
   - En `ErrorBanner.kt`, asegúrate de que el botón Copy funciona:
     ```kotlin
     val clipboardManager = LocalContext.current.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
     clipboardManager.setPrimaryClip(ClipData.newPlainText("ClipSync Error", "${error.summary}\n${error.detail}\n${error.suggestion}"))
     ```

3. Ambas plataformas — Limpieza:
   - Mac: auto-dismiss warnings después de 5 minutos (errores persisten hasta dismiss manual)
   - Android: limitar lista de errores a máximo 10 (FIFO, los más viejos se borran)

COMMIT: `feat[errors]: add native notifications for critical errors and copy-error support`

Verifica compilación en ambas plataformas:
```bash
cd mac && xcodebuild build -project ClipSync.xcodeproj -scheme ClipSync -configuration Debug -derivedDataPath build/DerivedData CODE_SIGN_IDENTITY="-" CODE_SIGNING_REQUIRED=YES CODE_SIGNING_ALLOWED=YES -quiet 2>&1 | tail -5
cd ../android && ./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:" | head -20
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
1. Actualiza `docs/development/ERROR_HANDLING_STATE.md` → Status: COMPLETE
2. NO mergees a main — deja en la rama `feature/error-handling-ux` para que el usuario revise

## Reglas

- Tú NO escribes código — solo los workers escriben código
- Workers son Sonnet (Agent tool, model: "sonnet") — más barato en tokens
- Validas TODO el output: `git diff`, build commands, grep para verificar que el código existe
- Si un worker produce código que no compila, lanza uno nuevo con el error exacto para que lo arregle
- Workers NO tienen contexto de esta conversación — sus prompts deben ser 100% auto-contenidos
- Commits siguen Conventional Commits: `feat[scope]: message`, `fix[scope]: message`
- Workers independientes pueden lanzarse en paralelo. Workers dependientes van en secuencia.
- Si te quedas sin contexto, escribe CONTEXT_LIMIT en el state file con notas detalladas de handoff

## EMPIEZA AHORA

Lee el plan (`docs/development/error-handling-plan.md`), crea el state file, la rama, y lanza Worker 1A.
