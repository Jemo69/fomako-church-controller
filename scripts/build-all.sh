#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build-linux.sh
if cargo zigbuild --help >/dev/null 2>&1; then
  ./scripts/build-windows.sh || echo "windows build failed (see above)"
else
  echo "skip windows: cargo-zigbuild not installed (cargo install cargo-zigbuild + zig)"
fi
if [[ "$(uname)" == "Darwin" ]]; then
  ./scripts/build-mac.sh
else
  echo "skip mac: Mac builds need a Mac (or GitHub Actions). See .github/workflows/build.yml"
fi
