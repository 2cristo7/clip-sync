# Master Plan — Shared Clipboard (Mac ↔ Pixel)

> **Proyecto**: App de sincronización de portapapeles (texto + imágenes) entre macOS y Google Pixel, sin nubes, sin cuentas, open source.
> **Redes soportadas**: LAN (Wi-Fi) y Tailscale (VPN de malla).
> **UI**: Mac → icono en menu bar. Android → pantalla de config + Share Targets.

---

## Principios Arquitectónicos (LEER SIEMPRE)

1. **Flujo asimétrico obligatorio** por políticas de Android 10+:
   - **Mac → Pixel**: push automático; Pixel muestra notificación; al tocarla, copia al clipboard del Pixel.
   - **Pixel → Mac**: Pixel NO lee su clipboard en background; solo envía vía Share Target nativo (intent `ACTION_SEND`).
2. **Mac es el servidor**. Android es el cliente.
3. **Bind siempre a `0.0.0.0`** en el Mac para que `en0` (LAN) y `utun*` (Tailscale) estén ambos alcanzables.
4. **Descubrimiento**: mDNS/Bonjour en LAN (`_clipsync._tcp`); para Tailscale el usuario mete la IP `100.x.x.x` manualmente.
5. **Seguridad**: pairing con secreto compartido, HMAC en cada payload, TLS con cert self-signed pinneado por el cliente.
6. **Cero dependencias innecesarias**. Preferir frameworks nativos (`Network.framework`, `NsdManager`) salvo para WebSocket donde es razonable usar librerías maduras (`Hummingbird` en Mac, `OkHttp` en Android).

---

## Wire Protocol (shared reference)

- **Transporte**: HTTPS + WebSocket sobre el mismo puerto (por defecto `7010`).
- **Endpoints HTTP**:
  - `GET /health` → `{"ok":true,"version":"x.y.z"}`
  - `POST /inject` (Pixel → Mac) → body: `{"type":"text|image","mime":"...","data":"<base64>","ts":..., "nonce":"...", "hmac":"..."}`
  - `GET /pair?code=XXXX` → devuelve token si el código está activo.
- **WebSocket** en `/ws` (Mac → Pixel): frames JSON con el mismo esquema que `/inject`.
- **Auth**: header `Authorization: Bearer <token>` + HMAC-SHA256 del body con el `pairing-secret`.
- **Tamaño máximo de imagen**: 20 MB (configurable).

---

## Fase 0 — Bootstrapping del Monorepo

**Objetivo**: Dejar la estructura base del repo con sub-proyectos y `.gitignore` correcto. Ningún código funcional todavía.

**Archivos a crear/modificar**:
- `.gitignore` (macOS, Xcode, Android Studio, JetBrains, VSCode)
- `README.md` (descripción breve del proyecto)
- `docs/protocol.md` (copiar la sección "Wire Protocol" de este plan)
- `docs/threat-model.md` (enumerar amenazas: MITM LAN, Tailscale exit-node, dispositivo comprometido, replay)
- `mac/` (carpeta vacía, con `.gitkeep`)
- `android/` (carpeta vacía, con `.gitkeep`)

**Comandos Git esperados**:
- Rama: `chore/bootstrap`
- Commits:
  - `chore[repo]: add gitignore for xcode and android studio`
  - `docs[protocol]: define clipboard sync wire protocol`
  - `docs[security]: add initial threat model`

**Prompt para esta fase**:
```
Contexto: trabajamos en /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee claude.md y master_plan.md primero. Estás ejecutando la Fase 0 (Bootstrapping del Monorepo).

Tareas:
1. Crea una rama chore/bootstrap.
2. Crea .gitignore cubriendo: macOS (.DS_Store), Xcode (DerivedData, *.xcuserdatad, xcuserstate, build/), Android Studio (*.iml, .gradle, /build, /.idea/*, /captures, local.properties, *.apk, *.keystore), JetBrains (.idea), Node (node_modules), VSCode (.vscode).
3. Crea docs/protocol.md con la sección "Wire Protocol" exactamente como está en master_plan.md.
4. Crea docs/threat-model.md enumerando: MITM en LAN, exit-node malicioso de Tailscale, dispositivo comprometido con el pairing-secret, ataques de replay, payload oversize (DoS). Para cada uno escribe mitigación concreta.
5. Crea mac/.gitkeep y android/.gitkeep.
6. Crea README.md con una descripción breve (≤150 palabras) del proyecto.
7. Haz commits atómicos según el plan.
8. Si todo está bien, mergea chore/bootstrap a main con --no-ff y borra la rama local.

Criterios de validación antes del merge:
- git log --oneline muestra al menos 3 commits en la rama.
- ls mac android docs retorna las carpetas con contenido esperado.
- cat .gitignore | grep -c "DerivedData" >= 1
```

**Criterios de Éxito**:
- `git log --all --oneline` muestra los commits esperados.
- `tree -L 2` muestra `mac/`, `android/`, `docs/` en raíz.
- `git check-ignore -v build/` retorna coincidencia con `.gitignore`.

