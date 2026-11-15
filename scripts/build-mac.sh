#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p dist
echo "== macOS builds (run this script ON a Mac) =="
rustup target add x86_64-apple-darwin aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cp -f target/x86_64-apple-darwin/release/fomako-church-controller dist/fomako-church-controller-mac-x64
cp -f target/aarch64-apple-darwin/release/fomako-church-controller dist/fomako-church-controller-mac-arm64
ls -lh dist/
