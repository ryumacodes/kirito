# kirito

kirito is a lightweight macOS app for running Windows apps with Wine.

It exists because using pure Wine directly can feel rough: terminal commands, hidden prefixes, scattered settings, and very little guidance for normal users. kirito wraps the Wine workflow in a fast, simple interface with app discovery, Steam support, per-bottle settings, compatibility info, and a library of installed apps.

## What kirito does

- Creates and manages isolated Wine bottles.
- Detects available Wine engines on macOS.
- Installs and launches Steam.
- Searches Steam and opens Steam game installs through the Steam client.
- Detects installed Steam games and shows them in kirito's Library.
- Shows compatibility signals from ProtonDB for Steam games.
- Uses WineHQ AppDB as the compatibility source for general Windows apps.
- Opens `.exe` and `.msi` installers through the native macOS file picker.
- Provides common Wine tools like Wine Config, Registry, Task Manager, and Command Prompt.
- Exposes practical game settings such as VSync and safer Steam graphics options.

## One-command install

For local builds, run:

```bash
./scripts/install.sh
```

This installs dependencies, builds kirito, copies `kirito.app` into `~/Applications`, and opens it.

The intended public install flow is the same shape: one command that downloads/builds or installs kirito, then leaves a normal macOS app in Applications.

## Pinned Steam game shortcuts

kirito can be launched with a Steam app id:

```bash
~/Applications/kirito.app/Contents/MacOS/kirito --steam-app 2094530
```

That opens kirito, launches Steam in the right bottle, and asks Steam to run that game.

To create a small macOS app shortcut for a Steam game:

```bash
./scripts/create_steam_shortcut.sh 2094530 "AdventureQuest Worlds Infinity Playtest"
```

The shortcut is created in `~/Applications` and can be pinned like a normal app. When opened, it starts kirito, Steam, and then the game.

## Install safety

kirito should not download random installers from search results.

The intended trust model is:

- Steam games install through Steam.
- Steam compatibility comes from ProtonDB.
- General Windows app compatibility comes from WineHQ AppDB/community reports.
- Apps without a verified source use a manual installer picker.
- Future verified downloads should come from trusted manifests with publisher and hash data.

## Why Tauri and Rust

kirito is built with Tauri and Rust so the app can stay small, fast, and native-feeling on macOS. The frontend stays simple, while the Rust backend handles filesystem work, process launching, Wine integration, Steam scanning, and settings.

## Run in development

```bash
pnpm install
pnpm run tauri:dev
```

## Build

```bash
pnpm run tauri:build
```

The macOS app bundle is produced under Tauri's release bundle directory.

## Current status

kirito is an early prototype. Steam install/launch, Steam game detection, compatibility display, bottle creation, Wine tool launching, and the core macOS UI are in progress. The goal is a friendly Windows-app launcher for Mac users who want Wine's flexibility without needing to manage Wine by hand.

## Metal runtime verification

The tested runtime is included as `Engines/metal-runtime.tar.gz` through Git LFS.
Before building a fresh checkout, run:

```bash
git lfs install
git lfs pull
pnpm install --frozen-lockfile
pnpm run tauri:build
```

The build verifies `Engines/metal-runtime.sha256` and fails clearly if the archive
is missing, corrupted, or still an LFS pointer. GitHub source ZIP downloads do not
replace an LFS checkout. The built Mac app includes the runtime for end users;
they do not need Git LFS or separate Metal downloads.

Maintainers can replace the matched host and libraries in `Engines/gptk-wine`
and `Engines/d3dmetal-redist`, test them, and run `scripts/package_metal.sh`.
Commit the archive and checksum together. Redistribution is enabled at the
publisher's explicit direction under their stated permission.
On first launch Kirito installs it as **D3DMetal (kirito)**. Enable D3DMetal in the
bottle or app settings. Quit an already-running Steam client before switching engines.

Run `./scripts/test_metal.sh` to compile and run a small DirectX 11/12 hardware-device
test in a temporary bottle. Xcode command-line tools and the Rust toolchain are
required. A runtime path may be passed as the first argument to test the installed
copy. Both device checks must pass; this is not a guarantee of game compatibility.

## Experimental Minecraft Bedrock (PC license)

The Install catalog provides **Minecraft Bedrock → Set up Bedrock** for an owned
Minecraft for Windows purchase. Setup uses Xodus Microsoft sign-in and a separate
GDK Wine runtime and bottle; Android/Google Play ownership is not required.
The runtime is pinned to [community launcher beta 0.1.0b](https://github.com/amha0270/Minecraft-Bedrock-GDK-Launcher/releases/tag/BETA)
and verified against SHA-256 `10165c17c992547b95dd873256a9c9fb4b7ed1c590c87b1c1366bc2d17be74d1`.
Its provider has not published all modified source code. This integration remains
experimental: successful graphics-device checks do not establish gameplay compatibility.

Game files and the runtime live in Kirito's application-support directory. Setup
registers the game in Library only after the licensed executable and bottle are
prepared. Launch output is in `Logs/minecraft-bedrock.log` within that directory.
Microsoft account authentication and an actual game launch are required for
end-to-end verification; Kirito does not import Java-launcher credentials.

GPTK 4 beta requires the Bedrock runtime's WineForge host; the earlier Wine 7.7
and stock Wine 11 tests failed with these libraries. To test this pairing in a
fresh temporary bottle:

```bash
./scripts/test_metal.sh "$HOME/Library/Application Support/app.kirito.desktop/Engines/BedrockGDK"
```

An optional second argument selects an extracted Apple `redist/lib` directory.
On this Mac, Apple's GPTK 4 beta libraries passed both D3D11 and D3D12 device
creation with the Bedrock host. A non-fatal Apple Neural Engine warning was also
reported; gameplay and MetalFX remain unverified.