---

## Fase 1 — Servidor Core macOS (Swift, Hummingbird)

**Objetivo**: App macOS (menu-bar) que levanta un servidor HTTP/WebSocket en `0.0.0.0:7010` con endpoint `/health`. Sin funcionalidad de clipboard aún.

**Archivos a crear/modificar**:
- `mac/ClipSync.xcodeproj/` (proyecto AppKit, `LSUIElement=YES`, SwiftUI + AppDelegate)
- `mac/ClipSync/App.swift` — `NSApplicationDelegate` con `NSStatusItem`
- `mac/ClipSync/Server/ClipServer.swift` — wrapper sobre Hummingbird con `/health` y logging
- `mac/ClipSync/Server/ServerConfig.swift` — host, port, logLevel
- `mac/ClipSync/Info.plist` — `LSUIElement`, `NSLocalNetworkUsageDescription`, `NSBonjourServices`
- `mac/Package.swift` o `Podfile`/SPM en Xcode añadiendo `hummingbird` ≥ 2.x
- `mac/README.md` con instrucciones de build

**Comandos Git esperados**:
- Rama: `feature/mac-server-core`
- Commits:
  - `chore[mac]: bootstrap xcode menubar app`
  - `feat[mac-server]: embed hummingbird http server on 0.0.0.0:7010`
  - `feat[mac-server]: add /health endpoint with version metadata`

**Prompt para esta fase**:
```
Contexto: proyecto en /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee claude.md y master_plan.md (especialmente Fase 1). Estás ejecutando la Fase 1.

Stack obligatorio:
- Swift 5.9+, macOS 13+.
- Xcode project (NO solo SwiftPM) porque necesitamos bundle .app con LSUIElement=YES.
- Hummingbird 2.x como servidor HTTP/WebSocket.
- AppKit + SwiftUI híbrido, NSApplicationDelegate.

Tareas:
1. Crea rama feature/mac-server-core.
2. Genera mac/ClipSync.xcodeproj con target "ClipSync" (macOS app, SwiftUI lifecycle + AppDelegate adapter).
3. Configura Info.plist con LSUIElement=YES, NSLocalNetworkUsageDescription="Sync clipboard with paired devices", NSBonjourServices=["_clipsync._tcp"].
4. Añade Hummingbird como SPM dependency.
5. Implementa ClipServer con un endpoint GET /health que retorne {"ok":true,"version":"0.1.0","platform":"macos"}. Bind a 0.0.0.0:7010.
6. En AppDelegate: NSStatusItem con icono SF Symbol "doc.on.clipboard" y menú con "Quit". Arranca ClipServer al lanzar la app.
7. Maneja el error de puerto ocupado con un log claro.
8. NO implementes NSPasteboard, NSPasteboard es Fase 2. NO implementes mDNS, es Fase 3.
9. Haz commits atómicos siguiendo Conventional Commits.
10. Valida con los criterios de éxito abajo. Si pasan, mergea a main con --no-ff y borra la rama local.

Criterios de Éxito (ejecutables en terminal):
- xcodebuild -project mac/ClipSync.xcodeproj -scheme ClipSync -configuration Debug build → exit 0.
- open mac/build/Debug/ClipSync.app (o desde Xcode), luego:
- curl -s http://127.0.0.1:7010/health | jq .ok → "true"
- curl -s http://$(ipconfig getifaddr en0):7010/health | jq .ok → "true" (valida bind 0.0.0.0)
- lsof -iTCP:7010 -sTCP:LISTEN muestra el proceso ClipSync.
```

**Criterios de Éxito**:
- Build limpio con `xcodebuild`.
- `curl http://127.0.0.1:7010/health` devuelve `ok:true`.
- `curl http://<ip-en0>:7010/health` también funciona (bind a `0.0.0.0` verificado).
- Icono visible en la barra de menú, menú "Quit" cierra y libera el puerto.

---

## Fase 2 — Observador de NSPasteboard + Endpoint /inject

**Objetivo**: Detectar cambios en `NSPasteboard` (texto e imágenes), emitirlos por WebSocket a clientes conectados; recibir contenido por `POST /inject` e inyectarlo en el `NSPasteboard`.

**Archivos a crear/modificar**:
- `mac/ClipSync/Clipboard/PasteboardWatcher.swift` — polling de `changeCount` con `Timer`/`DispatchSourceTimer`
- `mac/ClipSync/Clipboard/PasteboardInjector.swift` — `NSPasteboard.general.setData(...)`
- `mac/ClipSync/Clipboard/ClipPayload.swift` — modelo Codable (text, image, mime, base64, ts, nonce)
- `mac/ClipSync/Server/WebSocketHub.swift` — gestiona clientes conectados y broadcast
- `mac/ClipSync/Server/ClipServer.swift` — añade `POST /inject` y `WebSocket /ws`
- `mac/ClipSyncTests/PasteboardRoundtripTests.swift` — XCTest

