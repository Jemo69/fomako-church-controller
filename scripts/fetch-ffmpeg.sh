#!/usr/bin/env bash
# Fetch a static ffmpeg next to the app (or anywhere on PATH) so the live
# preview works with zero setup on the booth PC. Idempotent: skips what exists.
#   scripts/fetch-ffmpeg.sh [dest-dir]   (default: this repo's dist/)
# Windows (Git Bash): downloads gyan.dev essentials. Linux x86_64: johnvansickle static.
# macOS: static builds rot too fast to hotlink — the script prints the brew line instead.
set -euo pipefail
DEST="${1:-$(dirname "$0")/../dist}"
mkdir -p "$DEST"
have() { command -v "$1" >/dev/null 2>&1; }
OS="$(uname -s)"
ARCH="$(uname -m)"

if have ffmpeg; then
  echo "ffmpeg already on PATH: $(command -v ffmpeg)"
fi

case "$OS" in
  MINGW*|MSYS*|CYGWIN*|Windows_NT*)
    OUT="$DEST/ffmpeg.exe"
    if [ -x "$OUT" ]; then echo "exists: $OUT"; exit 0; fi
    URL="https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
    echo "downloading gyan.dev essentials (~80MB)..."
    tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
    curl -sSL --max-time 600 -o "$tmp/ff.zip" "$URL"
    if have unzip; then unzip -p "$tmp/ff.zip" '*/bin/ffmpeg.exe' > "$OUT"
    else python3 -c "import sys,zipfile; z=zipfile.ZipFile(sys.argv[1]); n=[n for n in z.namelist() if n.endswith('/bin/ffmpeg.exe')][0]; open(sys.argv[2],'wb').write(z.read(n))" "$tmp/ff.zip" "$OUT"
    fi
    chmod +x "$OUT"; echo "saved: $OUT"; ls -lh "$OUT"
    ;;
  Linux*)
    if [ "$ARCH" != "x86_64" ]; then echo "no static link for $ARCH here — apt install ffmpeg instead"; exit 1; fi
    OUT="$DEST/ffmpeg"
    if [ -x "$OUT" ]; then echo "exists: $OUT"; exit 0; fi
    URL="https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
    echo "downloading johnvansickle static (~70MB)..."
    tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
    curl -sSL --max-time 600 -o "$tmp/ff.txz" "$URL"
    tar -xJf "$tmp/ff.txz" -C "$tmp"
    cp "$tmp"/ffmpeg-*-amd64-static/ffmpeg "$OUT"
    chmod +x "$OUT"; echo "saved: $OUT"; ls -lh "$OUT"
    ;;
  Darwin*)
    echo "macOS: static builds rot fast — run this instead:"
    echo "  brew install ffmpeg"
    exit 2
    ;;
  *)
    echo "unknown OS $OS — apt/brew/winget install ffmpeg, or set FFMPEG_BIN"
    exit 1
    ;;
esac
echo "tip: drop it next to fomako-church-controller(.exe); the app finds it automatically."
