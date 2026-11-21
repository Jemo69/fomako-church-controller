#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p dist
echo "== Windows x86_64 release (cross from Linux via zig, no mingw needed) =="
rustup target add x86_64-pc-windows-gnu
if command -v cargo-zigbuild >/dev/null 2>&1 || cargo zigbuild --help >/dev/null 2>&1; then
  cargo zigbuild --release --target x86_64-pc-windows-gnu
else
  echo "cargo-zigbuild not found, falling back to plain cargo (needs mingw-w64)"
  cargo build --release --target x86_64-pc-windows-gnu
fi
cp -f target/x86_64-pc-windows-gnu/release/fomako-church-controller.exe dist/fomako-church-controller-windows-x64.exe
ls -lh dist/
file dist/fomako-church-controller-windows-x64.exe || true