**Comandos Git esperados**:
- Rama: `feature/mac-clipboard-core`
- Commits:
  - `feat[mac-clipboard]: add NSPasteboard watcher with changeCount polling`
  - `feat[mac-server]: broadcast clipboard changes via websocket`
  - `feat[mac-server]: accept POST /inject to write into NSPasteboard`
  - `test[mac-clipboard]: roundtrip text and image through pasteboard`

**Prompt para esta fase**:
```
Contexto: proyecto en /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee claude.md y master_plan.md (especialmente Fase 2). Asume que la Fase 1 ya está mergeada en main.

Objetivo: el Mac observa su clipboard y emite cambios por WebSocket; también acepta POST /inject para escribir en el clipboard.

Tareas:
1. Crea rama feature/mac-clipboard-core.
2. Define ClipPayload (Codable) con campos: type ("text"|"image"), mime, dataBase64, ts (epoch ms), nonce (UUID).
3. Implementa PasteboardWatcher que cada 500 ms compare NSPasteboard.general.changeCount contra el último visto. Cuando cambia, extrae: si hay NSPasteboard.PasteboardType.string → text; si hay NSPasteboard.PasteboardType.png/tiff → image. Publica el ClipPayload por un AsyncStream / Combine publisher.
4. Implementa PasteboardInjector.inject(payload:) que escribe en NSPasteboard.general (clearContents + setData/setString).
5. Implementa WebSocketHub que mantiene un Set<WebSocketClient>, broadcast(payload).
6. Conecta PasteboardWatcher → WebSocketHub.
7. Añade ruta POST /inject en ClipServer que decodifica JSON → PasteboardInjector.inject(...). Auth queda para Fase 4, pero añade un header X-ClipSync-Source para debugging.
8. Añade WebSocket /ws handshake con Hummingbird. Envía ping cada 30s.
9. PROTECCIÓN ANTI-LOOP: cuando /inject escribe en el pasteboard, PasteboardWatcher verá el cambio y lo re-emitirá. Solución: guarda el hash del último payload recibido por /inject y suprime el siguiente tick del watcher si coincide.
10. Tests XCTest: roundtrip de texto y PNG.
11. Commits atómicos. Si todos los tests + criterios pasan, mergea a main con --no-ff.

Criterios de Éxito:
- xcodebuild test -project mac/ClipSync.xcodeproj -scheme ClipSync → todos los tests pasan.
- Abrir la app. En terminal: websocat ws://127.0.0.1:7010/ws. Copiar "hola mundo" en otra app → websocat imprime el JSON con "hola mundo" en base64.
- curl -X POST http://127.0.0.1:7010/inject -H "Content-Type: application/json" -d '{"type":"text","mime":"text/plain","dataBase64":"aG9sYQ==","ts":1,"nonce":"abc"}' → pbpaste imprime "hola".
- Copiar una imagen (screenshot cmd+ctrl+shift+4) → frame WebSocket con type=image.
- Loop check: ejecutar /inject 3 veces seguidas no produce más de 1 broadcast por llamada.
```

**Criterios de Éxito**:
- Tests XCTest en verde.
- Roundtrip texto vía `websocat` y `pbpaste`.
- Roundtrip imagen.
- No hay loop infinito entre watcher y `/inject`.

---

## Fase 3 — mDNS/Bonjour + Menu Bar UI + Pairing

**Objetivo**: Anunciar el servicio por Bonjour (`_clipsync._tcp`), añadir menú funcional (estado, pairing con código de 6 dígitos, quit), generar y mostrar el token de pairing.

**Archivos a crear/modificar**:
- `mac/ClipSync/Network/BonjourAdvertiser.swift` — `NetService` con TXT record (version, fingerprint)
- `mac/ClipSync/Pairing/PairingManager.swift` — genera código de 6 dígitos válido 5 min; emite token JWT-like firmado
- `mac/ClipSync/UI/MenuBarController.swift` — `NSStatusItem` con menú dinámico (Conectado/Desconectado, "Start Pairing", clientes conectados, Quit)
- `mac/ClipSync/UI/PairingWindow.swift` — ventana SwiftUI mínima con QR + código
- `mac/ClipSync/Storage/Keychain.swift` — guarda pairing-secret en Keychain

**Comandos Git esperados**:
- Rama: `feature/mac-discovery-pairing`
- Commits:
  - `feat[mac-network]: add mDNS broadcasting for _clipsync._tcp`
  - `feat[mac-ui]: implement dynamic menu bar with connection state`
  - `feat[mac-pairing]: generate pairing code and bootstrap shared secret`

