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
ANDROID_SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/android-sdk}}"
if [[ -d "$ANDROID_SDK" ]]; then
  ./scripts/build-android.sh || echo "android build failed (see above)"
else
  echo "skip android: Android SDK not found at $ANDROID_SDK"
fi
if rustup target list --installed | grep -q wasm32-unknown-unknown || command -v wasm-bindgen >/dev/null 2>&1; then
  ./scripts/build-web.sh dist/web
  mkdir -p dist/web-release
  cp -r dist/web/* dist/web-release/
  cp -f scripts/visca-relay.py dist/web-release/
  (cd dist && rm -f fomako-church-controller-web.zip && zip -r -q fomako-church-controller-web.zip web-release)
fi
(cd dist && sha256sum fomako-church-controller-* 2>/dev/null > SHA256SUMS || true)
ls -lh dist/
