# pairing_error_invalid.json

Golden vector: 401 Unauthorized body returned by `GET /pair?code=<wrong>` when
the submitted code does not match the active pairing code.

- Source of truth: Mac Swift `PairingManager.swift` / Phase 1.5 commits
  `62ad7bc6` and `b3cc3159` (pairing 401 error bodies).
- Server: `clipsync-server::pairing` route — `{"error":"invalid"}` (no `message` field).

## Wire shape

```json
{"error":"invalid"}
```

## All `/pair` 401 codes (machine-readable)

| Code         | Trigger                                          |
|--------------|--------------------------------------------------|
| `invalid`    | Wrong code submitted while another code is active |
| `expired`    | Active code's TTL has elapsed                     |
| `consumed`   | Code was already exchanged successfully           |
| `notStarted` | No code has been generated yet                    |

`/pair` 401 bodies must NOT include a `message` field — the code IS the message.
This differs from `/inject` 4xx, which always includes `{error, message}`.