**Prompt para esta fase**:
```
Contexto: proyecto en /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee claude.md y master_plan.md (Fase 3). Asume Fases 0-2 mergeadas en main.

Tareas:
1. Crea rama feature/mac-discovery-pairing.
2. Implementa BonjourAdvertiser con NetService o NWListener + NWParameters.includePeerToPeer, tipo "_clipsync._tcp", puerto 7010. TXT record: version, name (hostname), fp (fingerprint SHA-256 truncado del cert TLS futuro; de momento random estable).
3. PairingManager: genera código 6 dígitos random (cryptographically secure), válido 5 minutos. Al consumirse (GET /pair?code=XXXXXX) retorna un token de 32 bytes base64, firmado por el pairing-secret (HMAC). El pairing-secret se guarda en Keychain como "com.clipsync.pairing-secret" y se genera si no existe.
4. MenuBarController con NSMenu dinámico:
   - Primera línea: estado (🟢 Connected (N) / ⚪️ Idle).
   - "Start Pairing..." → abre PairingWindow.
   - Submenu "Clients": lista IP + última vez visto.
   - Separador. "Quit ClipSync".
5. PairingWindow SwiftUI: muestra el código de 6 dígitos en grande, QR con payload "clipsync://pair?host=<hostname.local>&port=7010&code=XXXXXX". Timer de expiración visual.
6. Conecta WebSocketHub.clientsDidChange → MenuBarController.refresh.
7. Commits atómicos. Si criterios pasan, mergea a main con --no-ff.

Criterios de Éxito:
- dns-sd -B _clipsync._tcp desde otro terminal lista el servicio.
- dns-sd -L <nombre> _clipsync._tcp resuelve puerto 7010 y TXT con version+fp.
- Click "Start Pairing..." abre ventana con código visible y QR.
- curl "http://127.0.0.1:7010/pair?code=<código>" devuelve JSON con token. Segundo curl con mismo código devuelve 401.
- Keychain tiene la entrada (security find-generic-password -s "com.clipsync.pairing-secret" -g → sale el secreto).
```

**Criterios de Éxito**:
- `dns-sd -B _clipsync._tcp` descubre el servicio.
- Pairing flow completo: código → token (1 uso).
- Menú se actualiza cuando un cliente se conecta/desconecta.

---

## Fase 4 — Seguridad (TLS self-signed + HMAC + Auth)

**Objetivo**: Todo el tráfico cifrado (HTTPS + WSS), payloads firmados con HMAC, clientes autenticados con Bearer token emitido en pairing.

**Archivos a crear/modificar**:
- `mac/ClipSync/Security/TLSManager.swift` — genera/lee cert self-signed, persistido en Keychain
- `mac/ClipSync/Security/HMACValidator.swift` — valida firma de payloads entrantes
- `mac/ClipSync/Server/AuthMiddleware.swift` — middleware Hummingbird que valida Bearer + HMAC
- `mac/ClipSync/Pairing/TokenStore.swift` — tokens activos (persistidos)
- `docs/security.md` — modelo de confianza, rotación, revocación

**Comandos Git esperados**:
- Rama: `feature/mac-security`
- Commits:
  - `feat[mac-security]: generate self-signed TLS cert and persist in keychain`
  - `feat[mac-security]: add hmac payload validation middleware`
  - `feat[mac-security]: enforce bearer auth on /inject and /ws`
  - `docs[security]: document trust model and token rotation`

**Prompt para esta fase**:
```
Contexto: proyecto en /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee claude.md, master_plan.md (Fase 4), docs/threat-model.md. Asume Fases 0-3 mergeadas.

Tareas:
1. Crea rama feature/mac-security.
2. TLSManager: en primer arranque, genera un certificado self-signed RSA 2048 (o EC P-256) con SAN "localhost", "<hostname>.local", IP local. Guarda cert+key en Keychain. Expone identidad como SecIdentity para usar con Network.framework/Hummingbird.
3. Migra Hummingbird a HTTPS (puerto 7010 sigue igual, pero TLS). El /health puede quedar en HTTP plano en el puerto 7011 si hace falta para diagnósticos (opcional).
4. Actualiza BonjourAdvertiser: TXT "fp" ahora es SHA-256 real del SubjectPublicKeyInfo del cert, codificado base64url sin padding. El cliente lo usará para cert pinning.
5. HMACValidator: dado body + header "X-ClipSync-Signature: t=<ts>, v1=<hex>", valida que HMAC-SHA256(pairing-secret, "<ts>.<body>") == v1 y que |now - ts| < 60s (anti-replay).
6. AuthMiddleware: exige header Authorization: Bearer <token>. Valida contra TokenStore. Aplica a /inject y /ws; /health y /pair quedan abiertos.
7. TokenStore persistente (Keychain o SQLite cifrado). Cada token incluye: id, createdAt, lastSeenAt, deviceLabel.
8. docs/security.md: describe el modelo de confianza (TOFU sobre pairing), procedimiento de revocación (menú del app → "Clients" → "Revoke").
9. Commits atómicos. Merge con --no-ff si todo pasa.

Criterios de Éxito:
- curl -k https://127.0.0.1:7010/health → 200 ok (health sigue abierto o documenta si se movió).
- curl -k https://127.0.0.1:7010/inject SIN Authorization → 401.
- Sin HMAC válido → 401 aunque el Bearer sea correcto.
- openssl s_client -connect 127.0.0.1:7010 muestra el cert self-signed con el fingerprint que publica Bonjour.
- Replay: mismo body + ts viejos (>60s) → 401.
```

