#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p dist
echo "== Windows x86_64 (gnu) release, cross from Linux =="
echo "needs: rustup target add x86_64-pc-windows-gnu + mingw-w64 (x86_64-w64-mingw32-gcc)"
rustup target add x86_64-pc-windows-gnu
if ! command -v x86_64-w64-mingw32-gcc >/dev/null; then
  echo "install mingw: sudo apt install -y mingw-w64"
  exit 1
fi
cargo build --release --target x86_64-pc-windows-gnu
cp -f target/x86_64-pc-windows-gnu/release/fomako-church-controller.exe dist/fomako-church-controller-windows-x64.exe
ls -lh dist/
