# Shared Clipboard

Sincronización de portapapeles (texto e imágenes) entre macOS y Google Pixel, sin nubes, sin cuentas y de código abierto.

El Mac actúa como servidor (icono en la barra de menú) y expone un endpoint HTTPS + WebSocket en `0.0.0.0:7010`. El Pixel es el cliente: recibe cambios del Mac como notificación (un tap copia al `ClipboardManager`) y envía contenido al Mac mediante el menú nativo de compartir de Android (`ACTION_SEND`).

Redes soportadas:

- **LAN**: descubrimiento automático por mDNS/Bonjour (`_clipsync._tcp`).
- **Tailscale**: IP `100.x.x.x` manual para conectar fuera de la LAN común.

Seguridad: pairing con código de un solo uso, HMAC-SHA256 en cada payload, TLS self-signed con pinning por fingerprint publicado en el TXT de Bonjour.

Estructura del monorepo:

- `mac/` — app macOS (Swift, Hummingbird).
- `android/` — cliente Android (Kotlin, Compose, OkHttp).
- `docs/` — protocolo y modelo de amenazas.
