#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_NAME="kirito.app"
DEST_DIR="${KIRITO_INSTALL_DIR:-$HOME/Applications}"
DEST_APP="$DEST_DIR/$APP_NAME"

cd "$ROOT_DIR"

if ! command -v pnpm >/dev/null 2>&1; then
  echo "pnpm is required. Install it first, then run this again."
  echo "Recommended: corepack enable && corepack prepare pnpm@latest --activate"
  exit 1
fi

echo "Installing dependencies..."
pnpm install

echo "Building kirito..."
pnpm run tauri:build

mkdir -p "$DEST_DIR"
rm -rf "$DEST_APP"
cp -R "$ROOT_DIR/src-tauri/target/release/bundle/macos/$APP_NAME" "$DEST_APP"

echo "kirito installed at:"
echo "  $DEST_APP"
echo
echo "Opening kirito..."
open "$DEST_APP"