**Criterios de Éxito**:
- HTTPS obligatorio, cert pinneable por fingerprint Bonjour.
- `/inject` rechaza peticiones sin Bearer + HMAC válido.
- Replay attack rechazado.

---

## Fase 5 — Cliente Android Base (Kotlin) + Config + Foreground Service

**Objetivo**: App Android que se conecta por WebSocket al Mac (con manual IP o mDNS), mantiene la conexión viva con foreground service, logs visibles. Sin notificaciones de clipboard aún.

**Archivos a crear/modificar**:
- `android/settings.gradle.kts`, `android/build.gradle.kts`, `android/app/build.gradle.kts` (Kotlin + Compose + Hilt opcional)
- `android/app/src/main/AndroidManifest.xml` — permisos `INTERNET`, `ACCESS_NETWORK_STATE`, `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_DATA_SYNC`, `POST_NOTIFICATIONS` (API 33+)
- `android/app/src/main/java/com/clipsync/app/MainActivity.kt` — Compose nav
- `android/app/src/main/java/com/clipsync/ui/SettingsScreen.kt` — Auto/Manual, pairing code input
- `android/app/src/main/java/com/clipsync/discovery/NsdDiscovery.kt` — `NsdManager` para `_clipsync._tcp`
- `android/app/src/main/java/com/clipsync/net/ClipClient.kt` — OkHttp + OkHttp WebSocket con cert pinning
- `android/app/src/main/java/com/clipsync/service/ClipForegroundService.kt`
- `android/app/src/main/java/com/clipsync/storage/Prefs.kt` — EncryptedSharedPreferences con token + fp

**Comandos Git esperados**:
- Rama: `feature/android-client-core`
- Commits:
  - `chore[android]: bootstrap gradle kotlin compose project`
  - `feat[android-discovery]: add nsdmanager mdns discovery`
  - `feat[android-client]: okhttp websocket client with cert pinning`
  - `feat[android-service]: foreground service to keep websocket alive`
  - `feat[android-ui]: settings screen with auto/manual modes and pairing`

**Prompt para esta fase**:
```
Contexto: proyecto en /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee claude.md, master_plan.md (Fase 5), docs/protocol.md, docs/security.md. Asume Fases 0-4 mergeadas.

Stack:
- Kotlin 2.x, Android Gradle Plugin 8.x, minSdk 26, targetSdk 34.
- Jetpack Compose + Material3.
- OkHttp 4.x (WebSocket + cert pinning).
- NsdManager nativo para mDNS.
- EncryptedSharedPreferences para persistir token y fingerprint.
- Hilt opcional; si lo omites, inyección manual está bien.

Tareas:
1. Crea rama feature/android-client-core.
2. Genera proyecto Gradle en android/. Package com.clipsync.
3. Manifest: INTERNET, ACCESS_NETWORK_STATE, FOREGROUND_SERVICE, FOREGROUND_SERVICE_DATA_SYNC, POST_NOTIFICATIONS.
4. SettingsScreen Compose: Modo = [Auto mDNS, Manual IP]. En Manual pide IP (ej. 100.x.x.x) y puerto. Botón "Pair": abre input de código 6 dígitos, llama GET https://<host>:<port>/pair?code=XXXXXX, guarda token y fingerprint devueltos (el fingerprint se obtiene a la vez desde TXT de mDNS o desde un endpoint /fingerprint nuevo si Manual).
5. NsdDiscovery: registra listener para "_clipsync._tcp". Emite flow<Discovered(host, port, fp)>.
6. ClipClient con OkHttpClient configurado con CertificatePinner dinámico (pin = fingerprint guardado en Prefs). Métodos: connectWebSocket(), sendInject(payload).
7. ClipForegroundService tipo "dataSync": notificación persistente "ClipSync connected". Reintento exponencial hasta 30 s.
8. Handshake WebSocket envía Authorization: Bearer <token> y firma HMAC del upgrade nonce.
9. UI muestra estado: Disconnected / Connecting / Connected / Error(reason).
10. NO implementes ClipboardManager writes ni Share Target: eso es Fase 6 y 7. Solo queremos ver frames entrantes en Logcat.
11. Commits atómicos. Merge con --no-ff si todo pasa.

Criterios de Éxito:
- ./gradlew :app:assembleDebug → BUILD SUCCESSFUL.
- adb install android/app/build/outputs/apk/debug/app-debug.apk → OK.
- En Mac: arrancar app y generar código de pairing. En Pixel: meter código → Settings muestra "Connected" y aparece en el menú bar del Mac como cliente.
- adb logcat | grep ClipSync muestra los frames entrantes cuando copias algo en el Mac.
- Matar la app desde recents → el foreground service sigue y reconecta (verificable en logcat).
- Modo Manual con IP Tailscale 100.x.x.x también conecta (si ambos dispositivos tienen Tailscale).
```

