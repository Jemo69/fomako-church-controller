#!/usr/bin/env bash
# Build native Android APK (arm64-v8a + x86_64) using Rust JNI + Jetpack Compose.
set -euo pipefail
cd "$(dirname "$0")/.."

ANDROID_SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/android-sdk}}"
if [ ! -d "$ANDROID_SDK" ]; then
  echo "Android SDK not found at $ANDROID_SDK (set ANDROID_HOME)" >&2
  exit 1
fi
export ANDROID_HOME="$ANDROID_SDK"
export ANDROID_SDK_ROOT="$ANDROID_SDK"

NDK_DIR="${ANDROID_NDK_HOME:-$(ls -d "$ANDROID_SDK"/ndk/* 2>/dev/null | sort -V | tail -n 1)}"
if [ -z "${NDK_DIR:-}" ] || [ ! -d "$NDK_DIR" ]; then
  echo "Android NDK not found under $ANDROID_SDK/ndk" >&2
  exit 1
fi

HOST_TAG="linux-x86_64"
if [ "$(uname)" = "Darwin" ]; then
  HOST_TAG="darwin-x86_64"
fi
NDK_BIN="$NDK_DIR/toolchains/llvm/prebuilt/$HOST_TAG/bin"
API=24

echo "== Building Rust JNI core (NDK $(basename "$NDK_DIR")) =="
rustup target add aarch64-linux-android x86_64-linux-android >/dev/null 2>&1 || true

CC_aarch64_linux_android="$NDK_BIN/aarch64-linux-android${API}-clang" \
CXX_aarch64_linux_android="$NDK_BIN/aarch64-linux-android${API}-clang++" \
AR_aarch64_linux_android="$NDK_BIN/llvm-ar" \
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDK_BIN/aarch64-linux-android${API}-clang" \
cargo build --lib --release --target aarch64-linux-android

CC_x86_64_linux_android="$NDK_BIN/x86_64-linux-android${API}-clang" \
CXX_x86_64_linux_android="$NDK_BIN/x86_64-linux-android${API}-clang++" \
AR_x86_64_linux_android="$NDK_BIN/llvm-ar" \
CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$NDK_BIN/x86_64-linux-android${API}-clang" \
cargo build --lib --release --target x86_64-linux-android

mkdir -p android/app/src/main/jniLibs/arm64-v8a android/app/src/main/jniLibs/x86_64
cp -f target/aarch64-linux-android/release/libfomako_church_controller.so \
  android/app/src/main/jniLibs/arm64-v8a/libfomako_church_controller.so
cp -f target/x86_64-linux-android/release/libfomako_church_controller.so \
  android/app/src/main/jniLibs/x86_64/libfomako_church_controller.so

GRADLE_BIN=""
if [ -x "./android/gradlew" ]; then
  GRADLE_BIN="./gradlew"
elif command -v gradle >/dev/null 2>&1; then
  GRADLE_BIN="$(command -v gradle)"
else
  GRADLE_BIN="$(find "$HOME/.gradle/wrapper/dists" -type f -path '*/gradle-8.14*/bin/gradle' 2>/dev/null | head -n 1 || true)"
  if [ -z "$GRADLE_BIN" ]; then
    GRADLE_BIN="$(find "$HOME/.gradle/wrapper/dists" -type f -name gradle 2>/dev/null | head -n 1 || true)"
  fi
fi
if [ -z "$GRADLE_BIN" ]; then
  echo "gradlew or gradle not found (check ./android/gradlew or PATH)" >&2
  exit 1
fi

echo "== Building Android APK ($GRADLE_BIN) =="
(cd android && "$GRADLE_BIN" assembleDebug --no-daemon)

mkdir -p dist
cp -f android/app/build/outputs/apk/debug/app-debug.apk dist/fomako-church-controller-android.apk
ls -lh dist/fomako-church-controller-android.apk
echo "Install on Android phone: adb install -r dist/fomako-church-controller-android.apk"
