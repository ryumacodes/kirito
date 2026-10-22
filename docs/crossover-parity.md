# CrossOver Parity Notes

These notes track behaviors observed from a licensed local CrossOver install and map them to Whine features. They are a clean-room implementation checklist: mirror workflows and compatibility intent, use authorized upstream sources, and do not copy proprietary CodeWeavers code, assets, or recipe text.

## Steam Bottle Baseline

- Implemented: Steam installs into a dedicated Windows 10 64-bit bottle.
- Implemented: MSync is enabled by default for the Steam bottle, so Steam games inherit it from the shared prefix.
- Implemented: Steam setup and launches run with `WINE_WAIT_CHILD_PIPE_IGNORE=steam.exe`.
- Implemented: Steam dependency progress is emitted to the UI while installers download or run.
- Implemented: Whine-owned dependency recipes cover Steam installer, Wine Mono, Visual C++ v14 x86/x64, Source Han Sans CJK fonts, and Wine-bundled `msls31.dll` / `riched20.dll`.
- Implemented: Dependency downloads are allowlisted to authorized sources only.

## Steam Registry Prep

- Implemented: Disable the obsolete Wine OSS driver for `steam.exe`.
- Implemented: Add a `Lucida Console` font replacement to reduce old Wine font fallback issues.
- Implemented: Set `steamwebhelper.exe` to an app-specific Windows XP profile for the legacy web helper behavior seen in CrossOver recipes.
- Implemented: For CJK locales, disable Steam DirectWrite and map Meiryo fonts to Source Han Sans after installing the CJK font pack.
- Implemented: Prefer native then builtin RichEdit and Microsoft Line Services when the selected Wine runtime provides those DLLs.

## CrossOver Behaviors Not Copied Directly

- Not copied: CodeWeavers redirect URLs, private CrossTie metadata, icons, product assets, and proprietary patch code.
- Not copied: CrossOver-specific environment flags whose effect depends on CodeWeavers-patched Wine.
- Not copied yet: Legacy game-specific hacks for very old titles such as Half-Life 2 and Prey. These belong in a future quirks manifest keyed by executable name.

## Next Parity Targets

- Add a visible Steam bottle health view that shows installed dependency markers, selected engine, Windows profile, MSync, and missing runtime components.
- Add a first-class bottle actions panel for Run Command, Open C: Drive, Wine Configuration, Game Controllers, Task Manager, Internet Settings, and Simulate Reboot.
- Add a supported DirectX legacy component pack using authorized Microsoft sources or Wine-provided equivalents where licensing allows it.
- Add an app compatibility recipe format so Steam, Battle.net, GOG Galaxy, EA App, Ubisoft Connect, Rockstar Games Launcher, and unlisted installers share the same planner/progress UI.
- Add optional per-game quirks inherited from the Steam bottle but overrideable per app.

## Counter-Strike 2 (Steam 730)

- Implemented: High-confidence CS2 compatibility profile (D3DMetal on, DXVK off, MSync on, Retina off, VSync on).
- Implemented: Prefer CrossOver / Game Porting Toolkit / kirito-redistributed D3DMetal engines when D3DMetal is required; expose `d3dMetalCapable` on engine list.
- Implemented: Refuse CS2 launch without a D3DMetal-capable engine.
- Implemented: `VideoMemorySize` Direct3D registry string sized from Mac RAM (CrossOver community guidance).
- Implemented: CS2 launch via `steam.exe -applaunch 730 -nojoy -novid -fullscreen`.
- Implemented: When MSync is enabled, keep ESync off (CrossOver CS2 tip alignment).
- Implemented: Optional redistribute path — drop licensed GPTK Wine into `Engines/gptk-wine/` and Apple `redist/lib` into `Engines/d3dmetal-redist/`; kirito bundles them and installs to Application Support as `KiritoD3DMetal` on first run. Binaries stay gitignored; publisher supplies them at release under their Apple redistribution rights.
