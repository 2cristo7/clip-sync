# Threat Model — Shared Clipboard (v0)

Modelo inicial de amenazas. Se revisará en la Fase 4 cuando el stack de seguridad (TLS + HMAC + Bearer) esté operativo.

## Alcance

- Activos a proteger: contenido del portapapeles (texto + imágenes), pairing-secret, token de sesión, clave privada del cert TLS self-signed.
- Dispositivos confiables: Mac emparejado y Pixel emparejado.
- Redes consideradas: LAN Wi-Fi doméstica, tailnet (Tailscale).

## Amenazas y mitigaciones

### T1. MITM en LAN (ARP spoof / rogue AP)

- **Escenario**: un atacante en la misma LAN intercepta el tráfico entre Mac y Pixel.
- **Impacto**: captura o modificación de contenido del clipboard, robo del token Bearer, inyección de payloads arbitrarios.
- **Mitigación**:
  - TLS obligatorio (Fase 4). El cert self-signed del Mac se pinnea en el cliente por el fingerprint SHA-256 publicado en el TXT de Bonjour durante el pairing (TOFU).
  - HMAC-SHA256 del body con `pairing-secret` en cada payload: el atacante no puede forjar ni modificar mensajes sin el secreto.
  - El Bearer token por sí solo no basta; siempre se valida junto con la firma HMAC.

### T2. Exit-node / nodo malicioso en Tailscale

- **Escenario**: un nodo comprometido dentro del tailnet intenta conectarse al Mac o relayar tráfico.
- **Impacto**: acceso no autorizado a `/inject` o al WebSocket `/ws`.
- **Mitigación**:
  - Bearer token obtenido por pairing con código de 6 dígitos válido 5 minutos y un solo uso.
  - HMAC por payload (no basta con tener el token si no se tiene el `pairing-secret`).
  - Revocación manual de tokens desde el menú bar del Mac (lista de clientes conectados con opción "Revoke").
  - ACLs de Tailscale recomendadas en `docs/tailscale-setup.md` (Fase 8) para restringir qué peers pueden alcanzar el puerto 7010.

### T3. Dispositivo comprometido con el pairing-secret

- **Escenario**: un Pixel emparejado es robado, desbloqueado, o su almacenamiento es extraído.
- **Impacto**: el atacante puede falsificar payloads firmados y mantener sesión.
- **Mitigación**:
  - `pairing-secret` almacenado en Keychain (Mac) y EncryptedSharedPreferences (Android), ligado al keystore del device.
  - Rotación del `pairing-secret` en el Mac regenera el secreto e invalida todos los tokens emitidos, forzando re-pairing de todos los dispositivos.
  - Revocación selectiva por `deviceLabel` desde el menú bar.
  - Desde el lado Android, la app puede limpiar Prefs al detectar `Authorization` inválido reiterado.

### T4. Ataques de replay

- **Escenario**: un atacante captura un payload válido (firmado) y lo reemite más tarde o repetidamente.
- **Impacto**: ejecución repetida de `/inject`, DoS ligero, posible confusión del usuario.
- **Mitigación**:
  - Cada payload incluye `ts` (epoch ms) y `nonce` (UUID). El servidor rechaza si `|now - ts| > 60s`.
  - Cache de nonces vistos en una ventana de 120s para rechazar reenvíos exactos.
  - La firma HMAC cubre `ts.body`, por lo que modificar el timestamp invalida la firma.

### T5. Payload oversize (DoS)

- **Escenario**: un cliente (legítimo comprometido o atacante con credenciales) envía imágenes de cientos de MB para agotar memoria/disco.
- **Impacto**: OOM en el Mac, llenado del `cacheDir` en Android, degradación o crash.
- **Mitigación**:
  - Límite configurable (default 20 MB) validado antes de decodificar base64; la lectura del body se aborta al superar el umbral.
  - En Android, `ShareReceiverActivity` rechaza imágenes > 20 MB antes del POST con Toast "Image too large".
  - Rate limiting simple en `/inject` (p. ej. máximo 10 req/s por token).
  - Limpieza de imágenes en `cacheDir` con TTL de 24h (Fase 6).

## No incluido en v0

- Forward secrecy por mensaje (se añadiría con Noise o Double Ratchet si se amplía a multi-usuario).
- Defensa contra atacante físico con acceso a un Mac desbloqueado (fuera del modelo: si tiene el Mac, ya tiene el clipboard).
- Compromiso del Keychain de macOS o del Android Keystore (asumidos confiables).
