# Plan: Auto Clipboard Detection (Android → Mac)

**Objetivo:** Cuando el usuario copia algo en Android, enviar automáticamente
al Mac sin necesidad de pulsar el FAB. El FAB se mantiene como fallback visual.

**Dispositivo objetivo:** Pixel 9a (Android 14/15, API 34+).

---

## Análisis del problema

### Restricciones de Android 10+ (API 29+)

Android restringe `ClipboardManager.getPrimaryClip()` a apps con **ventana
enfocada** (o IME por defecto). Esto significa:

| Contexto                         | `OnPrimaryClipChangedListener` dispara? | `getPrimaryClip()` devuelve datos? |
|----------------------------------|:---------------------------------------:|:----------------------------------:|
| Activity con foco                | Sí                                      | Sí                                 |
| Foreground Service (sin ventana) | Sí                                      | **No** (null)                      |
| Overlay (TYPE_APPLICATION_OVERLAY)| Sí                                     | **No** (null, sin input focus)     |
| AccessibilityService             | Sí                                      | **Sí** (privilegio especial)       |

### Lo que ya tenemos

- **`SendClipActivity`** — trampoline transparente que obtiene foco de ventana
  y lee el clipboard. Funciona en Pixel/Android 12-15.
- **`ClipboardWriter.lastMacWriteMs`** — timestamp de la última escritura
  nuestra al clipboard. Supresión de eco lista para usar.
- **`Prefs.autoSendEnabled`** — toggle ya definido (default `true`).
- **`ClipAccessibilityService`** — placeholder registrado en manifest con
  config XML. No hace nada actualmente.
- **`ClipOverlayManager`** — vive en `ClipForegroundService`, muestra FAB
  cuando hay conexión WS.

---

## Estrategia: Enfoque híbrido en 2 capas

### Capa 1 — Listener en el Foreground Service (funciona siempre que el servicio corre)

`OnPrimaryClipChangedListener` registrado en `ClipForegroundService`. Cuando
se dispara:

1. Comprueba `autoSendEnabled` y `syncEnabled` en Prefs.
2. Comprueba supresión de eco: si `System.currentTimeMillis() - ClipboardWriter.lastMacWriteMs < 2000`, ignora (es contenido que escribimos nosotros desde el Mac).
3. Aplica debounce: guarda timestamp del último auto-send, ignora si < 1s
   (evita duplicados por apps que escriben clipboard múltiples veces).
4. Lanza `SendClipActivity` con un extra `EXTRA_AUTO_SEND = true`.
5. `SendClipActivity` obtiene foco → lee clipboard → envía → finish.

**¿Por qué funciona?** El listener dispara sin foco, pero no necesitamos leer
el clipboard en el listener — solo necesitamos saber que *algo cambió*. La
lectura real la hace `SendClipActivity` que sí obtiene foco.

**Limitación:** En Android 14+, si la app está en estado "restricted" por
el usuario (Settings → Battery → Restricted), el foreground service puede
ser matado. Pero si el usuario tiene ClipSync corriendo con la notificación
persistente, esto no aplica.

### Capa 2 — AccessibilityService (futuro, opcional)

Para cubrir edge cases donde el foreground service se pierde (ej. battery
optimization agresiva en algunos OEMs), el `ClipAccessibilityService` puede
servir como respaldo. **No es necesario para Pixel 9a con stock Android 14/15**
— el foreground service es suficiente.

Se deja como mejora futura porque:
- Requiere que el usuario active manualmente en Settings → Accessibility.
- Google Play tiene políticas restrictivas para apps que usan AccessibilityService.
- En Pixel 9a stock, el foreground service no se mata.

---

## Archivos a modificar

### 1. `ClipForegroundService.kt` — Registrar clipboard listener

**Cambios:**
- Añadir campo `clipboardManager: ClipboardManager`.
- Añadir campo `clipChangedListener: OnPrimaryClipChangedListener`.
- Añadir campo `lastAutoSendMs: Long` para debounce.
- En `onCreate()`: obtener `ClipboardManager`, crear listener.
- En `connect()` → `WsStatus.Open`: registrar listener (solo si `autoSendEnabled`).
- En `WsStatus.Closed/Error`: desregistrar listener.
- En `onDestroy()`: desregistrar listener.
- Nuevo método `onClipboardChanged()` con la lógica de filtrado.

```kotlin
// Pseudocódigo del listener
private var lastAutoSendMs = 0L

private val clipChangedListener = OnPrimaryClipChangedListener {
    if (!prefs.autoSendEnabled || !prefs.syncEnabled) return@OnPrimaryClipChangedListener
    if (prefs.overlayPaused) return@OnPrimaryClipChangedListener // si sync está pausado

    val now = System.currentTimeMillis()

    // Echo suppression: ignorar si nosotros escribimos al clipboard hace < 2s
    if (now - ClipboardWriter.lastMacWriteMs < 2_000) return@OnPrimaryClipChangedListener

    // Debounce: ignorar si ya enviamos hace < 1s
    if (now - lastAutoSendMs < 1_000) return@OnPrimaryClipChangedListener

    lastAutoSendMs = now
    launchAutoSend()
}

private fun launchAutoSend() {
    val intent = SendClipActivity.intent(this).apply {
        putExtra(SendClipActivity.EXTRA_AUTO_SEND, true)
    }
    startActivity(intent)
}
```

### 2. `SendClipActivity.kt` — Soportar modo auto-send

