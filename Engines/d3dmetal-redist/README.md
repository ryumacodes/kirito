# D3DMetal redist (licensed drop zone)

Place Apple Game Porting Toolkit **`redist/lib`** contents here under your redistribution rights.

Expected layout after copy:

```text
Engines/d3dmetal-redist/
  external/D3DMetal.framework/...
  external/libd3dshared.dylib
  wine/...
```

From a mounted GPTK evaluation DMG:

```bash
ditto "/Volumes/Evaluation environment for Windows games …/redist/lib/" \
  "Engines/d3dmetal-redist/"
```

The raw files are ignored; the packaged runtime is tracked through Git LFS. kirito copies them into the app resources at build time (when present) and installs them for end users on first run.

Also place a D3DMetal-capable Wine host in `Engines/gptk-wine/` (see that folder’s README).
