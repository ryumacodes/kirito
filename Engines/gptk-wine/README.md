# GPTK / D3DMetal Wine host (licensed drop zone)

Place a **D3DMetal-capable Wine** tree here (Game Porting Toolkit wine, or an equivalent host you are allowed to redistribute).

Expected layout:

```text
Engines/gptk-wine/
  bin/wine
  bin/wine64          # optional
  lib/...
```

kirito will merge `../d3dmetal-redist` into this runtime’s `lib` on first install (CrossOver-style: wine host + Apple D3DMetal libs).

Raw files remain ignored. The tested runtime archive is tracked through Git LFS under the publisher’s stated redistribution permission.

## Verified local pairing

GCenX Game Porting Toolkit 3.0-3 (Wine 7.7 / GPTK 1.1 host) with its matching
Apple D3DMetal 3.0 libraries passed `scripts/test_metal.sh` on this Mac.
Archive SHA-256: `d377683937340f914823dbb2e1252b329cbf834ff58907d0293db8cebf0e392e`.
Source: https://github.com/Gcenx/game-porting-toolkit/releases/tag/Game-Porting-Toolkit-3.0-3

Apple GPTK 4 beta 2's PE bridges require a newer Wine Unix-call interface and
cannot simply replace this host's DLLs. Validate a new host/library pairing with
the device-creation smoke test before replacing these drop-ins.

The build script archives both drop-in trees before Tauri bundles them, preserving
symlinks and permissions. The installer validates a staging copy before replacing
the installed runtime. The generated archive is tracked through Git LFS and verified at build time.
