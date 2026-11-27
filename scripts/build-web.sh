#!/usr/bin/env bash
# Browser build for phones + desktops (Rune web target).
# Serves the same UI; camera traffic goes via scripts/visca-relay.py.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-dist/web}"
RUNE_WEB_BUILD="$HOME/.cargo/git/checkouts/rune-c9b805e9ed8a3730/4a072bb/scripts/web-build.sh"
if [ ! -x "$RUNE_WEB_BUILD" ]; then
  RUNE_WEB_BUILD="$(find ~/.cargo/git/checkouts/rune-* -maxdepth 2 -name web-build.sh 2>/dev/null | head -n 1)"
fi
if [ -z "${RUNE_WEB_BUILD:-}" ] || [ ! -f "$RUNE_WEB_BUILD" ]; then
  echo "rune web-build.sh missing, fetching the rune git dep..."
  cargo fetch --target wasm32-unknown-unknown >/dev/null 2>&1 || cargo fetch >/dev/null 2>&1 || true
  RUNE_WEB_BUILD="$(find ~/.cargo/git/checkouts/rune-* -maxdepth 2 -name web-build.sh 2>/dev/null | head -n 1)"
fi
if [ -z "${RUNE_WEB_BUILD:-}" ] || [ ! -f "$RUNE_WEB_BUILD" ]; then
  echo "rune web-build.sh not found (cargo fetch the rune git dep first)"
  exit 1
fi
rustup target add wasm32-unknown-unknown
if ! command -v wasm-bindgen >/dev/null; then
  VER="$(grep -A2 'name = \"wasm-bindgen\"' Cargo.lock | head -n 3 | grep version | head -n 1 | cut -d'\"' -f2)"
  echo "installing wasm-bindgen-cli $VER (one time)..."
  cargo install wasm-bindgen-cli --version "$VER" --locked
fi
bash "$RUNE_WEB_BUILD" fomako-church-controller --out "$OUT"
echo "---"
echo "web UI in $OUT — serve + relay together:"
echo "  python3 scripts/visca-relay.py --port 8080 --serve-web $OUT"
echo "phone -> http://<this-PC-LAN-IP>:8080"
