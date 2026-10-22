#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNTIME="${1:-$ROOT/Engines/gptk-wine}"
RUNTIME="$(cd "$RUNTIME" && pwd)"
METAL_RUNTIME="${2:-}"
if [[ -x "$RUNTIME/dist/bin/wine" ]]; then
  METAL_RUNTIME="${METAL_RUNTIME:-$RUNTIME/d3dmetal/gptk}"
  RUNTIME="$RUNTIME/dist"
fi
WINE="$RUNTIME/bin/wine64"
[[ -x "$WINE" ]] || WINE="$RUNTIME/bin/wine"
TEST_DIR="$(mktemp -d "${TMPDIR:-/tmp}/kirito-metal-test.XXXXXX")"
RUST_HOST="$(rustc -vV | sed -n 's/^host: //p')"
LINKER="$(rustc --print sysroot)/lib/rustlib/$RUST_HOST/bin/rust-lld"
cleanup() {
  WINEPREFIX="$TEST_DIR/prefix" "$RUNTIME/bin/wineserver" -k 2>/dev/null || true
  rm -rf "$TEST_DIR"
}
trap cleanup EXIT
cat > "$TEST_DIR/kernel32.def" <<'DEF'
LIBRARY kernel32.dll
EXPORTS
LoadLibraryA
GetProcAddress
GetStdHandle
WriteFile
ExitProcess
DEF
"$LINKER" -flavor link /lib "/def:$TEST_DIR/kernel32.def" /machine:x64 "/out:$TEST_DIR/kernel32.lib"
clang --target=x86_64-pc-windows-msvc -fno-stack-protector -c "$ROOT/scripts/metal_probe.c" -o "$TEST_DIR/probe.obj"
"$LINKER" -flavor link /entry:mainCRTStartup /subsystem:console /nodefaultlib "/out:$TEST_DIR/probe.exe" "$TEST_DIR/probe.obj" "$TEST_DIR/kernel32.lib"
if [[ -n "$METAL_RUNTIME" ]]; then
  METAL_RUNTIME="$(cd "$METAL_RUNTIME" && pwd)"
  # WineForge selects Apple's libraries through its runtime loader. Do not
  # force the older GPTK host's DLL search path onto this host.
  WINEPREFIX="$TEST_DIR/prefix" WINEDEBUG=-all \
    GRAPHICS_BACKEND=d3dmetal D3DMETAL_RUNTIME_DIR="$METAL_RUNTIME" \
    WINEDLLOVERRIDES='mscoree,mshtml=' \
    "$WINE" "$TEST_DIR/probe.exe"
else
WINEPREFIX="$TEST_DIR/prefix" WINEDEBUG=-all \
  WINEDLLOVERRIDES='d3d10,d3d10core,d3d11,d3d12,dxgi=b;mscoree,mshtml=' \
  WINEDLLPATH="$RUNTIME/lib/wine" \
  "$WINE" "$TEST_DIR/probe.exe"
fi
