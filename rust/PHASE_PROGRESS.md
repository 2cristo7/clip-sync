# Phase 0: Archive Swift Server & Extract Golden Tests
## Tasks
- [x] 0.1 Create dev branch, extract golden test data
- [x] 0.2 Archive Swift server (git mv mac/ mac-legacy/), update docs
- [x] 0.3 Merge chore/archive-swift → dev
## Test Results
N/A — no code to test, only file operations and golden data creation.
## Notes
- Golden test HMAC values computed with real HMAC-SHA256 via python3.
- hmac_vector.json: secret=deadbeefcafebabe1234567890abcdef, sig=7267897e...
- pair_response.json: sig=lze3X+4SqEIS7HWzJR6bFaezMziCynROKo7+p8QTM6M=
- All 52 Swift files moved from mac/ to mac-legacy/ with git mv (history preserved).
- CLAUDE.md and README.md updated to reflect mac-legacy/ paths and Rust migration.