**Cambios:**
- Nuevo companion const `EXTRA_AUTO_SEND = "auto_send"`.
- Leer el extra en `onCreate()`.
- Si `autoSend = true`:
  - No mostrar toast de "Sent to Mac" (sería invasivo cada vez que copias).
  - Sí mantener toast de error (el usuario necesita saber si falla).
  - El broadcast de resultado sigue igual (para que el FAB muestre feedback
    si está visible).

```kotlin
private var isAutoSend = false

override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    isAutoSend = intent.getBooleanExtra(EXTRA_AUTO_SEND, false)
    // ... resto igual
}

private fun handleResult(result: ClipSender.Result) {
    when (result) {
        is ClipSender.Result.Ok -> {
            if (!isAutoSend) toast("Sent to Mac")
            sendBroadcast(...)
        }
        is ClipSender.Result.Failed -> {
            // Siempre mostrar error
            toast("Failed: ${result.reason}")
            sendBroadcast(...)
        }
    }
    finish()
}
```

### 3. `ClipOverlayManager.kt` — Feedback visual en auto-send

**Cambios mínimos:**
- El FAB ya escucha `ACTION_SEND_RESULT` broadcasts.
- Auto-send también emite ese broadcast → el FAB muestra verde/rojo
  automáticamente. **No hay cambio necesario aquí.**

### 4. `SettingsScreen.kt` — Toggle de auto-send

**Cambios:**
- Añadir un toggle "Auto-send clipboard" en la sección de settings.
- Conectado a `Prefs.autoSendEnabled`.
- Descripción: "Automatically send copied content to Mac".

### 5. `SettingsViewModel.kt` — Exponer autoSend state

**Cambios:**
- Añadir `autoSendEnabled: Boolean` al `SettingsState`.
- Función `toggleAutoSend()`.

### 6. `accessibility_service_config.xml` — Sin cambios por ahora

Se deja como está. La Capa 2 (AccessibilityService) es mejora futura.

---

## Flujo completo (post-implementación)

```
Usuario copia texto/imagen en cualquier app
        ↓
Android dispara OnPrimaryClipChangedListener
        ↓
ClipForegroundService.onClipboardChanged()
        ↓
¿autoSendEnabled && syncEnabled?  ─── No ──→ Ignorar
        ↓ Sí
¿Es eco nuestro? (lastMacWriteMs < 2s)  ─── Sí ──→ Ignorar
        ↓ No
¿Debounce? (lastAutoSendMs < 1s)  ─── Sí ──→ Ignorar
        ↓ No
Lanza SendClipActivity(EXTRA_AUTO_SEND=true)
        ↓
Activity obtiene foco → getPrimaryClip() → OK
        ↓
ClipSender.send() → POST /inject (HMAC + Bearer)
        ↓
Broadcast ACTION_SEND_RESULT
        ↓
FAB muestra verde (si visible) | Sin toast (modo auto)
        ↓
Activity.finish() — invisible para el usuario
```

---

## Edge cases y mitigaciones

| Edge case | Mitigación |
|-----------|------------|
| Usuario copia mientras no hay conexión WS | Listener no se registra hasta `WsStatus.Open` → no se intenta enviar |
| App escribe clipboard múltiples veces seguidas | Debounce de 1s ignora ráfagas |
| Contenido recibido del Mac genera eco | `lastMacWriteMs` con ventana de 2s suprime el reenvío |
| SendClipActivity no obtiene foco (raro en Pixel) | Fallback `postDelayed(200ms)` ya implementado |
| Clipboard vacío o tipo no soportado | `SendClipActivity` ya maneja: toast "Nothing to send" y finish |
| Battery optimization mata el foreground service | En Pixel 9a stock no ocurre con foreground service + notificación. Para OEMs agresivos, documentar que el usuario debe excluir la app de optimización |
| Usuario pausa sync desde FAB/settings | Check `syncEnabled` en el listener, no envía |
| Imagen > 20MB copiada | `ClipPayloadBuilder.MAX_IMAGE_BYTES` ya la rechaza |

---

## UX en Pixel 9a

- **Invisible por defecto**: el usuario copia normalmente, el contenido aparece
  en el Mac sin acción adicional. Sin toasts en éxito.
- **FAB como indicador**: si el FAB está visible y se hace auto-send, el icono
  parpadea verde brevemente como confirmación visual sutil.
- **Errores visibles**: si falla, toast con motivo (el usuario necesita saber).
- **Controlable**: toggle en Settings para desactivar auto-send sin desactivar
  la recepción desde Mac.
- **Android 14 clipboard toast**: Android 14 muestra un toast del sistema
  "Copied to clipboard" cuando una app lee el clipboard. `SendClipActivity`
  lo disparará cada vez. **Mitigación**: no hay forma de evitarlo en Android 14+
  (es una protección del sistema). El usuario lo verá pero es consistente
  con el comportamiento nativo. En Android 15+ esto podría cambiar.

---

## Orden de implementación

1. **`ClipForegroundService.kt`** — listener + lógica de filtrado
2. **`SendClipActivity.kt`** — modo auto-send sin toast
3. **`SettingsViewModel.kt`** — estado autoSend
4. **`SettingsScreen.kt`** — toggle UI
5. **Test manual en Pixel 9a**: copiar texto → verificar que llega al Mac

---

## Fuera de alcance (mejoras futuras)

- **Capa 2 AccessibilityService**: para OEMs que matan foreground services.
- **Filtrado inteligente**: ignorar contenido sensible (passwords de managers).
- **Historial de clips**: mantener log local de lo enviado.
- **Notificación inline**: mostrar preview del contenido enviado en la
  notificación persistente.
- **Rate limiting servidor**: el Mac podría recibir muchos `/inject` si el
  usuario copia rápidamente.
