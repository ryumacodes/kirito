# kirito Mac Feature Parity Plan

As of June 11, 2026, WineHQ lists Wine 11.0 as stable and Wine 11.2 as the latest development release. kirito targets Wine 11.0 plus Wine Mono 10.4.1, vkd3d 1.18, D3DMetal 3.0, and DXMT v0.72 as its compatibility baseline.

## Open-Source Engine Baseline

- Wine 11.0 stable is the default kirito target.
- Wine 11.2 development should be offered as an opt-in engine channel.
- Wine Mono and Gecko should be downloadable per engine.
- DXVK, VKD3D-Proton, MoltenVK, and DXMT should be managed as separately versioned components.
- D3DMetal requires Apple's Game Porting Toolkit license path and cannot simply be redistributed as a Wine feature.

## Polished Mac Features

- Bottle library with isolated `WINEPREFIX` folders.
- Install `.exe` and `.msi` into selected bottles.
- Auto-generated app launchers from Start Menu entries.
- Per-bottle toggles for D3DMetal, DXVK, VKD3D, MSync/ESync, Retina mode, Windows version, environment variables, and command-line overrides.
- Bottle tools for `winecfg`, `regedit`, `taskmgr`, command prompt, file explorer, uninstallers, and logs.
- Engine manager for stable, development, and custom Wine builds.
- Compatibility recipes for common apps and game launchers.
- Backup, duplicate, repair, and export bottles.
- macOS integration for Dock launchers, file associations, controller guidance, microphone/camera/privacy prompts, and app-specific icons.

## Known Gaps

- Some proprietary polish, paid support, curated recipes, and vendor patches are not always upstream.
- 32-bit Windows apps on modern macOS and Apple silicon are the hardest compatibility target. Wine32-on-64 and newer ARM translation work require deep toolchain integration.
- D3DMetal and DXMT are separate graphics translation technologies; redistribution and versioning must be handled carefully.
- Anti-cheat, DRM, kernel drivers, and Microsoft Store/UWP apps remain non-goals unless upstream compatibility changes.

## Build Milestones

1. Native SwiftUI shell with bottle creation, install/run flows, engine detection, and logs.
2. Engine bootstrapper for Wine stable/development, Wine Mono, Gecko, DXVK, VKD3D-Proton, MoltenVK, and DXMT.
3. Start Menu scanner that turns installed apps into kirito launchers.
4. Recipe format for repeatable installs and per-app settings.
5. Signed `.app` packaging with bundled engine and first-run diagnostics.
