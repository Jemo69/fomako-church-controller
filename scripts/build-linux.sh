#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p dist
echo "== Linux x86_64 release =="
cargo build --release
cp -f target/release/fomako-church-controller dist/fomako-church-controller-linux-x64
cp -f shots/*.png dist/ 2>/dev/null || true
ls -lh dist/
echo "run: ./dist/fomako-church-controller-linux-x64"
