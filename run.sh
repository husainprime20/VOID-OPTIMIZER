#!/usr/bin/env sh
# Linux / macOS: build and serve the UI (simulation mode — tweaks are Windows-only)
set -e
cd "$(dirname "$0")"
cargo build --release
exec ./target/release/void "$@"