**Criterios de Éxito**:
- Build + install OK.
- Pairing end-to-end funcional.
- Conexión sobrevive a app swipe away.
- Funciona con mDNS en LAN y con IP manual en Tailscale.

---

## Fase 6 — Android: Notificaciones + Inyección en ClipboardManager

**Objetivo**: Cuando el Pixel recibe un frame del Mac, muestra una notificación. Al tocarla, el contenido se copia al `ClipboardManager` del Pixel.

**Archivos a crear/modificar**:
- `android/app/src/main/java/com/clipsync/clipboard/ClipboardWriter.kt`
- `android/app/src/main/java/com/clipsync/notifications/IncomingClipNotifier.kt` — canal `clipsync_incoming`
- `android/app/src/main/java/com/clipsync/notifications/ApplyClipActivity.kt` — trampolín, escribe al clipboard y hace `finish()`
- `android/app/src/main/java/com/clipsync/images/ImageCache.kt` — guarda imagen en `cacheDir`, retorna `Uri` via `FileProvider`
- `android/app/src/main/res/xml/file_paths.xml` + manifest `<provider>`
- `android/app/src/main/AndroidManifest.xml` — `ApplyClipActivity` + `FileProvider`

**Comandos Git esperados**:
- Rama: `feature/android-notifications-clipboard`
- Commits:
  - `feat[android-notifications]: show notification on incoming clip`
  - `feat[android-clipboard]: write text clips via ApplyClipActivity trampoline`
  - `feat[android-clipboard]: support image clips via FileProvider uri`

**Prompt para esta fase**:
```
Contexto: proyecto en /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee claude.md, master_plan.md (Fase 6). Asume Fases 0-5 mergeadas.

Tareas:
1. Crea rama feature/android-notifications-clipboard.
2. Crea canal "clipsync_incoming" (IMPORTANCE_DEFAULT, sound off).
3. IncomingClipNotifier: para cada ClipPayload recibido por WebSocket:
   - Texto: muestra notificación con preview ≤120 chars y PendingIntent a ApplyClipActivity con el texto en extras (encryption opcional, pero está en RAM del propio device).
   - Imagen: guarda bytes decodificados en cacheDir/clipsync/<uuid>.(png|jpg). Notificación con BigPictureStyle. PendingIntent que incluye la Uri via FileProvider.
4. ApplyClipActivity (theme Translucent/NoDisplay):
   - Si hay texto: ClipboardManager.setPrimaryClip(ClipData.newPlainText("clipsync", text)).
   - Si hay Uri de imagen: ClipData.newUri(contentResolver, "clipsync", uri) con grantUriPermission a android.permission.READ. Fallback a guardar en MediaStore si la app destino lo necesita (documentar limitación Android 13+: solo algunas apps respetan imágenes en clipboard).
   - Toast corto "Copied to clipboard", finish().
5. FileProvider con authority "com.clipsync.fileprovider" apuntando a cacheDir/clipsync.
6. Limpia imágenes > 24h en cacheDir al arrancar el service.
7. Commits atómicos. Merge con --no-ff.

Criterios de Éxito:
- Copiar "hello" en el Mac → notif en Pixel → tap → adb shell service call clipboard 2 (o una app de notas) muestra "hello".
- Copiar una imagen en el Mac → notif con preview → tap → pegar en Google Keep / WhatsApp funciona.
- cacheDir no crece sin límite (comprobable tras copiar 5 imágenes y revisar /data/data/com.clipsync/cache/clipsync).
- Si el usuario revoca POST_NOTIFICATIONS, la UI muestra "Notifications disabled" sin crashear.
```

**Criterios de Éxito**:
- Notificación con preview texto/imagen.
- Tap copia al clipboard real del Pixel (verificable en otra app).
- Limpieza de cache funciona.

---

## Fase 7 — Android: Share Target (Texto + Imágenes)

**Objetivo**: La app aparece en el menú nativo "Compartir" de Android para `text/plain` y `image/*`. Al seleccionarla, POST al Mac y se inyecta en `NSPasteboard`.

**Archivos a crear/modificar**:
- `android/app/src/main/java/com/clipsync/share/ShareReceiverActivity.kt` — handle `ACTION_SEND` y `ACTION_SEND_MULTIPLE`
- `android/app/src/main/java/com/clipsync/share/ShareSender.kt` — construye `ClipPayload`, firma HMAC, POST `/inject`
- `android/app/src/main/res/xml/shortcuts.xml` — Direct Share Targets (opcional, Android 10+)
- `android/app/src/main/AndroidManifest.xml` — `<activity>` con `intent-filter` para `ACTION_SEND` y `mimeType` `text/*` + `image/*`

