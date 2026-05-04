# inject_400_decode.json

Golden vector: 400 Bad Request body returned by `POST /inject` when the body
is not valid JSON or fails to decode into a `ClipPayload`.

- Source of truth: Mac Swift `ClipServer.swift` / Phase 1.3 commit `4f8ab8de`
  (inject 400 shape with `error` + `message`).
- Server: `clipsync-server::errors::InjectError::DecodeError` →
  serialized via `axum::Json` → `{"error":"decode_error","message":"<detail>"}`.

## Wire shape

```json
{"error":"decode_error","message":"invalid JSON: expected value at line 1 column 2"}
```

## All `/inject` 4xx codes

`/inject` 4xx bodies always have shape `{ "error": <code>, "message": <text> }`.

| Code                     | HTTP | Trigger                                            |
|--------------------------|------|----------------------------------------------------|
| `decode_error`           | 400  | Body is not valid JSON OR shape mismatches schema  |
| `unsupported_kind`       | 400  | `type` is a string but not `text`/`image`/`file`   |
| `timestamp_out_of_range` | 400  | `ts` deviates from now by more than ±5 minutes     |
| `payload_too_large`      | 400  | Body exceeds `MAX_PAYLOAD_BYTES` (20 MB)           |

The `message` field carries human-readable detail (e.g. exact serde error,
size in bytes, or the unknown kind value). The exact `message` text is
**not** part of the wire contract — only the `error` code is. This vector
captures one representative message for shape-level assertions.
