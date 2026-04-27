# Plan: AccessibilityService para auto-send de clipboard

## Contexto

Actualmente, enviar el clipboard de Android al Mac requiere que el usuario
toque la FAB manualmente. El objetivo es detectar automáticamente cuando el
usuario copia algo y enviarlo al Mac sin interacción adicional.

**Por qué AccessibilityService y no solo `OnPrimaryClipChangedListener`:**
Un foreground service puede recibir el evento de cambio de clipboard, pero
`getPrimaryClip()` devuelve `null` desde background (Android 10+). Un
`AccessibilityService` tiene privilegio elevado de sistema y **sí puede leer
el contenido del clipboard desde background**. Esto permite envío 100%
automático.

---

## Arquitectura propuesta

```
[Usuario copia algo en cualquier app]
          |
          v
[ClipAccessibilityService]
  - OnPrimaryClipChangedListener dispara
  - getPrimaryClip() devuelve contenido (privilegio de accesibilidad)
  - Comprueba: ¿es un eco? (label == "clipsync") → skip
  - Comprueba: ¿sync pausado? → skip
  - Comprueba: ¿hay pairing? → skip si no
  - Construye ClipPayload via ClipPayloadBuilder
  - Envía via ClipSender.send() en coroutine IO
  - Notifica resultado via broadcast → FAB muestra feedback
```

### Diagrama de flujo con prevención de eco

```
Mac envía clipboard → WebSocket → ClipForegroundService
  → IncomingClipNotifier → ApplyClipActivity
    → ClipboardWriter.writeText(label="clipsync")
      → OnPrimaryClipChangedListener dispara
        → clip.description.label == "clipsync" → SKIP (no eco)

Usuario copia algo en Chrome (label="Chrome", etc.)
  → OnPrimaryClipChangedListener dispara
    → clip.description.label != "clipsync" → ENVIAR al Mac
```

---

## Archivos a crear

### 1. `android/app/src/main/java/com/clipsync/accessibility/ClipAccessibilityService.kt`

Servicio de accesibilidad que escucha cambios de clipboard y envía automáticamente.

```kotlin
package com.clipsync.accessibility

class ClipAccessibilityService : AccessibilityService() {

    private lateinit var clipboardManager: ClipboardManager
    private lateinit var prefs: Prefs
    private val sender = ClipSender()
    private val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())

    private val clipListener = OnPrimaryClipChangedListener {
        handleClipChange()
    }

    override fun onServiceConnected() {
        clipboardManager = getSystemService(ClipboardManager::class.java)
        prefs = Prefs(applicationContext)
        clipboardManager.addPrimaryClipChangedListener(clipListener)
    }

    private fun handleClipChange() {
        // Guard: sync pausado
        if (!prefs.syncEnabled) return

        // Guard: no hay pairing
        if (!prefs.hasPairing() || prefs.pairingSecret.isNullOrEmpty()) return

        val clip = clipboardManager.primaryClip ?: return
        if (clip.itemCount == 0) return

        // Guard: prevención de eco — ClipboardWriter usa label "clipsync"
        val label = clip.description?.label?.toString() ?: ""
        if (label == ClipboardWriter.LABEL) return

        val item = clip.getItemAt(0)
        val mimeType = clip.description?.getMimeType(0) ?: ""

        // Extraer credenciales una vez
        val host = prefs.host ?: return
        val port = prefs.port
        val token = prefs.token ?: return
        val secret = prefs.pairingSecret ?: return
        val fp = prefs.fp ?: return

        when {
            mimeType.startsWith("text/") || item.text != null -> {
                val text = item.coerceToText(this)?.toString()
                if (text.isNullOrBlank()) return
                val payload = ClipPayloadBuilder.text(text)
                sendAsync(host, port, token, secret, fp, payload)
            }
            mimeType.startsWith("image/") -> {
                val uri = item.uri ?: return
                scope.launch(Dispatchers.IO) {
                    try {
                        val stream = contentResolver.openInputStream(uri) ?: return@launch
                        val bytes = stream.use { it.readBytes() }
                        if (bytes.size > ClipPayloadBuilder.MAX_IMAGE_BYTES) return@launch
                        val mime = contentResolver.getType(uri) ?: mimeType
                        val payload = ClipPayloadBuilder.image(mime, bytes)
                        sendAndBroadcast(host, port, token, secret, fp, payload)
                    } catch (_: Throwable) { }
                }
            }
        }
    }

    private fun sendAsync(host: String, port: Int, token: String,
                          secret: String, fp: String, payload: ClipPayload) {
        scope.launch(Dispatchers.IO) {
            sendAndBroadcast(host, port, token, secret, fp, payload)
        }
    }

    private suspend fun sendAndBroadcast(host: String, port: Int, token: String,
                                         secret: String, fp: String, payload: ClipPayload) {
        val result = sender.send(host, port, token, secret, fp, payload)
        withContext(Dispatchers.Main) {
            sendBroadcast(Intent(SendClipActivity.ACTION_SEND_RESULT).apply {
                setPackage(packageName)
                putExtra(SendClipActivity.EXTRA_SUCCESS, result is ClipSender.Result.Ok)
            })
        }
    }

    override fun onDestroy() {
        clipboardManager.removePrimaryClipChangedListener(clipListener)
        scope.cancel()
        super.onDestroy()
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent) = Unit
    override fun onInterrupt() = Unit
}
```

