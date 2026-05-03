# Error Handling Overhaul — State
## Status: COMPLETE
## Current Phase: 5 (all done)
## Completed Phases: [1, 2, 3, 4, 5]
## Branch: feature/error-handling-ux
## Commits:
1. `107a82b` feat[mac-errors]: add AppError model, ErrorStore, and LocalizedError conformances
2. `bf245db` feat[mac-errors]: surface TLS fallback and server errors in menu bar
3. `b71a012` feat[android-errors]: add AppError model, ErrorBanner composable, migrate SettingsState
4. `1e293ab` fix[android-errors]: surface all silent catches as user-visible AppErrors
5. `ba1108b` feat[mac-errors]: propagate server and WebSocket errors to ErrorStore
6. `5b709c0` feat[android-errors]: propagate discovery and ping errors with context
7. `a7c41c1` feat[errors]: add native notifications for critical errors and copy-error support
## Notes: Ready for review. Branch NOT merged to main.
