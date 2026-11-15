#!/usr/bin/env sh
# Linux / macOS: build and open the native window.
# Simulation mode — tweaks are Windows-only, so they are logged rather than run.
set -e
cd "$(dirname "$0")"
cargo build --release --bins
exec ./target/release/void "$@"