**Comandos Git esperados**:
- Rama: `feature/android-share-target`
- Commits:
  - `feat[android-share]: register share target for text and images`
  - `feat[android-share]: post shared content to mac via /inject`
  - `test[android-share]: integration test with mock server`

**Prompt para esta fase**:
```
Contexto: proyecto en /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee claude.md, master_plan.md (Fase 7). Asume Fases 0-6 mergeadas.

Tareas:
1. Crea rama feature/android-share-target.
2. ShareReceiverActivity con theme NoDisplay. Intent filters:
   - android.intent.action.SEND con mimeType text/plain.
   - android.intent.action.SEND con mimeType image/*.
   - android.intent.action.SEND_MULTIPLE con mimeType image/* (opcional, para cuando el usuario comparte varias).
3. Parse del intent:
   - Texto: EXTRA_TEXT.
   - Imagen: EXTRA_STREAM (Uri). Abre InputStream, verifica tamaño ≤ 20 MB, lee bytes, detecta MIME real con ContentResolver.getType o MimeTypeMap.
4. ShareSender construye ClipPayload (base64), firma HMAC con el pairing-secret guardado. POST a https://<host>:<port>/inject. Usa el mismo OkHttpClient con cert pinning del ClipClient.
5. Feedback al usuario: Toast "Sent to Mac" / "Failed: <reason>". finish().
6. Si no hay conexión (no paired / offline): Toast "Pair ClipSync first" y abre MainActivity.
7. Registro opcional de shortcuts.xml para Direct Share Targets en Android 11+ (no crítico).
8. Tests de integración con MockWebServer: valida que el body incluye HMAC y Bearer correctos.
9. Commits atómicos. Merge con --no-ff si todo pasa.

Criterios de Éxito:
- Abre Chrome en Pixel, selecciona texto → Compartir → aparece "ClipSync" en la hoja de share.
- Tras tap → pbpaste en Mac imprime el texto.
- Abre Galería, selecciona una foto → Compartir → ClipSync → en Mac, Cmd+V en Preview pega la imagen.
- Compartir una imagen de 30 MB → Toast de error "Image too large".
- ./gradlew test → todos los tests verdes.
```

**Criterios de Éxito**:
- App visible en share sheet para texto e imagen.
- `pbpaste` en Mac devuelve el texto compartido.
- Imagen pegable en apps Mac.
- Tests de integración verdes.

---

## Fase 8 — Integración Tailscale + Pruebas Remotas

**Objetivo**: Validar funcionamiento end-to-end sobre Tailscale (redes distintas, sin LAN común). Documentar setup. Manejar edge cases de MagicDNS.

**Archivos a crear/modificar**:
- `docs/tailscale-setup.md` — instrucciones paso a paso (instalar, `tailscale up`, obtener IP 100.x.x.x, MagicDNS)
- `android/app/src/main/java/com/clipsync/net/NetworkChangeObserver.kt` — escucha `ConnectivityManager` y fuerza reconexión
- `mac/ClipSync/Network/ReachabilityMonitor.swift` — `NWPathMonitor`

**Comandos Git esperados**:
- Rama: `feature/tailscale-validation`
- Commits:
  - `docs[tailscale]: add end-to-end setup guide`
  - `feat[mac-net]: reconnect on nwpath change`
  - `feat[android-net]: observe connectivity and trigger reconnect`

**Prompt para esta fase**:
```
Contexto: proyecto en /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee claude.md, master_plan.md (Fase 8). Asume Fases 0-7 mergeadas.

Tareas:
1. Crea rama feature/tailscale-validation.
2. docs/tailscale-setup.md: pasos para Mac (brew install tailscale / app oficial) y Android (Play Store), obtener tailnet, IPs, MagicDNS, autorizar devices. Incluye troubleshooting de firewall macOS (pfctl / System Settings > Network > Firewall).
3. ReachabilityMonitor (Mac) con NWPathMonitor: re-anuncia Bonjour al cambiar de interfaz, loggea interfaces disponibles.
4. NetworkChangeObserver (Android): NetworkCallback que detecta cambios y reabre el WebSocket si el estado era Connected.
5. Ejecuta el protocolo de prueba remota (ver Criterios de Éxito). Documenta resultados en docs/tailscale-setup.md sección "Tested scenarios".
6. Commits atómicos. Merge con --no-ff.

Criterios de Éxito (protocolo remoto, requiere datos móviles en el Pixel):
- Mac en Wi-Fi casa, Pixel en 5G. Ambos en Tailscale.
- En Pixel, Settings → Manual IP → 100.x.x.x del Mac. Pair con código (ya no hay mDNS cross-tailnet, normal).
- Copiar en Mac → notif en Pixel (latencia típica < 2s).
- Share desde Pixel → NSPasteboard del Mac.
- Apagar Wi-Fi del Pixel (pasa a 5G puro) → foreground service reconecta en < 30s.
- Apagar Tailscale temporalmente en el Mac → cliente pasa a "Disconnected", al reactivar vuelve a "Connected".
```

