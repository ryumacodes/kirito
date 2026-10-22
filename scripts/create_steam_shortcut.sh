#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: scripts/create_steam_shortcut.sh <steam-app-id> <app-name>"
  echo "Example: scripts/create_steam_shortcut.sh 2094530 \"AdventureQuest Worlds Infinity Playtest\""
  exit 1
fi

STEAM_APP_ID="$1"
APP_NAME="$2"
KIRITO_APP="${KIRITO_APP:-$HOME/Applications/kirito.app}"
DEST_DIR="${KIRITO_SHORTCUT_DIR:-$HOME/Applications}"
SAFE_NAME="$(printf '%s' "$APP_NAME" | tr '/:' '--')"
SHORTCUT_APP="$DEST_DIR/$SAFE_NAME.app"
MACOS_DIR="$SHORTCUT_APP/Contents/MacOS"

if [[ ! -d "$KIRITO_APP" ]]; then
  echo "kirito was not found at:"
  echo "  $KIRITO_APP"
  echo "Install kirito first with scripts/install.sh, or set KIRITO_APP=/path/to/kirito.app"
  exit 1
fi

mkdir -p "$MACOS_DIR"

cat > "$SHORTCUT_APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>launch</string>
  <key>CFBundleIdentifier</key>
  <string>app.kirito.shortcut.$STEAM_APP_ID</string>
  <key>CFBundleName</key>
  <string>$APP_NAME</string>
  <key>CFBundleDisplayName</key>
  <string>$APP_NAME</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
</dict>
</plist>
PLIST

cat > "$MACOS_DIR/launch" <<LAUNCH
#!/usr/bin/env bash
exec "$KIRITO_APP/Contents/MacOS/kirito" --steam-app "$STEAM_APP_ID"
LAUNCH

chmod +x "$MACOS_DIR/launch"

echo "Created shortcut:"
echo "  $SHORTCUT_APP"