**Notas clave del diseño:**
- `ClipboardWriter.LABEL` ya existe como `"clipsync"` — lo usamos como
  marcador para detectar escrituras propias y evitar ecos.
- No necesitamos `canRetrieveWindowContent` ni eventos de accesibilidad —
  solo usamos el contexto privilegiado para el clipboard listener.
- El resultado se emite por broadcast con el mismo `ACTION_SEND_RESULT`
  que ya usa `SendClipActivity`, así la FAB muestra feedback sin cambios.

### 2. `android/app/src/main/res/xml/accessibility_service_config.xml`

Configuración mínima — no espiamos pantallas ni interacciones UI.

```xml
<?xml version="1.0" encoding="utf-8"?>
<accessibility-service
    xmlns:android="http://schemas.android.com/apk/res/android"
    android:accessibilityEventTypes=""
    android:accessibilityFeedbackType="feedbackGeneric"
    android:accessibilityFlags="flagDefault"
    android:canRetrieveWindowContent="false"
    android:notificationTimeout="0"
    android:description="@string/accessibility_service_description" />
```

### 3. `android/app/src/main/res/values/strings.xml` — Nuevo string

```xml
<string name="accessibility_service_description">
    ClipSync usa este servicio para detectar cuando copias algo y enviarlo
    automáticamente a tu Mac. No lee ni analiza el contenido de las pantallas.
</string>
```

---

## Archivos a modificar

### 4. `android/app/src/main/AndroidManifest.xml`

Añadir la declaración del servicio dentro de `<application>`:

```xml
<service
    android:name="com.clipsync.accessibility.ClipAccessibilityService"
    android:exported="true"
    android:permission="android.permission.BIND_ACCESSIBILITY_SERVICE">
    <intent-filter>
        <action android:name="android.accessibilityservice.AccessibilityService" />
    </intent-filter>
    <meta-data
        android:name="android.accessibilityservice"
        android:resource="@xml/accessibility_service_config" />
</service>
```

**Nota:** `exported="true"` es obligatorio para que el sistema pueda
bindear el servicio. El `android:permission` impide que otras apps lo
bindeen — solo el sistema tiene `BIND_ACCESSIBILITY_SERVICE`.

### 5. `android/.../storage/Prefs.kt`

Añadir una preferencia para activar/desactivar el auto-send:

```kotlin
var autoSendEnabled: Boolean
    get() = prefs.getBoolean(K_AUTO_SEND, true)
    set(v) { prefs.edit().putBoolean(K_AUTO_SEND, v).apply() }

// En companion:
private const val K_AUTO_SEND = "auto_send_enabled"
```

**Por qué un toggle separado de `syncEnabled`:**
- `syncEnabled` controla la recepción (WebSocket) + envío.
- `autoSendEnabled` controla solo el envío automático por accesibilidad.
- El usuario puede querer recibir clips del Mac pero enviar solo
  manualmente (vía FAB).

### 6. `android/.../ui/SettingsScreen.kt`

Añadir un toggle "Auto-envío" en la pantalla de settings, debajo del toggle
de sync. Comportamiento:

- **Toggle ON**: el AccessibilityService envía automáticamente al copiar.
- **Toggle OFF**: solo envío manual vía FAB.
- Si el AccessibilityService no está habilitado en Ajustes del sistema,
  mostrar un banner con botón que abre:
  `Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)`

Detección de si el servicio está activo:

```kotlin
fun isAccessibilityServiceEnabled(context: Context): Boolean {
    val service = "${context.packageName}/.accessibility.ClipAccessibilityService"
    val enabledServices = Settings.Secure.getString(
        context.contentResolver,
        Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES
    ) ?: return false
    return enabledServices.contains(service)
}
```

