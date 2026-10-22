#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENGINE_DIR="$ROOT_DIR/Engines"
WINE_VERSION="${WINE_VERSION:-11.0}"
ARCHIVE="wine-${WINE_VERSION}.tar.xz"
if [[ "$WINE_VERSION" =~ ^[0-9]+\.0$ ]]; then
  SOURCE_CHANNEL="$WINE_VERSION"
else
  SOURCE_CHANNEL="${WINE_VERSION%%.*}.x"
fi
URL="https://dl.winehq.org/wine/source/${SOURCE_CHANNEL}/${ARCHIVE}"

mkdir -p "$ENGINE_DIR"

echo "Fetching Wine ${WINE_VERSION} from ${URL}"
curl -L --fail --progress-bar "$URL" -o "$ENGINE_DIR/$ARCHIVE"

echo "Archive saved to $ENGINE_DIR/$ARCHIVE"
echo
echo "Building Wine on macOS is intentionally separate because it requires Xcode CLT,"
echo "freetype, gnutls, molten-vk/vulkan headers, and optional 32-bit/ARM translation work."
echo "For immediate local testing, install a Wine binary with Homebrew:"
echo "  brew install --cask wine-stable"
echo
echo "Kirito auto-installs Wine Stable into Application Support when no engine is found."
echo "Manual fallback locations:"
echo "  $ENGINE_DIR/wine-${WINE_VERSION}/bin/wine"
echo "  ~/Library/Application Support/app.kirito.desktop/Engines/Wine Stable.app"
echo "  /opt/homebrew/bin/wine"
echo "  /usr/local/bin/wine"
echo "  /Applications/Wine Stable.app/Contents/Resources/wine/bin/wine"