**Criterios de Éxito**:
- E2E funcional sobre Tailscale fuera de LAN común.
- Reconexión tras cambio de red.
- Guía Tailscale reproducible.

---

## Fase 9 — Polish, Release y CI

**Objetivo**: README usable, licencia, scripts de build, firma de app Android, notarización opcional del `.app` Mac, GitHub Actions opcional.

**Archivos a crear/modificar**:
- `README.md` (expandido)
- `LICENSE` (MIT recomendada, decidir con el usuario)
- `mac/scripts/build-release.sh`
- `android/app/proguard-rules.pro`
- `.github/workflows/ci.yml` (opcional) — build mac + android, lint, tests

**Comandos Git esperados**:
- Rama: `release/v0.1.0`
- Commits:
  - `docs[readme]: document installation and usage`
  - `chore[license]: add MIT license`
  - `chore[ci]: add github actions for mac and android builds`
  - Tag: `v0.1.0`

**Prompt para esta fase**:
```
Contexto: proyecto en /Users/2cristo7/Documents/personal-proyects/shared-clipboard.
Lee claude.md, master_plan.md (Fase 9). Asume Fases 0-8 mergeadas.

Tareas:
1. Crea rama release/v0.1.0.
2. README.md expandido: features, screenshots (placeholder), install (Mac y Android), setup pairing, setup Tailscale, limitations (Android 13+ image clipboard, etc.), licencia.
3. LICENSE (confirma MIT con el usuario si duda).
4. mac/scripts/build-release.sh: xcodebuild archive + export. Opcional: codesign + notarytool si hay perfil.
5. android proguard rules básicos para OkHttp y Compose.
6. .github/workflows/ci.yml:
   - Job mac: macos-latest, xcodebuild build + test.
   - Job android: ubuntu-latest, gradle build + test.
7. Crea tag v0.1.0 tras mergear a main.
8. Si todo OK, mergea release/v0.1.0 a main con --no-ff y crea el tag.

Criterios de Éxito:
- bash mac/scripts/build-release.sh produce ClipSync.app firmada (o sin firmar si no hay cert) en mac/build/Release.
- ./gradlew :app:assembleRelease produce app-release-unsigned.apk.
- CI verde en GitHub Actions (si está habilitado).
- git tag --list muestra v0.1.0.
```

**Criterios de Éxito**:
- Build release Mac + Android sin errores.
- README completo.
- Tag `v0.1.0` creado.

---

## Apéndice A — Decisiones Técnicas Clave

| Área | Decisión | Alternativa descartada | Motivo |
|---|---|---|---|
| Server Mac | Hummingbird 2.x | Vapor, Swifter, raw Network.framework | Ligero, WebSocket nativo, Swift Concurrency |
| WebSocket Android | OkHttp | Ktor, Scarlet | Estándar, cert pinning trivial |
| Discovery LAN | mDNS `_clipsync._tcp` | Broadcast UDP custom | Estándar, soportado nativo ambos lados |
| Discovery remoto | IP Manual (Tailscale 100.x.x.x) | Servidor de rendezvous | Filosofía zero-cloud |
| Auth | Pairing code → token Bearer + HMAC por mensaje | OAuth, mTLS | Simple, sin PKI |
| TLS | Self-signed + fingerprint pinning (TOFU vía Bonjour TXT) | Let's Encrypt, mkcert | No dominio público, zero dependencias externas |
| Android background | Foreground Service `dataSync` | WorkManager, JobScheduler | Persistencia real de WebSocket |
| Clipboard Android inbound | Notification → Trampoline Activity | Accessibility Service | Privacy-friendly, sin permisos invasivos |
| Clipboard Android outbound | Share Target (`ACTION_SEND`) | Listener en background | Impuesto por Android 10+ |

## Apéndice B — Checklist de seguridad

- [ ] TLS obligatorio en producción.
- [ ] HMAC con timestamp ±60s en cada payload.
- [ ] Token revocable desde el menú bar.
- [ ] Pairing-secret rotable regenerando en el Mac (invalida todos los devices).
- [ ] Tamaño máximo de payload configurable (default 20 MB).
- [ ] Logs no filtran contenido del clipboard (solo longitud + tipo).
- [ ] Keychain para secretos en Mac; EncryptedSharedPreferences en Android.
- [ ] Imágenes se borran del cache tras 24h.

## Apéndice C — Flujo de trabajo por fase

1. **Abrir chat limpio** (contexto cero) y pegar el "Prompt para esta fase" de la fase correspondiente.
2. Claude crea la rama, implementa, commitea, valida criterios de éxito, mergea a `main` con `--no-ff` y borra la rama local.
3. Antes de pasar a la siguiente fase: `git log --oneline --graph` para revisar integridad; si algo falla, volver atrás en la misma rama antes de mergear.
4. Cada fase es atómica: si el merge ya ocurrió y se descubre un bug después, se abre una rama `fix/...` nueva.
