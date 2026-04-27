# ClipSync — Análisis Técnico Profundo

> Generado por **Gemini Deep Research** a partir del código fuente completo de v0.1.0 (abril 2026).  
> El prompt pedía explicar cada capa del sistema de arriba abajo, comparar cada decisión de diseño
> con las alternativas descartadas, y evaluar los trade-offs con honestidad.  
> Escrito en español. Dirigido a un lector con base técnica sólida.

---

## Índice

1. [Visión general](#1-visión-general)
2. [Descubrimiento de dispositivos](#2-descubrimiento-de-dispositivos)
3. [Emparejamiento TOFU](#3-emparejamiento-tofu)
4. [TLS con certificado autofirmado](#4-tls-con-certificado-autofirmado)
5. [HMAC-SHA256](#5-hmac-sha256)
6. [Bearer Token](#6-bearer-token)
7. [Almacenamiento de secretos](#7-almacenamiento-de-secretos)
8. [El problema del portapapeles en Android](#8-el-problema-del-portapapeles-en-android)
9. [Flujos de datos completos](#9-flujos-de-datos-completos)
10. [Threat model](#10-threat-model)
11. [Deuda técnica y decisiones de diseño](#11-deuda-técnica-y-decisiones-de-diseño)

---

## 1. Visión general

### ¿Qué hace ClipSync?

ClipSync sincroniza el portapapeles entre un Mac y un Android en tiempo real. Copias
texto o una imagen en el Mac → aparece en Android (y viceversa). Sin nube, sin cuenta,
sin servidor intermedio.

### Arquitectura: el Mac es el servidor, Android es el cliente

Esta inversión es contraintuitiva pero deliberada. El Mac tiene IP fija en la LAN
doméstica (o IP fija de Tailscale), está siempre encendido y tiene un demonio
(`ClipForegroundService` en Android sería el extremo que se conecta). El teléfono
va y viene de redes, así que es el cliente.

```
┌─────────────────────────────────────────────────────┐
│                      LAN / Tailscale                │
│                                                     │
│  ┌──────────────────────┐    ┌───────────────────┐  │
│  │   Mac  (servidor)    │    │ Android (cliente) │  │
│  │                      │    │                   │  │
│  │ ClipServer           │◄──►│ ClipClient        │  │
│  │  :7010 (HTTPS/WS)    │    │ (OkHttp)          │  │
│  │                      │    │                   │  │
│  │ PasteboardWatcher    │    │ ClipForeground    │  │
│  │  (poll 500ms)        │    │  Service          │  │
│  │                      │    │                   │  │
│  │ BonjourAdvertiser    │    │ NsdDiscovery      │  │
│  │  mDNS _clipsync._tcp │    │  (mDNS listener) │  │
│  └──────────────────────┘    └───────────────────┘  │
└─────────────────────────────────────────────────────┘

Flujos principales:
  Mac → Android : WebSocket /ws (push)
  Android → Mac : POST /inject (pull)
  Descubrimiento: mDNS (LAN) o IP manual (Tailscale)
  Emparejamiento: GET /pair?code=XXXXXX (una sola vez)
```

### Por qué no hay nube

**Ventajas reales:**
- Privacidad total: el contenido del portapapeles nunca sale de tu red.
- Latencia mínima: LAN es órdenes de magnitud más rápida que un servidor remoto.
- Sin dependencia de terceros: funciona aunque el proveedor de la app desaparezca.
- Sin cuenta: no hay superficie de ataque en un servidor compartido.

**Inconvenientes reales:**
- Mac y Android deben estar en la misma red (o en un VPN como Tailscale).
- El usuario debe configurar el emparejamiento manualmente.
- No hay backup de historial del portapapeles.
- Si el Mac está apagado, Android no puede enviar nada.

---

## 2. Descubrimiento de dispositivos

### mDNS / Bonjour: el pregonero de la red local

Imagina que llegas a una ciudad nueva y en la plaza mayor hay un pregonero que grita
"¡Aquí está el panadero! ¡Aquí está el herrero!". mDNS (Multicast DNS) hace lo mismo
en tu red local: los dispositivos "gritan" su presencia sin necesitar un servidor central
de nombres.

En términos técnicos: mDNS usa paquetes UDP multicast a `224.0.0.251:5353`. Cualquier
dispositivo en la misma LAN puede escuchar. macOS lo implementa mediante el protocolo
Bonjour (antes llamado Rendezvous).

ClipSync registra el tipo de servicio `_clipsync._tcp` en mDNS:

```swift
// BonjourAdvertiser.swift
let svc = NetService(
    domain: "",
    type: "_clipsync._tcp",
    name: serviceName,
    port: port
)
svc.setTXTRecord(NetService.data(fromTXTRecord: dict))
svc.publish()
```

El TXT record contiene el campo `fp` — el fingerprint SPKI del certificado TLS:

```
_clipsync._tcp TXT record:
  fp = <base64url de SHA-256 del SubjectPublicKeyInfo>
  v  = <versión del protocolo>
```

**¿Por qué incluir el fingerprint en el TXT record?**

Es ingenioso: el cliente Android descubre al Mac por mDNS y en ese mismo paquete
ya recibe la "foto del DNI" del certificado TLS. Cuando más tarde se conecta por
HTTPS, puede verificar que el servidor es realmente ese Mac y no un impostor.
Sin este mecanismo, el cliente tendría que confiar ciegamente en el primer
certificado que le presente quien sea que responda en esa IP.

### Por qué mDNS falla sobre Tailscale

mDNS depende de multicast. Tailscale crea un túnel WireGuard entre dispositivos:
los paquetes multicast **no se propagan** por ese túnel. Es como intentar gritar
a través de un tubo: el sonido no llega.

**Solución implementada:** cuando el usuario configura Tailscale, introduce la IP
manualmente (del rango `100.64.0.0/10` asignado por Tailscale). En ese caso se
usa el modo TOFU: el cliente se conecta, acepta cualquier certificado en el primer
handshake, captura su fingerprint, y lo pina para siempre.

```kotlin
// SettingsViewModel.kt
is PairingTarget.Manual -> {
    val resp = withContext(Dispatchers.IO) {
        api.pairWithTofu(target.host, target.port, code)
    }
    persistAndStart(context, prefs, target.host, target.port, resp.token, resp.fpBase64Url, resp.secret, Prefs.MODE_MANUAL)
}
```

### ¿Qué es Tailscale?

Tailscale es una VPN mesh basada en WireGuard. En lugar de un servidor central
(modelo hub-and-spoke tradicional), cada dispositivo en tu "tailnet" puede hablar
directamente con los demás usando un túnel cifrado punto a punto. Tailscale gestiona
el intercambio de claves usando su servidor de coordinación (basado en DERP), pero
el tráfico real viaja directo entre dispositivos siempre que sea posible.

Para ClipSync es la solución más limpia cuando Mac y Android no están en la misma
LAN: el Mac tiene IP `100.x.x.x` en el tailnet, Android también, y pueden hablar
como si estuvieran en la misma red local — con la diferencia de que el tráfico
va cifrado por WireGuard.

---

## 3. Emparejamiento TOFU

### ¿Qué es TOFU?

TOFU = Trust On First Use (confiar en el primer uso).

En el sistema de CA (Certificate Authority) tradicional, hay una lista preinstalada
de autoridades de confianza (Verisign, Let's Encrypt, etc.) que avalan los certificados.
Si el certificado está firmado por una CA conocida, confías en él.

TOFU es distinto: no hay árbol de confianza previo. La primera vez que ves a alguien,
memorizas su "cara" (fingerprint). En conexiones futuras, rechazas cualquier "cara"
diferente. Es como guardar la foto de alguien la primera vez que os conocéis: si
al día siguiente aparece alguien diferente diciendo ser esa persona, lo detectas.

**Ventaja sobre CA para este caso:** No necesitas comprar un certificado ni usar
Let's Encrypt (que requiere que el servidor sea accesible desde Internet). El Mac
genera su propio certificado y ese es suficiente.

**Desventaja:** Si al hacer TOFU el atacante ya está haciendo MITM, habrás pinado
la clave del atacante. Por eso el canal de distribución del código de 6 dígitos
importa tanto (ver más abajo).

### Diagrama de secuencia del emparejamiento

```
  Usuario              Android              Mac
    │                     │                  │
    │  "Start Pairing"     │                  │
    │─────────────────────►│                  │
    │                     │  Bonjour discover │
    │                     │◄─────────────────│
    │                     │  (fp en TXT)      │
    │                     │                  │
    │ Muestra código 6D   │                  │
    │◄─────────────────────────────────────── │
    │  "123456"           │                  │
    │                     │                  │
    │  Introduce código   │                  │
    │────────────────────►│                  │
    │                     │                  │
    │                     │ TLS handshake    │
    │                     │─────────────────►│
    │                     │ (verifica fp)    │
    │                     │                  │
    │                     │ GET /pair?code=  │
    │                     │ 123456           │
    │                     │─────────────────►│
    │                     │                  │
    │                     │ {token, sig,     │
    │                     │  secret}         │
    │                     │◄─────────────────│
    │                     │                  │
    │                     │ Guarda token,    │
    │                     │ fp, secret en    │
    │                     │ EncryptedPrefs   │
    │                     │                  │
    │  "Paired!"          │                  │
    │◄────────────────────│                  │
```

### El código de 6 dígitos: por qué funciona como lo hace

**Generación criptográficamente aleatoria** (`PairingManager.swift`):

```swift
static func generate6DigitCode() throws -> String {
    var digits = ""
    while digits.count < 6 {
        var byte: UInt8 = 0
        SecRandomCopyBytes(kSecRandomDefault, 1, &byte)
        if byte < 250 {  // evita sesgo
            digits += String(byte % 10)
        }
    }
    return digits
}
```

El truco de `byte < 250` es importante: 250 es divisible exactamente por 10 (deja
250 valores válidos: 0–249). Si se usara `byte % 10` directamente sobre 0–255, los
dígitos 0-5 tendrían 26 posibilidades y los 6-9 tendrían 25, generando un sesgo
estadístico detectable. Al descartar 250-255, cada dígito tiene exactamente 25
posibilidades.

**TTL de 300 segundos:** el código expira. El servidor lo elimina del estado
interno tras 5 minutos.

**Un solo uso:** una vez consumido, `active?.consumed = true`. Aunque alguien capture
el código, no puede reutilizarlo.

```swift
guard !a.consumed else { throw PairingError.consumed }
```

**¿Qué pasa si alguien intercepta el código durante 300 segundos?**

El código viaja de Mac a usuario por pantalla (visualización local), y del usuario
a Android por el teclado. No hay transmisión por red del código en sí. Sin embargo,
si el atacante puede observar la pantalla o escuchar el teclado, podría interceptarlo.

El código solo es útil si el atacante también puede hacer una conexión TLS al Mac
(debe estar en la misma red o en el tailnet). Si logra ambas cosas, podría emparejarse.
Esto está documentado como fuera de scope en el threat model.

**¿Qué pasa con fuerza bruta?**

10^6 = 1 millón de combinaciones. Con 300 segundos de TTL y un solo intento válido
(cualquier intento erróneo no "reinicia" el contador pero el código correcto puede
ser enviado una sola vez), el espacio de ataque es limitado. No hay rate limiting
explícito implementado en el servidor para `/pair`, pero el TTL corto y el código
de un solo uso limitan el daño.

---

## 4. TLS con certificado autofirmado

### ¿Por qué TLS incluso en una red doméstica?

Analogía: aunque estés en casa, ¿dejarías la llave debajo del felpudo? La red
doméstica no es inherentemente segura: dispositivos IoT comprometidos, un router
con firmware antiguo, o un vecino con acceso al Wi-Fi pueden hacer ARP spoofing
(interceptar tráfico fingiendo ser otro dispositivo).

TLS garantiza dos cosas: **confidencialidad** (el tráfico va cifrado, nadie puede
leerlo) y **autenticación** (sabes con quién estás hablando). Sin TLS, un
atacante en la LAN podría leer el contenido del portapapeles en texto plano.

### ¿Qué es un certificado autofirmado?

Un certificado X.509 contiene:
- Una clave pública.
- Metadatos (quién lo emitió, para qué dominio, hasta cuándo es válido).
- Una firma digital que certifica que el emisor avala los metadatos.

En un certificado "normal", esa firma la pone una CA conocida. En uno autofirmado,
el emisor y el sujeto son el mismo: el Mac firma su propio certificado.

El sistema operativo Android no confía en este certificado porque no está en su
lista de CAs. Por eso ClipSync implementa su propia validación: SPKI pinning.

### SPKI Pinning: qué es y por qué es superior

SPKI = SubjectPublicKeyInfo. Es la parte del certificado que contiene la clave pública
en formato estándar ASN.1.

El **pin** es SHA-256(SPKI), codificado en base64url. Es el fingerprint de la
clave pública del servidor, independiente de todo lo demás (nombre del dominio,
fecha de expiración, CA emisora).

**Analogía:** Tener la foto del DNI de alguien, no solo su nombre en un papel.
Si alguien te presenta un DNI diferente (aunque el nombre sea el mismo), lo detectas.

En el código:

```swift
// TLSManager.swift
static func spkiFingerprint(certDER: Data) throws -> String {
    let cert = try Certificate(derEncoded: [UInt8](certDER))
    var serializer = DER.Serializer()
    try serializer.serialize(cert.publicKey)
    let spkiBytes = serializer.serializedBytes
    let digest = SHA256.hash(data: spkiBytes)
    return base64URLNoPadding(Data(digest))
}
```

En Android, el cliente verifica el pin manualmente dentro del `TrustManager`:

```kotlin
// ClipClient.kt
override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
    val leaf = chain?.firstOrNull()
        ?: throw CertificateException("Empty certificate chain")
    val actual = Fingerprint.spkiSha256Base64Url(leaf)
    if (actual != fpBase64Url) {
        throw CertificateException(
            "SPKI pin mismatch! Expected=$fpBase64Url Actual=$actual"
        )
    }
}
```

**Por qué es superior a confiar en la CA del sistema:** Un atacante podría instalar
una CA maliciosa en el dispositivo (o ya estar instalada si el dispositivo está
comprometido). SPKI pinning ignora la cadena de CAs: solo valida la clave pública
del servidor específico. Si el pin no coincide, la conexión se rechaza.

### EC P-256 vs RSA

El certificado se genera con `P256.Signing.PrivateKey()` (Curva Elíptica P-256,
también llamada `secp256r1` o `prime256v1`).

**Por qué P-256 sobre RSA:**

| Característica       | RSA-2048         | EC P-256         |
|---------------------|------------------|------------------|
| Tamaño clave privada | 2048 bits        | 256 bits         |
| Tamaño firma         | 256 bytes        | 64 bytes         |
| Seguridad equivalente| ~112 bits        | ~128 bits        |
| Velocidad (handshake)| Más lento        | ~10x más rápido  |
| Soporte              | Universal        | Bien soportado   |

Para dispositivos móviles, la velocidad y el tamaño de los mensajes importan.
P-256 ofrece más seguridad con menos overhead.

### ¿Qué protege TLS y qué NO protege?

**TLS protege:**
- Confidencialidad del contenido del portapapeles en tránsito.
- Integridad básica del canal (si alguien modifica los bytes, la conexión falla).
- Autenticación del servidor (con SPKI pinning).

**TLS NO protege:**
- Contra un atacante que tiene la clave privada del servidor (si el Mac es robado).
- Contra ataques a la aplicación misma (bugs en el código).
- El contenido del portapapeles una vez que llega al destino (es plaintext en memoria).
- Contra un cliente legítimamente emparejado que decide exfiltrar datos.

### Ataque MITM y cómo lo bloquea el pin

Un ataque MITM ("Man In The Middle") es cuando un atacante se coloca entre Mac y
Android interceptando la comunicación:

```
Android → [Atacante] → Mac
          ↑
    Puede leer/modificar todo
```

Con SPKI pinning:
1. El atacante presenta su propio certificado a Android.
2. Android calcula SHA-256(SPKI del certificado del atacante).
3. Ese hash NO coincide con el pin almacenado (que es el del Mac real).
4. La conexión se rechaza con `CertificateException`.

El atacante no puede "falsificar" el pin del Mac sin tener la clave privada del Mac.

---

## 5. HMAC-SHA256

### ¿Qué es HMAC y la diferencia entre integridad y cifrado?

**Cifrado** oculta el contenido. Solo quien tiene la clave puede leerlo.

**HMAC** (Hash-based Message Authentication Code) no oculta nada: verifica que
el mensaje no fue modificado y que lo generó alguien con la clave secreta.

**Analogía:** es como un sello de lacre en una carta. La carta sigue siendo legible,
pero si alguien la abre y vuelve a cerrar, el lacre está roto. En HMAC, el "lacre"
es una firma criptográfica que no puede forjarse sin el secreto.

La fórmula: `HMAC-SHA256(secret, "<timestamp>.<body>")`

En el código:

```swift
// HMACValidator.swift
let signingString = "\(parsed.timestamp).".data(using: .utf8)! + body
let mac = HMAC<SHA256>.authenticationCode(
    for: signingString,
    using: SymmetricKey(data: secret)
)
```

El header resultante: `X-ClipSync-Signature: t=1714123456, v1=a3f7...`

### ¿Por qué el timestamp protege contra replay attacks?

Un **replay attack** funciona así:
1. El atacante captura un request válido con su firma HMAC.
2. Más tarde, lo reenvía al servidor idéntico.
3. El servidor lo acepta porque la firma es correcta.

Ejemplo concreto: imagina que capturas el request que pega "transferir 1000€" en
el portapapeles del Mac. Puedes re-enviarlo mañana y el Mac lo volvería a aplicar.

Con timestamp en la firma:
- El servidor rechaza cualquier request cuyo `t` difiera en más de ±60 segundos
  del tiempo actual.
- El atacante no puede cambiar el timestamp porque eso invalidaría la firma.
- El atacante no puede "usar" el request capturado más de 60 segundos después.

```swift
// HMACValidator.swift
guard abs(now - Double(parsed.timestamp)) < skewSeconds else {
    throw HMACValidationError.replayOrSkew
}
```

### Constant-time comparison: por qué `==` puede ser un fallo de seguridad

Cuando compares strings de forma normal, el procesador devuelve `false` en cuanto
encuentra el primer carácter diferente. Esto crea un **timing attack**: si mides
cuánto tarda la comparación, puedes adivinar cuántos caracteres iniciales son
correctos.

Por ejemplo: si el HMAC correcto es `abc123` y pruebas `xyz000`, la comparación
falla en 0 nanosegundos (la 'x' ya no coincide con 'a'). Si pruebas `abd000`,
falla después de 2 caracteres. Un atacante puede medir estas diferencias.

La solución es comparar TODOS los bytes sin importar si ya encontraste una diferencia:

```swift
// HMACValidator.swift
static func constantTimeEquals(_ a: String, _ b: String) -> Bool {
    guard a.count == b.count else { return false }
    var diff: UInt8 = 0
    let aBytes = Array(a.utf8)
    let bBytes = Array(b.utf8)
    for i in 0..<aBytes.count {
        diff |= aBytes[i] ^ bBytes[i]  // XOR: 0 si iguales, no-cero si diferentes
    }
    return diff == 0  // solo true si TODOS los bytes fueron iguales
}
```

### ¿Qué pasaría si solo hubiera TLS sin HMAC?

Con solo TLS:
- El canal está cifrado y autenticado.
- Pero cualquiera que tenga el Bearer token puede enviar cualquier payload.
- Si el token es robado (del dispositivo, de la memoria, de logs), el atacante
  puede inyectar lo que quiera en el portapapeles del Mac.

Con TLS + HMAC + Bearer:
- **TLS:** confidencialidad e integridad del canal.
- **Bearer:** identifica qué dispositivo emitió el request.
- **HMAC:** verifica que el payload no fue modificado y que el emisor tiene el
  pairing-secret (un secreto distinto al token). Un atacante que solo robe el
  token no puede forjar la firma HMAC sin el pairing-secret.

```
Amenaza: solo robo de Bearer token
→ HMAC bloquea: el atacante no puede firmar payloads sin el pairing-secret

Amenaza: solo robo del pairing-secret
→ Bearer bloquea: el atacante no puede autenticarse sin el token

Amenaza: robo de ambos
→ El dispositivo está completamente comprometido (out of scope)
```

---

## 6. Bearer Token

### ¿Qué es un Bearer token?

Un Bearer token es un credential opaco (una cadena de bytes aleatorios) que el
cliente presenta en cada request: `Authorization: Bearer <token>`. El servidor
lo valida sin necesidad de conocer quién es el cliente — el token "habla por sí solo".

En ClipSync el token se genera en el momento del pairing:

```swift
// PairingManager.swift
let tokenBytes = try Self.randomBytes(count: 32)  // 256 bits de entropía
return PairingResponse(
    token: tokenBytes.base64EncodedString(),
    sig: Data(signature).base64EncodedString(),
    secret: secret.base64EncodedString()
)
```

32 bytes = 256 bits de entropía. La probabilidad de adivinarlo por fuerza bruta
es 1/2^256 — prácticamente imposible.

### Por qué el Mac almacena el SHA-256 del token

El TokenStore nunca guarda el token en plaintext:

```swift
// TokenStore.swift
static func hashHex(_ tokenPlain: String) -> String {
    let digest = SHA256.hash(data: Data(tokenPlain.utf8))
    return digest.map { String(format: "%02x", $0) }.joined()
}
```

**¿Qué ataque evita?**

Si el Keychain del Mac fuera comprometido (ej: un atacante lee los archivos del
disco con el Mac apagado y la clave de cifrado del Keychain obtenida por otro medio),
obtendría los hashes SHA-256 de los tokens. SHA-256 es una función de un solo
sentido: no puede revertirse para obtener el token original.

El atacante con los hashes NO puede autenticarse porque necesita el token original
(plaintext) para enviarlo en el header. Y no puede derivar el plaintext del hash.

### Revocación global via pairing-secret

Si el `pairing-secret` (la clave HMAC) es rotado, todos los tokens se vuelven
inútiles de golpe, incluso sin borrar el TokenStore:

```
Flujo de revocación global:
1. Usuario elimina el Keychain item "com.clipsync.pairing-secret"
2. Mac genera un nuevo secreto al reiniciar
3. Todos los tokens existentes: sus firmas HMAC ya no verifican
   porque el secreto cambió
4. POST /inject → 401 para todos los clientes
5. Todos deben volver a emparejarse
```

Esta es la "bomba atómica" de seguridad: si sospechas que algún dispositivo
fue comprometido pero no sabes cuál, rotas el secreto y fuerza a todos a
volver a emparejarse con el código nuevo.

### Cuándo caduca el token y qué pasa si alguien lo roba

El token **no caduca automáticamente** por diseño. La vida útil es:
- Hasta que el usuario haga "unpair" en Android.
- Hasta que el Mac revoque ese token específico.
- Hasta que se rote el pairing-secret (revocación global).

Si alguien roba el token Y el pairing-secret (ambos almacenados en
`EncryptedSharedPreferences`), puede inyectar contenido en el portapapeles del
Mac indefinidamente — hasta que el propietario lo detecte y revoque.

---

## 7. Almacenamiento de secretos

### macOS Keychain

El Keychain de macOS es un almacén cifrado de secretos gestionado por el sistema.
Conceptualmente, es un archivo cifrado con AES-256 cuya clave maestra se deriva de
la contraseña de login del usuario.

ClipSync usa tres items del Keychain:
- `com.clipsync.tls-identity` → clave privada TLS y certificado.
- `com.clipsync.pairing-secret` → la clave HMAC compartida.
- `com.clipsync.token-store` → los hashes SHA-256 de los tokens emitidos.

**¿Qué pasa si el Mac es robado con sesión abierta?**

Si la sesión está abierta, el Keychain está desbloqueado. El atacante puede
acceder a los secretos si tiene acceso físico al Mac desbloqueado. Esta amenaza
está documentada como out-of-scope: si tienes acceso físico al Mac con sesión
activa, ya tienes el portapapeles directamente.

### Android EncryptedSharedPreferences + Android Keystore

En Android, `Prefs.kt` usa `EncryptedSharedPreferences`:

```kotlin
val masterKey = MasterKey.Builder(context)
    .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
    .build()
EncryptedSharedPreferences.create(
    context,
    FILE,
    masterKey,
    EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
    EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
)
```

La `MasterKey` se genera y almacena en el **Android Keystore**. En dispositivos
modernos, el Keystore está respaldado por hardware (TEE — Trusted Execution
Environment, o StrongBox en Pixel). Esto significa que la clave **nunca sale del
chip de seguridad**, ni siquiera en RAM accesible para el SO.

**Esquema de cifrado:** AES-256-GCM para valores, AES-256-SIV para claves.
SIV (Synthetic IV) es más seguro para claves porque tolera reutilización de nonce
sin vulnerabilidad catastrófica.

### Threat model del almacenamiento

| Escenario de ataque                              | ¿Resiste? | Razón                                            |
|--------------------------------------------------|-----------|--------------------------------------------------|
| App maliciosa sin privilegios, mismo dispositivo | Sí        | Sandboxing Android/macOS                        |
| Forense de disco con dispositivo apagado         | Sí (Android) | TEE: clave nunca en disco descifrable          |
| Forense de disco con dispositivo apagado         | Parcial (Mac) | Keychain cifrado con contraseña del usuario   |
| Malware con root (Android)                       | No        | Root puede extraer claves del TEE en algunos chips |
| Ataque físico con dispositivo desbloqueado       | No        | Keystore accesible con sesión activa            |

---

## 8. El problema del portapapeles en Android

Esta es la parte técnicamente más compleja del proyecto. Android ha ido cerrando
el acceso al portapapeles de forma progresiva por privacidad.

### Historia cronológica de las restricciones

**Android 9 y anterior:** cualquier app puede leer `ClipboardManager.getPrimaryClip()`
en cualquier momento, incluso en background. Las apps de malware podían robar
passwords de gestores de contraseñas de este modo.

**Android 10 (API 29):** las apps en **background** ya no pueden leer el portapapeles.
Solo pueden leerlo apps en primer plano o apps con foco de ventana. Ruptura enorme
para apps de sincronización.

**Android 12 (API 31):** `OnPrimaryClipChangedListener` está bloqueado para procesos
en background, incluyendo `AccessibilityService`. Ya no se puede detectar cambios
por eventos; hay que hacer polling.

**Android 13+ (API 33):** refuerzo adicional — cuando una app accede al portapapeles,
el sistema muestra un toast visual al usuario ("X ha accedido al portapapeles").
Esto hace que el acceso frecuente sea incómodo para el usuario.

**¿Por qué estas restricciones?** En 2019, investigadores descubrieron que apps
como LinkedIn, TikTok o aplicaciones de teclado terceras accedían silenciosamente
al portapapeles en background, potencialmente robando contraseñas, datos bancarios
y otra información sensible que los usuarios habían copiado.

### Las tres capas de ClipSync para superar la barrera

ClipSync implementa tres estrategias en orden de preferencia:

---

#### Capa 1: Shizuku — el shell privilegiado

**¿Qué es ADB?** Android Debug Bridge es una herramienta de desarrollo que permite
a un ordenador conectado por USB ejecutar comandos en el Android con privilegios
de shell — más que una app normal, menos que root.

**Wireless Debugging:** desde Android 11, ADB puede funcionar por Wi-Fi. Shizuku
aprovecha esto: actúa como un "servidor ADB local" en el dispositivo que te permite
ejecutar código con privilegios de shell sin necesitar un ordenador conectado.

**¿Qué es AIDL?** Android Interface Definition Language es el mecanismo de Android
para llamadas entre procesos (IPC). Shizuku lanza un `UserService` que corre en
un proceso privilegiado separado. Ese proceso SÍ puede acceder al portapapeles
sin restricciones porque las restricciones de Android 10+ aplican a procesos de
apps normales, no a procesos de shell.

```kotlin
// ShizukuClipboardManager.kt: máquina de estados
enum class State {
    NOT_INSTALLED,  // Shizuku no está instalado
    NOT_RUNNING,    // instalado pero el daemon no corre
    NO_PERMISSION,  // instalado y running, pero no hay permiso
    BINDING,        // estableciendo conexión con UserService
    READY,          // listo para usar
    DEAD            // UserService murió, intentando reconectar
}
```

**Analogía:** Shizuku es como tener un empleado con llave maestra (el proceso
privilegiado) que te hace recados. Tú no puedes entrar a ciertos sitios (el
portapapeles en background), pero él sí puede y te trae lo que necesitas.

**El costo real:** el usuario debe:
1. Activar "Wireless Debugging" en Opciones de Desarrollador.
2. Instalar la app de Shizuku.
3. Seguir las instrucciones de Shizuku para que adquiera sus privilegios.
4. Dar permiso a ClipSync para usar Shizuku.

Es un setup de ~5 minutos que se hace una sola vez. Después funciona automáticamente.

---

#### Capa 2: Accessibility Service — polling como fallback

Un `AccessibilityService` tiene acceso especial al sistema, diseñado originalmente
para hacer Android accesible a personas con discapacidad. Puede observar eventos
de UI, leer texto en pantalla, y en Android ≤11 puede leer el portapapeles.

**Estrategia:** polling de 500ms con detección por hash:

```kotlin
// ClipAccessibilityService.kt
private val pollRunnable = object : Runnable {
    override fun run() {
        checkClipboard()
        handler.postDelayed(this, POLL_MS)  // POLL_MS = 500
    }
}
```

En lugar de comparar el texto completo (que podría ser largo), se usa `hashCode()`:

```kotlin
val hash = content.hashCode()
if (hash == 0 || hash == lastClipHash) return
```

**¿Por qué solo funciona en Android 11 y menor?**

```kotlin
override fun onServiceConnected() {
    if (Build.VERSION.SDK_INT > Build.VERSION_CODES.R) {  // R = Android 11
        disableSelf()
        return
    }
    // ...
}
```

En Android 12+, incluso los AccessibilityService tienen bloqueado
`OnPrimaryClipChangedListener`. Aunque el polling técnicamente funciona, genera
el toast de privacidad cada 500ms, lo que es inaceptable para el usuario. ClipSync
lo deshabilita proactivamente en Android 12+.

**El costo:** Los AccessibilityService requieren un permiso especial que el
usuario debe activar en Ajustes → Accesibilidad. Google los mira con sospecha
porque apps maliciosas los han abusado para grabar contraseñas, hacer phishing
de autenticación de dos factores, etc. Google Play tiene restricciones sobre qué
apps pueden usar AccesibilityService.

---

#### Capa 3: FAB tap — la solución siempre disponible

FAB = Floating Action Button. Es el botón burbuja flotante que aparece sobre
cualquier app.

**El problema:** para leer el portapapeles en Android 10+, la app necesita tener
una ventana con foco de input. Un servicio en background no tiene ventana, así
que no puede leerlo.

**La solución:** al tocar el FAB, `ClipOverlayManager` lanza `SendClipActivity`:
una Activity transparente (invisible al usuario) que sí tiene ventana y puede
recibir foco.

```kotlin
// SendClipActivity.kt
override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    // Sin setContentView(), el sistema no registra la ventana correctamente.
    // La Activity transparente NECESITA una vista real para recibir foco.
    setContentView(View(this))
    handler.postDelayed(fallbackRunnable, 200)
}

override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus && !clipboardAttempted) {
        clipboardAttempted = true
        handler.removeCallbacks(fallbackRunnable)
        sendClipboard()  // Aquí ya podemos leer el portapapeles
    }
}
```

**El truco del fallback de 200ms:** en algunos Android 12-13, `onWindowFocusChanged`
nunca se dispara para Activities translúcidas. El `postDelayed(fallbackRunnable, 200)`
garantiza que aunque eso ocurra, el portapapeles se lee después de 200ms (tiempo
suficiente para que la ventana se haya establecido).

**Por qué es la más fiable pero menos automática:** siempre funciona porque se
ejecuta en primer plano con ventana activa. Pero requiere que el usuario toque el FAB.

---

### Anti-echo y debounce

Sin estas protecciones, el sistema entraría en bucle infinito:

1. Mac pega texto en Android.
2. Android detecta el cambio de portapapeles.
3. Android envía el texto de vuelta al Mac.
4. Mac detecta el cambio de portapapeles.
5. Mac envía el texto de vuelta a Android.
6. → BUCLE INFINITO.

**Anti-echo Android (2 segundos):**

```kotlin
// ClipboardWriter.kt
@Volatile var lastMacWriteMs: Long = 0L

fun writeText(context: Context, text: String) {
    lastMacWriteMs = System.currentTimeMillis()  // marcamos el tiempo de escritura
    // ...
}

// ClipAccessibilityService.kt / ShizukuClipboardManager
val now = System.currentTimeMillis()
if (now - ClipboardWriter.lastMacWriteMs < 2_000) {
    return  // skip: este cambio lo hicimos nosotros
}
```

**Anti-echo Mac (hash suppression):**

```swift
// PasteboardWatcher.swift
func suppressNextMatching(_ payload: ClipPayload) {
    let digest = Self.digest(for: payload)
    queue.async {
        self.suppressedDigests.append(digest)
        self.lastChangeCount = self.pasteboard.changeCount
    }
}
```

Cuando el Mac escribe en su propio portapapeles (para inyectar lo que llegó de
Android), registra el hash de ese payload. Si el watcher detecta ese mismo contenido,
lo ignora.

**Debounce de 1 segundo entre auto-sends Android:**

```kotlin
if (now - lastAutoSendMs < 1_000) { return }  // skip: debounce
lastAutoSendMs = now
```

Evita que si el usuario pega algo rápidamente varias veces (generando varios
eventos de clipboard change), se envíen múltiples requests al Mac.

---

## 9. Flujos de datos completos

### Flujo 1: Android envía texto al Mac (via FAB tap)

```
Usuario toca FAB
     │
     ▼
ClipOverlayManager lanza SendClipActivity
     │  (FLAG_ACTIVITY_NEW_TASK)
     ▼
SendClipActivity.onCreate()
  setContentView(View(this))  ← ventana registrada
  postDelayed(fallback, 200)
     │
     ▼
onWindowFocusChanged(hasFocus=true)
  clipboardAttempted = true
  handler.removeCallbacks(fallback)
  sendClipboard()
     │
     ▼
ClipboardManager.getPrimaryClip()  ← OK: tenemos foco
     │
     ▼
ClipPayloadBuilder.text(text)
  { type:"text", mime:"text/plain", data:<base64>, ts:..., nonce:... }
     │
     ▼
ClipSender.send()
  1. pinnedClient(host, fp)  ← TLS con SPKI pin
  2. HmacSigner.signatureHeader(secret, ts, body)  ← HMAC-SHA256
  3. Request con Bearer + X-ClipSync-Signature
     │
     ▼
Mac POST /inject
  1. AuthMiddleware verifica Bearer → 401 si inválido
  2. HMACValidator.validate() → 401 si falla
  3. PasteboardInjector.inject(payload)
  4. PasteboardWatcher.suppressNextMatching()  ← anti-echo
     │
     ▼
NSPasteboard.general.setString(text)
     │
     ▼
SendClipActivity.finish()  ← Activity desaparece
Toast "Sent to Mac"
```

**Capas de seguridad aplicadas:**
- TLS + SPKI pin → protege el canal, previene MITM.
- Bearer → identifica el dispositivo.
- HMAC con timestamp → previene replay y tampering.

**¿Qué falla si se rompe una capa?**
- Sin TLS: el contenido viaja en claro, atacante LAN puede leerlo.
- Sin Bearer: cualquiera en la red puede intentar inyectar.
- Sin HMAC: un atacante con el Bearer puede inyectar contenido arbitrario.

---

### Flujo 2: Android envía texto al Mac (via auto-send / Shizuku)

```
ShizukuClipboardManager.getClipboardHash()  ← proceso privilegiado
     │  (cada 500ms desde ClipForegroundService)
     ▼
Hash cambió?
  No → skip
  Sí → anti-echo check (lastMacWriteMs < 2s?)
         Sí → skip (echo)
         No → debounce check (lastAutoSendMs < 1s?)
                Sí → skip
                No → lanzar SendClipActivity(EXTRA_AUTO_SEND=true)
     │
     ▼
[igual que Flujo 1 desde "onWindowFocusChanged"]
```

La diferencia: en auto-send via Shizuku, la Activity se lanza automáticamente
sin intervención del usuario. Desde la perspectiva del usuario, el texto "se
sincroniza solo" tras copiar algo.

---

### Flujo 3: Mac envía texto a Android (NSPasteboard → WebSocket)

```
PasteboardWatcher (poll 500ms en DispatchSourceTimer)
     │
     ▼
pasteboard.changeCount != lastChangeCount?
  No → skip
  Sí → capturePayload()
        1. ¿Hay fileURL? → capturar como archivo
        2. ¿Hay PNG/TIFF? → capturar como imagen (TIFF→PNG)
        3. ¿Hay string? → capturar como texto
     │
     ▼
¿digest está en suppressedDigests?
  Sí → skip (echo de algo que nosotros escribimos)
  No → yield(payload) al AsyncStream
     │
     ▼
ClipServer recibe el payload via stream
  WebSocketHub.broadcast(frame)
     │
     ▼
Todos los WebSocket conectados reciben el frame JSON
     │
     ▼
Android ClipForegroundService recibe frame en onMessage()
  ClipPayload.fromJson(text)
     │
     ▼
¿tipo text?
  ClipboardWriter.writeText(ctx, text)
  lastMacWriteMs = now  ← anti-echo

¿tipo image?
  ImageCache.save(data) → Uri en cacheDir
  ClipboardWriter.writeImage(ctx, uri, mime)
  IncomingClipNotifier.showImageNotification(uri)
```

**Nota sobre WebSocket:** el Mac NO hace polling del portapapeles ni espera
eventos de Android para enviar. Cuando el `PasteboardWatcher` detecta un cambio,
inmediatamente hace broadcast por WebSocket a todos los clientes conectados. La
latencia real es ~500ms (intervalo del poll) + el tiempo de red.

---

### Flujo 4: Mac envía imagen a Android

Igual que el Flujo 3, pero con tipo `image`. La diferencia importante:

```swift
// PasteboardWatcher.swift
if types.contains(.tiff), let data = pasteboard.data(forType: .tiff) {
    if let pngData = Self.tiffToPng(data) {
        return ClipPayload.image(pngData, mime: "image/png")
    }
}
```

macOS usa TIFF internamente. ClipSync convierte TIFF a PNG antes de enviar porque
Android no tiene soporte nativo de TIFF. La conversión usa `NSBitmapImageRep`.

En Android, la imagen se guarda en `cacheDir` y se escribe al portapapeles como
una URI (`content://`). El sistema Android maneja la entrega real de los bytes
cuando otra app accede al portapapeles.

Además, se muestra una notificación con thumbnail para que el usuario sepa que
hay una imagen en el portapapeles.

---

### Flujo 5: Android envía imagen al Mac

```
ClipboardManager.getPrimaryClip()
  mimeType = "image/*"
  item.uri = content://...
     │
     ▼
contentResolver.openInputStream(uri)
  → bytes[]
  
¿bytes.size > MAX_IMAGE_BYTES (20MB)?
  Sí → toast "Image too large", finish()
  No → ClipPayloadBuilder.image(mime, bytes)
       data = Base64.encode(bytes)
     │
     ▼
ClipSender.send()
  [igual que Flujo 1: TLS + Bearer + HMAC]
     │
     ▼
Mac POST /inject
  payload.type == "image"
  data = Base64.decode()
  PasteboardInjector.injectImage(data, mime)
  NSPasteboard.setData(data, forType: .png)
```

La imagen viaja codificada en base64 dentro del JSON. El máximo es 20MB,
configurado en `ClipPayload.maxFileBytes`.

---

## 10. Threat model

### Ataques que ClipSync resiste

**MITM en LAN (ARP spoofing, rogue AP)**

El atacante se pone entre Mac y Android, presentando su propio certificado TLS.
→ Bloqueado por SPKI pinning: el certificado del atacante tiene un SPKI diferente
al del Mac real. Android rechaza la conexión con `CertificateException`.

**Replay attacks**

El atacante captura un request HMAC válido y lo reenvía.
→ Bloqueado por el timestamp: el servidor rechaza requests con `|now - t| > 60s`.
El atacante no puede cambiar el timestamp sin invalidar la firma.

**Tampering de payloads en tránsito**

El atacante modifica el body del request (ej: cambia el texto del portapapeles).
→ Bloqueado por HMAC: la firma cubre el body entero. Cualquier modificación
invalida la firma → 401.

**Acceso sin emparejamiento**

Alguien intenta hacer POST /inject sin token.
→ Bloqueado por AuthMiddleware: requiere `Authorization: Bearer <token>` válido.

**Reutilización del código de pairing expirado o ya usado**

El atacante captura el código de 6 dígitos pero lo usa pasados 5 minutos o cuando
ya fue consumido.
→ Bloqueado: `PairingError.expired` o `PairingError.consumed`.

**Enumeración de códigos de pairing**

El atacante intenta todos los 1.000.000 de combinaciones posibles.
→ Parcialmente bloqueado: el TTL de 300s limita la ventana. Sin rate limiting
explícito implementado, pero el código de un solo uso limita el impacto a un
intento exitoso como máximo.

**Exfiltración de claves desde disco**

Un atacante extrae el disco del Mac o Android para leer los secretos.
→ Bloqueado en Android por TEE (la clave maestra nunca sale del chip).
→ Parcialmente bloqueado en Mac: Keychain cifrado con contraseña de login.
Si el atacante tiene la contraseña del usuario del Mac (ej: contraseña débil o
reutilizada), puede derivar la clave del Keychain.

---

### Ataques que ClipSync NO resiste

**Dispositivo Android comprometido con root**

Con root, el atacante puede leer la memoria del proceso, acceder al Keystore,
o inyectar código en la app. Todo el modelo de seguridad cae.

**Keychain del Mac comprometido**

Si el atacante tiene la contraseña de login del Mac, puede descifrar el Keychain
con herramientas como `security` CLI o `chainbreaker`. Obtendría el pairing-secret
y los tokens → acceso total.

**Ataque físico al dispositivo desbloqueado**

Con el dispositivo en mano y desbloqueado, el atacante puede acceder al portapapeles
directamente, sin necesidad de atacar ClipSync.

**App maliciosa con permisos de Shizuku**

Si otra app en el mismo Android tiene acceso al daemon de Shizuku con permisos
de shell, puede leer el portapapeles igual que ClipSync. Shizuku requiere
conceder permisos app por app, pero si el usuario concede acceso a una app maliciosa,
el portapapeles queda expuesto.

**Exfiltración por dispositivo legítimamente emparejado (by design)**

Si el dispositivo Android es tuyo y confías en él, puede leer todo lo que se
sincroniza. Esto es una característica, no un bug. Si compartes el emparejamiento
con alguien, les estás dando acceso total.

**Ataques a las bibliotecas subyacentes**

Vulnerabilidades en CryptoKit (Apple), NIOSSL, OkHttp, o el TEE del procesador
son out of scope. ClipSync confía en que sus dependencias son seguras.

---

### Comparativa con apps cloud (Pushbullet, Clipt)

| Amenaza                              | ClipSync (local)          | Apps cloud               |
|--------------------------------------|---------------------------|--------------------------|
| MITM en LAN                          | Resistido (SPKI pin)      | Mitigado por HTTPS + CA  |
| Servidor comprometido                | No aplica (sin servidor)  | Riesgo real              |
| Data breach en la nube               | No aplica                 | Riesgo real              |
| Subpoena / solicitud legal           | No aplica                 | Datos en servidor del proveedor |
| Acceso sin estar en misma red        | No funciona               | Funciona siempre         |
| Multi-dispositivo (más de 2)         | Posible (múltiples WS)    | Nativo                   |
| Historial del portapapeles           | No hay                    | Disponible               |
| Dependencia de terceros              | Ninguna                   | Alta                     |

**¿Cuándo usar cada modelo?**

- **ClipSync / local:** perfil de usuario que valora la privacidad absoluta, trabaja
  siempre en casa o con Tailscale, y no necesita historial de portapapeles.

- **Apps cloud:** usuario que necesita acceso desde cualquier red, múltiples
  dispositivos, y acepta que sus datos pasan por servidores de un tercero.

---

## 11. Deuda técnica y decisiones de diseño

### Implementado pero no cableado a la UI

**Revocación por dispositivo individual:** `TokenStore.revoke(id:)` existe y funciona.
El menú bar de Mac tiene la sección "Clients" que muestra los dispositivos conectados,
pero no tiene botón de "Revoke". La estructura de datos está lista; falta el
`NSMenuItem` con target que llame a `revoke()`.

```swift
// MenuBarController.swift — lo que falta:
// let revokeItem = NSMenuItem(title: "Revoke", ...)
// revokeItem.target = self  // ← TODO
```

### Qué falta

- **iOS:** no hay cliente iOS. La arquitectura lo permitiría (el servidor Mac ya
  funciona), pero habría que escribir la app iOS con las mismas capas de seguridad.
- **Windows:** no hay servidor Windows.
- **Rotación automática de tokens:** los tokens no tienen TTL. Un sistema robusto
  rotaría los tokens periódicamente.
- **File sync completo bidireccional:** el Mac puede enviar archivos a Android
  (tipo `file` implementado). Android puede enviar archivos al Mac via
  `MacShareActivity`. Pero el flow no está completamente probado end-to-end.

### Por qué mDNS no funciona sobre Tailscale

mDNS usa multicast UDP (`224.0.0.251:5353`). WireGuard (que Tailscale usa
internamente) es un túnel unicast punto a punto: no forwarded multicast.

Para arreglarlo sin cambiar el protocolo de descubrimiento habría que:
1. Usar un servidor DNS interno en el tailnet (Tailscale permite "MagicDNS"
   pero requiere configuración adicional).
2. Implementar un mecanismo de descubrimiento custom sobre Tailscale (ej:
   un servidor HTTPS de anuncio en la IP del tailnet).

La solución actual (IP manual) es pragmática y funciona.

### Por qué se eligió Hummingbird 2.x

Hummingbird es el framework HTTP/WebSocket de Swift que ClipSync usa en el Mac.
La versión 2.x usa Swift concurrency (`async/await`) nativo, lo que simplifica
mucho el código. El tradeoff: requiere macOS 14 (Sonoma) o superior.

Esto excluye Macs con macOS 12 o 13. La decisión fue consciente: el target de
usuario es alguien con Mac actualizado.

### Decisiones tomadas a propósito

**Sin cloud, sin cuenta:**
- Ventaja: privacidad total, sin servidores que mantener, sin GDPR, sin costes.
- Consecuencia: el usuario debe estar en la misma red o configurar Tailscale.
  No hay sincronización cuando ambos dispositivos están en redes distintas sin VPN.

**TOFU en lugar de PKI:**
- Ventaja: cero fricción (sin CAs, sin certificados pagados, sin ACME/Let's Encrypt).
- Consecuencia: si el TOFU inicial es interceptado (atacante ya en MITM posición),
  el atacante pina su propia clave. El canal visual del código de 6 dígitos es la
  última línea de defensa.

**Bearer token sin expiración:**
- Ventaja: el usuario no tiene que volver a emparejarse periódicamente.
- Consecuencia: si el token es robado y el usuario no lo sabe, el atacante tiene
  acceso indefinido. Mitigación: la revocación manual existe (aunque no esté en la UI).

---

*Documento generado desde el código fuente de ClipSync v0.1.0 (abril 2026).*  
*Archivos analizados: TLSManager.swift, HMACValidator.swift, PairingManager.swift,*  
*TokenStore.swift, ClipClient.kt, PairingApi.kt, ShizukuClipboardManager.kt,*  
*ClipAccessibilityService.kt, SendClipActivity.kt, ClipSender.kt, PasteboardWatcher.swift,*  
*BonjourAdvertiser.swift, Prefs.kt, SettingsViewModel.kt, security.md, protocol.md, threat-model.md.*
