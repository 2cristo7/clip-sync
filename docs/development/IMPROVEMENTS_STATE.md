# General Improvements — State
## Status: COMPLETE
## Current Phase: 5 (done)
## Completed Phases: [1, 2, 3, 4, 5]
## Branch: chore/general-improvements
## Last Commit: fb2bb3e refactor[android-ui]: split SettingsScreen into section composables

## Summary (13 commits)

### Phase 1 — Security Hardening
- `c08bfb1` fix[android-security]: add payload validation and size guard before decode
- `6db869c` fix[mac-security]: upgrade Keychain accessibility to WhenUnlockedThisDeviceOnly
- `e9d65ae` fix[mac-security]: add rate limiting, payload validation, and reduce HMAC skew

### Phase 2 — Performance & Battery
- `9ea2fc7` fix[mac-perf]: increase poll interval to 750ms and echo suppression window to 16
- `697b130` fix[android-perf]: adaptive Shizuku polling, coroutine health check, scope cleanup
- `360c9a6` fix[android-memory]: add cache size limit, subsample large images, recycle bitmaps

### Phase 3 — Build & Config
- `82a5e0e` chore[ci]: pin macOS runner, add Gradle cache, upload test artifacts
- `3c533bc` chore[config]: centralize port constant, add VERSION file, expand gitignore
- `2b3b48f` chore[android-build]: bump targetSdk 35, enable R8 minification, stable securityCrypto

### Phase 4 — UX Polish
- `5fab952` feat[mac-ux]: add pause/resume sync toggle to menu bar
- `b7e023a` feat[android-ux]: implement ErrorAction handlers, accessibility labels, pairing timeout
- `14efa53` feat[android-network]: add retry with backoff for transient send errors

### Phase 5 — Code Quality
- `fb2bb3e` refactor[android-ui]: split SettingsScreen into section composables

## Notes
- securityCrypto kept at 1.1.0-alpha06 (1.0.0 lacks MasterKey.Builder)
- Branch NOT merged to main — ready for review
