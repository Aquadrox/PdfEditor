#!/usr/bin/env bash
# Télécharge la bibliothèque PDFium précompilée (bblanchon/pdfium-binaries),
# vérifie son empreinte SHA-256 et la place dans vendor/pdfium/.
#
# Utilisé par la CI et en développement (Linux, ou Windows avec Git Bash).
# La version doit correspondre à la fonction `pdfium_XXXX` choisie pour
# pdfium-render dans crates/pde-pdf/Cargo.toml.
#
# Changer de version : modifier PDFIUM_BUILD et les deux empreintes (calculées
# avec `sha256sum` sur les archives téléchargées), puis la fonction de
# pdfium-render.
set -euo pipefail

PDFIUM_BUILD=8125
SHA256_LINUX_X64=e1c3ce6e270cb5e0c20816b379ad9f2c1309972697c505f508de92bf5e7d6659
SHA256_WIN_X64=9a964261760cff52570aabf038ab346d3a5034dd8b30d0d5fe8861235d356b8b

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$root/vendor/pdfium"

case "$(uname -s)" in
  Linux)
    archive=pdfium-linux-x64.tgz
    expected=$SHA256_LINUX_X64
    library=lib/libpdfium.so
    ;;
  MINGW* | MSYS* | CYGWIN*)
    archive=pdfium-win-x64.tgz
    expected=$SHA256_WIN_X64
    library=bin/pdfium.dll
    ;;
  *)
    echo "plateforme non prise en charge : $(uname -s)" >&2
    exit 1
    ;;
esac

stamp="$dest/VERSION-$PDFIUM_BUILD"
if [ -f "$stamp" ]; then
  echo "PDFium $PDFIUM_BUILD déjà présent dans $dest"
  exit 0
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

url="https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F$PDFIUM_BUILD/$archive"
echo "Téléchargement de $url"
curl --fail --silent --show-error --location --retry 3 --output "$work/$archive" "$url"

actual=$(sha256sum "$work/$archive" | cut -d' ' -f1)
if [ "$actual" != "$expected" ]; then
  echo "empreinte SHA-256 inattendue pour $archive : $actual (attendu $expected)" >&2
  exit 1
fi

mkdir -p "$work/x"
tar -xzf "$work/$archive" -C "$work/x"
rm -rf "$dest"
mkdir -p "$dest"
cp "$work/x/$library" "$dest/"
cp "$work/x/LICENSE" "$dest/LICENSE-pdfium-binaries"
cp -r "$work/x/licenses" "$dest/licenses" 2>/dev/null || true
touch "$stamp"
echo "PDFium $PDFIUM_BUILD installé dans $dest"
