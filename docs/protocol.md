# Wire Protocol — Shared Clipboard

Referencia canónica del protocolo de sincronización entre el servidor macOS y los clientes Android.

## Wire Protocol (shared reference)

- **Transporte**: HTTPS + WebSocket sobre el mismo puerto (por defecto `7010`).
- **Endpoints HTTP**:
  - `GET /health` → `{"ok":true,"version":"x.y.z"}`
  - `POST /inject` (Pixel → Mac) → body: `{"type":"text|image","mime":"...","data":"<base64>","ts":..., "nonce":"...", "hmac":"..."}`
  - `GET /pair?code=XXXX` → devuelve token si el código está activo.
- **WebSocket** en `/ws` (Mac → Pixel): frames JSON con el mismo esquema que `/inject`.
- **Auth**: header `Authorization: Bearer <token>` + HMAC-SHA256 del body con el `pairing-secret`.
- **Tamaño máximo de imagen**: 20 MB (configurable).