### 7. `android/.../accessibility/ClipAccessibilityService.kt` — Respetar `autoSendEnabled`

En `handleClipChange()`, añadir al inicio:

```kotlin
if (!prefs.autoSendEnabled) return
```

---

## Prevención de eco — Análisis detallado

El problema: Mac envía clip → Android lo escribe en clipboard →
el listener lo detecta → lo envía de vuelta al Mac → bucle infinito.

**Solución primaria: label check**

`ClipboardWriter` ya usa `LABEL = "clipsync"` como etiqueta al crear
`ClipData`. Cuando el listener detecta un cambio, comprueba:

```kotlin
if (clip.description?.label?.toString() == "clipsync") return
```

Esto funciona porque:
- Los clips escritos por `ClipboardWriter` (incoming del Mac) tienen
  label `"clipsync"`.
- Los clips copiados por el usuario en cualquier app tienen el label
  de esa app (Chrome, Notes, etc.) o un label vacío.

**Solución secundaria: debounce temporal** (defensa en profundidad)

Guardar el timestamp + hash del último clip enviado. Si el listener
recibe un clip idéntico en menos de 2 segundos, ignorar:

```kotlin
private var lastSentHash: Int = 0
private var lastSentTime: Long = 0

private fun isDuplicate(text: String): Boolean {
    val hash = text.hashCode()
    val now = System.currentTimeMillis()
    if (hash == lastSentHash && now - lastSentTime < 2000) return true
    lastSentHash = hash
    lastSentTime = now
    return false
}
```

---

## Orden de implementación

```
Paso 1 — Crear accessibility_service_config.xml
Paso 2 — Añadir string de descripción en strings.xml
Paso 3 — Crear ClipAccessibilityService.kt
Paso 4 — Registrar servicio en AndroidManifest.xml
Paso 5 — Añadir autoSendEnabled a Prefs.kt
Paso 6 — Actualizar SettingsScreen con toggle + banner de permisos
Paso 7 — Testing manual en Pixel
```

Pasos 1-4 son independientes de 5-6. El paso 3 es el núcleo.

---

## Testing

### Setup manual (one-time en el Pixel)

1. Instalar la app (`adb install`)
2. Ir a Ajustes → Accesibilidad → Aplicaciones instaladas → ClipSync → Activar
3. Aceptar el diálogo de advertencia

### Verificaciones

| Test | Acción | Resultado esperado |
|------|--------|--------------------|
| Auto-send texto | Copiar texto en Chrome | Llega al Mac en <1s |
| Anti-eco | Mac envía texto a Android | NO se reenvía al Mac |
| Sync pausado | Pausar sync, copiar texto | No se envía |
| Auto-send OFF | Desactivar toggle, copiar | No se envía |
| Auto-send imagen | Copiar imagen en Fotos | Llega al Mac |
| FAB feedback | Copiar texto | FAB muestra verde brevemente |
| Sin pairing | Desemparejar, copiar | No crash, no se envía |
| Duplicado rápido | Copiar lo mismo 2x rápido | Solo 1 envío |

### Verificación de que no espía pantallas

Con la config mínima (`accessibilityEventTypes=""`,
`canRetrieveWindowContent="false"`), el servicio:
- NO recibe eventos de UI
- NO puede leer texto de pantallas
- Solo usa el contexto privilegiado para clipboard

Se puede verificar poniendo un log en `onAccessibilityEvent` — nunca
debería dispararse.

---

## Riesgos y mitigaciones

| Riesgo | Probabilidad | Mitigación |
|--------|-------------|------------|
| OEM mata el servicio | Baja (Pixel stock) | El usuario puede excluir de optimización de batería |
| Label check falla (app pone "clipsync") | Casi nula | Debounce temporal como backup |
| Clipboard contiene datos sensibles | Media | Respetar `IS_SENSITIVE` flag — no enviar si está marcado |
| Servicio de accesibilidad no inicia | Baja | Banner en SettingsScreen detecta estado y guía al usuario |

---

## Mejora futura (fuera de scope)

- **Filtro de apps**: permitir al usuario elegir desde qué apps se
  auto-envía (ej. solo Chrome y Notes, no WhatsApp).
- **Historial de clips**: mantener un log local de los últimos N clips
  enviados.
- **Confirmación visual**: animación sutil en la FAB cada vez que se
  auto-envía (pulso verde).
