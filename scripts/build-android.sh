#!/usr/bin/env bash
# APK Android ARM64 de PdfEditor (Tauri 2 mobile) : PDFium Android figé
# (empreinte vérifiée) placé dans les bibliothèques natives de l'APK, puis
# construction par la CLI Tauri (version figée dans apps/desktop/package.json).
#
# Prérequis : ANDROID_HOME (SDK avec platforms;android-37.0 et
# ndk;29.0.14206865), Java 17 ou plus, Node, cible Rust aarch64-linux-android.
# `build-android.sh` : APK de test (debug) ; `build-android.sh release` :
# APK non signé à signer ensuite (A-04).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
ndk_version="29.0.14206865"
: "${ANDROID_HOME:?ANDROID_HOME doit pointer vers le SDK Android}"
export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/$ndk_version}"

"$root/scripts/fetch-pdfium.sh" android
libs="$root/apps/desktop/gen/android/app/src/main/jniLibs/arm64-v8a"
mkdir -p "$libs"
cp "$root/vendor/pdfium-android/libpdfium.so" "$libs/"

mode=(--debug)
if [ "${1:-}" = "release" ]; then
  mode=()
fi
cd "$root/apps/desktop"
# CLI Tauri de package-lock.json : Gradle la rappelle (« npm run tauri »).
npm ci --no-audit --no-fund
npm run tauri -- android build "${mode[@]}" --target aarch64 --apk --ci
