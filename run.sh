#!/usr/bin/env bash
# Build frontend lalu jalankan backend (macOS/Linux). Setara dengan 3 perintah di README.
set -euo pipefail
cd "$(dirname "$0")"
npm --prefix frontend ci
npm --prefix frontend run build
cargo run --release --manifest-path backend/Cargo.toml
