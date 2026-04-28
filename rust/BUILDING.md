# Building ClipSync

## Prerequisites

- Rust 1.75+ (install via [rustup](https://rustup.rs/))
- Platform-specific dependencies (see below)

### macOS

No extra dependencies. Xcode Command Line Tools recommended.

### Linux

```bash
sudo apt-get install -y \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev
```

### Windows

No extra dependencies. Visual Studio Build Tools with C++ workload recommended.

## Quick Build

```bash
cd rust

# Debug build (fast compile)
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run clippy
cargo clippy --all-targets -- -D warnings
```

## Building Individual Crates

```bash
# Core library only
cargo build -p clipsync-core
cargo test -p clipsync-core

# Server binary
cargo build -p clipsync-server --release

# Client binary
cargo build -p clipsync-client --release
```

Release binaries are placed in `rust/target/release/`.

## Packaging

### macOS (.app bundles)

```bash
./rust/packaging/scripts/package-macos.sh --version 0.2.0
# Output: rust/dist/ClipSync-Server.app, rust/dist/ClipSync-Client.app
```

### Linux (.deb packages)

```bash
./rust/packaging/scripts/package-linux.sh --version 0.2.0
# Output: rust/dist/clipsync-server_0.2.0_amd64.deb, etc.
```

### Windows (.zip archives)

```powershell
.\rust\packaging\scripts\package-windows.ps1 -Version "0.2.0"
# Output: rust\dist\clipsync-server-0.2.0-windows-x86_64.zip, etc.
```

## Cross-Compilation

### Using `cross` for ARM Linux

[`cross`](https://github.com/cross-rs/cross) uses Docker to cross-compile without configuring toolchains manually.

```bash
cargo install cross --git https://github.com/cross-rs/cross

# Build for ARM64 Linux (e.g., Raspberry Pi 4)
cd rust
cross build -p clipsync-server --release --target aarch64-unknown-linux-gnu
cross build -p clipsync-client --release --target aarch64-unknown-linux-gnu

# Build for ARMv7 Linux
cross build -p clipsync-server --release --target armv7-unknown-linux-gnueabihf
```

### Native Cross-Compilation (without Docker)

Add the target and a linker, then build:

```bash
rustup target add aarch64-unknown-linux-gnu
# Install the cross-linker (Ubuntu/Debian):
sudo apt-get install gcc-aarch64-linux-gnu

# Set linker in .cargo/config.toml:
# [target.aarch64-unknown-linux-gnu]
# linker = "aarch64-linux-gnu-gcc"

cargo build -p clipsync-server --release --target aarch64-unknown-linux-gnu
```

### macOS Universal Binary (x86_64 + ARM64)

```bash
rustup target add x86_64-apple-darwin aarch64-apple-darwin

cargo build -p clipsync-server --release --target x86_64-apple-darwin
cargo build -p clipsync-server --release --target aarch64-apple-darwin

# Combine into universal binary
lipo -create \
  target/x86_64-apple-darwin/release/clipsync-server \
  target/aarch64-apple-darwin/release/clipsync-server \
  -output target/release/clipsync-server-universal
```

## CI Targets

The CI pipeline (`.github/workflows/rust-ci.yml`) produces binaries for:

| Platform | Architecture | Runner |
|----------|-------------|--------|
| Linux    | x86_64      | ubuntu-latest |
| macOS    | ARM64       | macos-latest |
| Windows  | x86_64      | windows-latest |

For additional targets (aarch64-linux, x86_64-darwin), use `cross` locally or extend the CI matrix.
