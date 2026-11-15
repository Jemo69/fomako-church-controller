#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build-linux.sh
if command -v x86_64-w64-mingw32-gcc >/dev/null; then
  ./scripts/build-windows.sh || echo "windows build failed (see above)"
else
  echo "skip windows: mingw not installed (sudo apt install -y mingw-w64)"
fi
if [[ "$(uname)" == "Darwin" ]]; then
  ./scripts/build-mac.sh
else
  echo "skip mac: run scripts/build-mac.sh on a Mac"
fi
