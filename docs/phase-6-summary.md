# Phase 6 Summary — Android Notifications + ClipboardManager Injection

**Branch**: `feature/android-notifications-clipboard` (merged into `main` via `12968fc`, `--no-ff`).

## What shipped
- `IncomingClipNotifier` — creates channel `clipsync_incoming` (IMPORTANCE_DEFAULT, sound off). Text clips use `BigTextStyle` with a ≤120-char preview; image clips decode base64 bytes, cache under `cacheDir/clipsync/<uuid>.<ext>`, and use `BigPictureStyle`.
- `ApplyClipActivity` — translucent, `singleTask`, `noHistory` trampoline. Reads extras, delegates to `ClipboardWriter`, shows a short Toast, finishes.
- `ClipboardWriter` — pure helper over `ClipboardManager`. `newPlainText("clipsync", text)` for text; `newUri` with `FLAG_GRANT_READ_URI_PERMISSION` for images.
- `ImageCache` — writes decoded bytes under `cacheDir/clipsync/`, exposes via FileProvider authority `com.clipsync.fileprovider`, cleans files older than 24h on service start. Testable constructor injection for JVM tests.
- `file_paths.xml` — `<cache-path name="clipsync" path="clipsync/"/>`.
- `AndroidManifest.xml` — registers `ApplyClipActivity` (exported=false) and `FileProvider` (grantUriPermissions=true).
- `ClipForegroundService` — calls `ImageCache.cleanupOlderThan24h()` on start; `onFrame(payload)` now invokes `IncomingClipNotifier.notify`. Guards against revoked `POST_NOTIFICATIONS` on API 33+.

## Commits
```
6555dac feat[android-notifications]: show notification on incoming clip
1557505 feat[android-clipboard]: support image clips via FileProvider uri
b4b00da feat[android-clipboard]: write text clips via ApplyClipActivity trampoline
```

## Validation
- `./gradlew :app:testDebugUnitTest` → **23 passed, 0 failed** (11 prior + 12 new across `ClipboardWriterTest`, `ImageCacheTest`, `IncomingClipNotifierHelpersTest`).
- `./gradlew :app:assembleDebug` → BUILD SUCCESSFUL.
- `./gradlew :app:lintDebug` → clean, no blocking errors.
- Manifest inspection confirms `ApplyClipActivity` exported=false and FileProvider authority `com.clipsync.fileprovider` with `grantUriPermissions=true`.

## Deviations from plan
- Tests are JVM-only (no Robolectric added). `ImageCache` exposes an `internal` secondary constructor taking a `rootProvider: () -> File` so tests avoid `android.jar`. Notifier logic was split into pure helpers (`IncomingClipNotifierHelpersTest` covers them).

## Out of scope / Follow-ups
- Functional end-to-end test (Mac copy → Pixel notif → paste) is manual; tracked for later QA.
- No new runtime permission prompts — `POST_NOTIFICATIONS` was declared in Phase 5.
- Share Target (Phase 7) can reuse `ClipboardWriter.LABEL = "clipsync"` and the FileProvider authority if needed (likely not — Share flow is Android → Mac).
