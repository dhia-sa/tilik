# Build frontend lalu jalankan backend (Windows PowerShell). Setara dengan 3 perintah di README.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
npm --prefix frontend ci
npm --prefix frontend run build
cargo run --release --manifest-path backend/Cargo.toml
