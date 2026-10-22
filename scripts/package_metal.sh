#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
[[ -d "$ROOT/Engines/gptk-wine/lib" && -d "$ROOT/Engines/d3dmetal-redist/external" ]] || {
  echo 'Supply the matched Wine and Metal drop-ins before repackaging.' >&2
  exit 1
}
TEMP_ARCHIVE="$(mktemp "$ROOT/Engines/metal-runtime.XXXXXX")"
trap 'rm -f "$TEMP_ARCHIVE"' EXIT
COPYFILE_DISABLE=1 tar -czf "$TEMP_ARCHIVE" -C "$ROOT/Engines" gptk-wine d3dmetal-redist
mv "$TEMP_ARCHIVE" "$ROOT/Engines/metal-runtime.tar.gz"
(cd "$ROOT/Engines" && shasum -a 256 metal-runtime.tar.gz > metal-runtime.sha256)
echo 'Runtime repackaged. Test the matching runtime, then commit the archive and checksum together.'
