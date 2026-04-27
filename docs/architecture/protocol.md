# Wire Protocol

Canonical reference for the sync protocol between the macOS server and Android clients.

## Transport

HTTPS + WebSocket on the same port (default `7010`).

## HTTP endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Returns `{"ok":true,"version":"x.y.z"}` |
| `GET` | `/pair?code=XXXX` | Returns bearer token if the code is active |
| `POST` | `/inject` | Android → Mac: push clipboard content |
| `GET` | `/ws` | Upgrade to WebSocket (Mac → Android: push) |

## Payload schema

All payloads (both `/inject` and WebSocket frames) use the same JSON schema:

```json
{
  "type":  "text | image | file",
  "mime":  "text/plain | image/png | …",
  "data":  "<base64-encoded content>",
  "ts":    1714000000,
  "nonce": "<random string>",
  "hmac":  "<HMAC-SHA256 hex of the body>"
}
```

## Authentication

Every request must include:

```
Authorization: Bearer <token>
```

The `hmac` field is HMAC-SHA256 of the raw request body, keyed with the `pairing-secret` exchanged during pairing.

Timestamps are validated within **±60 seconds** to prevent replay attacks.

## Limits

- Maximum payload size: **20 MB** (configurable in `ServerConfig.swift`)
