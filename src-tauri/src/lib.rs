mod bedrock;
mod steam_logs;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    env,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime},
};
use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri::{Emitter, Manager};
use uuid::Uuid;

static STEAM_EXECUTABLE_CACHE: OnceLock<Mutex<HashMap<PathBuf, Vec<String>>>> = OnceLock::new();
static PROCESS_LIST_CACHE: OnceLock<Mutex<Option<CachedProcessList>>> = OnceLock::new();
static STEAM_LIBRARY_CACHE: OnceLock<Mutex<Option<CachedSteamLibrary>>> = OnceLock::new();
static ANTI_CHEAT_CACHE: OnceLock<Mutex<Option<CachedAntiCheatReports>>> = OnceLock::new();
static CONTROLLER_CACHE: OnceLock<Mutex<Option<CachedControllerDiagnostics>>> = OnceLock::new();
static ENGINE_ENSURE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
const UNTIL_DAWN_STEAM_APP_ID: u64 = 217_2010;
const CS2_STEAM_APP_ID: u64 = 730;
const PROCESS_LIST_TTL: Duration = Duration::from_millis(900);
const ANTI_CHEAT_TTL: Duration = Duration::from_secs(60 * 60 * 24);
const CONTROLLER_TTL: Duration = Duration::from_secs(20);
const WINE_STABLE_VERSION: &str = "11.0_1";
const WINE_STABLE_ARCHIVE: &str = "wine-stable-11.0_1-osx64.tar.xz";
const WINE_STABLE_URL: &str =
    "https://github.com/Gcenx/macOS_Wine_builds/releases/download/11.0_1/wine-stable-11.0_1-osx64.tar.xz";
const WINE_STABLE_SHA256: &str = "b50dc50ec7f41d58b115a6b685d4d1315ba3c797bd3aa0f49213f2703cb82388";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BottleSettings {
    d3d_metal: bool,
    dxvk: bool,
    msync: bool,
    retina: bool,
    #[serde(default = "default_true")]
    vsync: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    engine_executable: Option<PathBuf>,
    d3d_metal: Option<bool>,
    dxvk: Option<bool>,
    msync: Option<bool>,
    retina: Option<bool>,
    vsync: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InstalledApp {
    id: Uuid,
    name: String,
    command: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Bottle {
    id: Uuid,
    name: String,
    path: PathBuf,
    windows_version: String,
    settings: BottleSettings,
    #[serde(default)]
    app_settings: HashMap<String, AppSettings>,
    apps: Vec<InstalledApp>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WineEngine {
    display_name: String,
    executable: PathBuf,
    d3d_metal_capable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamStatus {
    installed: bool,
    running: bool,
    bottle_id: Option<Uuid>,
    bottle_path: Option<PathBuf>,
    games: Vec<InstalledSteamGame>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamGame {
    app_id: u64,
    name: String,
    image: Option<String>,
    price: Option<String>,
    store_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InstalledSteamGame {
    app_id: u64,
    name: String,
    install_dir: Option<String>,
    image: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProtonRating {
    app_id: u64,
    tier: String,
    confidence: Option<String>,
    score: Option<f64>,
    total: Option<u64>,
    source_url: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AntiCheatReport {
    matched_name: String,
    name: String,
    status: String,
    anti_cheats: Vec<String>,
    notes: Option<String>,
    updated: Option<String>,
    source_url: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ControllerDevice {
    name: String,
    transport: String,
    vendor: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ControllerDiagnostics {
    checked_at: u64,
    devices: Vec<ControllerDevice>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatus {
    steam_running: bool,
    app_ids: Vec<Uuid>,
    steam_app_ids: Vec<u64>,
}

#[derive(Clone)]
struct CachedProcessList {
    captured_at: Instant,
    processes: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
struct SteamLibrarySignature {
    steamapps_dirs: Vec<PathBuf>,
    manifests: Vec<(PathBuf, Option<SystemTime>)>,
}

#[derive(Clone)]
struct CachedSteamLibrary {
    bottle_path: PathBuf,
    signature: SteamLibrarySignature,
    games: Vec<InstalledSteamGame>,
}

#[derive(Clone)]
struct CachedAntiCheatReports {
    captured_at: Instant,
    reports: Vec<AntiCheatReport>,
}

#[derive(Clone)]
struct CachedControllerDiagnostics {
    captured_at: Instant,
    diagnostics: ControllerDiagnostics,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstallProgress {
    app_name: String,
    message: String,
    step_index: Option<usize>,
    step_total: Option<usize>,
    percent: Option<u8>,
}

#[derive(Clone, Copy)]
enum DependencyAction {
    Exe { args: &'static [&'static str] },
    Msi { args: &'static [&'static str] },
    FontZip,
    WineDlls { dlls: &'static [&'static str] },
}

struct DependencyRecipe {
    id: &'static str,
    name: &'static str,
    source: &'static str,
    url: Option<&'static str>,
    file_name: Option<&'static str>,
    sha256: Option<&'static str>,
    action: DependencyAction,
}

#[tauri::command]
fn list_bottles(app: tauri::AppHandle) -> Result<Vec<Bottle>, String> {
    let bottles_dir = bottles_dir(&app)?;
    ensure_dir(&bottles_dir)?;

    let mut bottles = fs::read_dir(&bottles_dir)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("whine.json"))
        .filter(|path| path.exists())
        .filter_map(|path| read_bottle_at(&path).ok())
        .collect::<Vec<_>>();

    bottles.sort_by_key(|bottle| bottle.name.to_lowercase());
    Ok(bottles)
}

#[tauri::command]
fn runtime_status(
    app: tauri::AppHandle,
    known_steam_games: Option<Vec<InstalledSteamGame>>,
) -> Result<RuntimeStatus, String> {
    let process_list = process_list();
    let steam_running = steam_is_running_in(&process_list);
    let mut app_ids = HashSet::new();
    let mut steam_app_ids = HashSet::new();
    let known_steam_games = known_steam_games.as_deref();

    for bottle in list_bottles(app)? {
        for installed_app in &bottle.apps {
            if app_is_running(installed_app, &process_list) {
                app_ids.insert(installed_app.id);
            }
        }

        if bottle.name.eq_ignore_ascii_case("steam") {
            let scanned_games;
            let games = if let Some(games) = known_steam_games {
                games
            } else {
                scanned_games = scan_steam_games(&bottle);
                &scanned_games
            };

            for game in games {
                if steam_game_is_running(&bottle, game, &process_list) {
                    steam_app_ids.insert(game.app_id);
                }
            }
        }
    }

    let mut app_ids = app_ids.into_iter().collect::<Vec<_>>();
    app_ids.sort();
    let mut steam_app_ids = steam_app_ids.into_iter().collect::<Vec<_>>();
    steam_app_ids.sort();

    Ok(RuntimeStatus {
        steam_running,
        app_ids,
        steam_app_ids,
    })
}

#[tauri::command]
fn list_engines(app: tauri::AppHandle) -> Result<Vec<WineEngine>, String> {
    Ok(detect_engines(&app)?)
}

#[tauri::command]
fn ensure_engine(app: tauri::AppHandle) -> Result<Vec<WineEngine>, String> {
    let lock = ENGINE_ENSURE_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock
        .lock()
        .map_err(|_| "Wine engine install is already in progress.".to_string())?;

    let existing = detect_engines(&app)?;
    if existing.iter().any(|engine| engine.d3d_metal_capable) {
        return Ok(existing);
    }

    // Prefer redistributed GPTK + D3DMetal when the publisher supplied drop-ins.
    let _ = install_bundled_d3dmetal_runtime(&app);
    if detect_engines(&app)?
        .iter()
        .any(|engine| engine.d3d_metal_capable)
    {
        return detect_engines(&app);
    }

    install_bundled_wine_engine(&app)?;
    let engines = detect_engines(&app)?;
    if engines.is_empty() {
        Err("Wine engine installed, but kirito could not find wine. Reopen kirito or install Wine Stable manually.".to_string())
    } else {
        Ok(engines)
    }
}

fn detect_engines(app: &tauri::AppHandle) -> Result<Vec<WineEngine>, String> {
    let root = app_root(app)?;
    let mut candidates = vec![
        managed_d3dmetal_wine_executable(app)?,
        managed_wine_executable(app)?,
        root.join("Engines/wine-11.0/bin/wine"),
        PathBuf::from("/opt/homebrew/bin/wine"),
        PathBuf::from("/usr/local/bin/wine"),
        PathBuf::from("/Applications/Wine Stable.app/Contents/Resources/wine/bin/wine"),
    ];

    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(home.join("Applications/Wine Stable.app/Contents/Resources/wine/bin/wine"));
    }

    add_crossover_engines(&mut candidates, Path::new("/Applications"));
    add_app_bundle_engines(&mut candidates, Path::new("/Applications"));

    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        add_crossover_engines(&mut candidates, &home.join("Applications"));
        add_app_bundle_engines(&mut candidates, &home.join("Applications"));
        add_gptk_engines(&mut candidates, &home);
    }

    add_gptk_engines(&mut candidates, Path::new("/usr/local"));
    add_gptk_engines(&mut candidates, Path::new("/opt/homebrew"));

    if let Ok(wine) = env::var("WINE") {
        candidates.insert(0, PathBuf::from(wine));
    }

    let mut engines = Vec::new();
    for path in candidates {
        if is_executable(&path)
            && !engines
                .iter()
                .any(|engine: &WineEngine| engine.executable == path)
        {
            engines.push(WineEngine {
                display_name: engine_name(&path),
                d3d_metal_capable: engine_supports_d3dmetal(&path),
                executable: path,
            });
        }
    }

    engines.sort_by(|left, right| {
        right
            .d3d_metal_capable
            .cmp(&left.d3d_metal_capable)
            .then_with(|| left.display_name.cmp(&right.display_name))
    });

    Ok(engines)
}

fn install_bundled_wine_engine(app: &tauri::AppHandle) -> Result<(), String> {
    ensure_dir(&engines_dir(app)?)?;
    ensure_rosetta_best_effort();

    let archive = engines_dir(app)?.join(WINE_STABLE_ARCHIVE);
    let wine_app = managed_wine_app(app)?;
    let total_steps = 3usize;

    if !is_executable(&managed_wine_executable(app)?) {
        if !archive.exists() {
            emit_install_progress(
                app,
                "Wine",
                &format!("Downloading Wine Stable {WINE_STABLE_VERSION}…"),
                Some(1),
                Some(total_steps),
                Some(5),
            );
            ensure_authorized_engine_url(WINE_STABLE_URL)?;
            download_file(WINE_STABLE_URL, &archive)
                .map_err(|error| format!("Could not download Wine Stable: {error}"))?;
        } else {
            emit_install_progress(
                app,
                "Wine",
                "Using cached Wine Stable archive…",
                Some(1),
                Some(total_steps),
                Some(20),
            );
        }

        emit_install_progress(
            app,
            "Wine",
            "Verifying Wine Stable archive…",
            Some(2),
            Some(total_steps),
            Some(55),
        );
        verify_sha256(&archive, WINE_STABLE_SHA256)?;

        emit_install_progress(
            app,
            "Wine",
            "Installing Wine Stable engine…",
            Some(3),
            Some(total_steps),
            Some(75),
        );
        extract_wine_stable_archive(app, &archive, &wine_app)?;
        clear_quarantine(&wine_app);
    }

    if !is_executable(&managed_wine_executable(app)?) {
        return Err(
            "Wine Stable archive extracted, but wine was not found inside Wine Stable.app."
                .to_string(),
        );
    }

    emit_install_progress(
        app,
        "Wine",
        "Wine engine ready",
        Some(total_steps),
        Some(total_steps),
        Some(100),
    );
    Ok(())
}

fn extract_wine_stable_archive(
    app: &tauri::AppHandle,
    archive: &Path,
    wine_app: &Path,
) -> Result<(), String> {
    let staging = engines_dir(app)?.join(format!("wine-stable-{WINE_STABLE_VERSION}-extract"));
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    ensure_dir(&staging)?;

    let status = Command::new("tar")
        .args(["-xJf"])
        .arg(archive)
        .arg("-C")
        .arg(&staging)
        .status()
        .map_err(|error| format!("Could not extract Wine Stable: {error}"))?;

    if !status.success() {
        let _ = fs::remove_dir_all(&staging);
        return Err("Could not extract Wine Stable archive.".to_string());
    }

    let extracted_app = find_extracted_wine_app(&staging).ok_or_else(|| {
        let _ = fs::remove_dir_all(&staging);
        "Wine Stable.app was missing from the downloaded archive.".to_string()
    })?;

    if wine_app.exists() {
        let _ = fs::remove_dir_all(wine_app);
    }
    fs::rename(&extracted_app, wine_app).or_else(|_| {
        copy_dir_recursive(&extracted_app, wine_app)?;
        let _ = fs::remove_dir_all(&extracted_app);
        Ok::<(), String>(())
    })?;

    let _ = fs::remove_dir_all(&staging);
    Ok(())
}

fn find_extracted_wine_app(staging: &Path) -> Option<PathBuf> {
    let direct = staging.join("Wine Stable.app");
    if direct.is_dir() {
        return Some(direct);
    }

    let entries = fs::read_dir(staging).ok()?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("Wine Stable.app"))
        {
            return Some(path);
        }
        if path.is_dir() {
            let nested = path.join("Wine Stable.app");
            if nested.is_dir() {
                return Some(nested);
            }
        }
    }
    None
}

fn clear_quarantine(path: &Path) {
    let _ = Command::new("xattr").args(["-cr"]).arg(path).status();
}

fn ensure_rosetta_best_effort() {
    let check = Command::new("/usr/bin/arch")
        .args(["-x86_64", "/usr/bin/true"])
        .status();
    if check.map(|status| status.success()).unwrap_or(false) {
        return;
    }

    let _ = Command::new("/usr/sbin/softwareupdate")
        .args(["--install-rosetta", "--agree-to-license"])
        .status();
}

fn ensure_authorized_engine_url(url: &str) -> Result<(), String> {
    let allowed = [
        "https://github.com/Gcenx/macOS_Wine_builds/",
        "https://github.com/Gcenx/wine-official/",
    ];
    if allowed.iter().any(|prefix| url.starts_with(prefix)) {
        Ok(())
    } else {
        Err(format!("Refusing unauthorized Wine engine source: {url}"))
    }
}

fn managed_wine_app(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(engines_dir(app)?.join("Wine Stable.app"))
}

fn managed_wine_executable(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(managed_wine_app(app)?.join("Contents/Resources/wine/bin/wine"))
}

fn managed_d3dmetal_runtime_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(engines_dir(app)?.join("KiritoD3DMetal"))
}

fn managed_d3dmetal_wine_executable(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let root = managed_d3dmetal_runtime_dir(app)?;
    Ok([
        root.join("bin/wine64"),
        root.join("bin/wine"),
        root.join("wine"),
    ]
    .into_iter()
    .find(|path| is_executable(path))
    .unwrap_or_else(|| root.join("bin/wine64")))
}

fn install_bundled_d3dmetal_runtime(app: &tauri::AppHandle) -> Result<(), String> {
    let destination = managed_d3dmetal_runtime_dir(app)?;
    if engine_supports_d3dmetal(&managed_d3dmetal_wine_executable(app)?) {
        return Ok(());
    }
    let staging = engines_dir(app)?.join(format!(".metal-install-{}", Uuid::new_v4()));
    ensure_dir(&staging)?;
    let result = (|| {
        let archive = app
            .path()
            .resource_dir()
            .ok()
            .map(|root| root.join("metal-runtime.tar.gz"))
            .filter(|path| path.is_file());
        let (wine_source, redist_source) = if let Some(archive) = archive {
            let status = Command::new("/usr/bin/tar")
                .arg("-xzf")
                .arg(archive)
                .arg("-C")
                .arg(&staging)
                .status()
                .map_err(|error| error.to_string())?;
            if !status.success() {
                return Err("Could not unpack the bundled Metal runtime.".to_string());
            }
            (staging.join("gptk-wine"), staging.join("d3dmetal-redist"))
        } else {
            (
                find_bundled_gptk_wine_source(app)?.ok_or(
                    "No bundled Metal Wine host. Supply Engines/gptk-wine before building.",
                )?,
                find_bundled_d3dmetal_redist_source(app)?.ok_or(
                    "No bundled Apple libraries. Supply Engines/d3dmetal-redist before building.",
                )?,
            )
        };
        emit_install_progress(
            app,
            "D3DMetal",
            "Installing D3DMetal runtime…",
            Some(1),
            Some(2),
            Some(20),
        );
        let runtime = staging.join("runtime");
        copy_dir_recursive(&wine_source, &runtime)?;
        copy_dir_recursive(&redist_source, &runtime.join("lib"))?;
        let wine = [
            runtime.join("bin/wine64"),
            runtime.join("bin/wine"),
            runtime.join("wine"),
        ]
        .into_iter()
        .find(|path| engine_supports_d3dmetal(path))
        .ok_or(
            "Bundled Metal runtime is incomplete: Wine or Apple Direct3D bridges are missing.",
        )?;
        let status = Command::new(&wine)
            .arg("--version")
            .output()
            .map_err(|error| error.to_string())?;
        if !status.status.success() {
            return Err("Bundled Metal Wine host could not start.".to_string());
        }
        clear_quarantine(&runtime);
        let previous = staging.join("previous");
        if destination.exists() {
            fs::rename(&destination, &previous).map_err(|error| error.to_string())?;
        }
        if let Err(error) = fs::rename(&runtime, &destination) {
            if previous.exists() {
                let _ = fs::rename(&previous, &destination);
            }
            return Err(error.to_string());
        }
        emit_install_progress(
            app,
            "D3DMetal",
            "D3DMetal runtime ready",
            Some(2),
            Some(2),
            Some(100),
        );
        Ok(())
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}

fn find_bundled_gptk_wine_source(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    for candidate in bundled_engine_dropin_candidates(app, "gptk-wine")? {
        if candidate.join("bin/wine64").is_file()
            || candidate.join("bin/wine").is_file()
            || candidate.join("wine").is_file()
        {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn find_bundled_d3dmetal_redist_source(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    for candidate in bundled_engine_dropin_candidates(app, "d3dmetal-redist")? {
        if candidate.join("external/D3DMetal.framework").exists()
            || candidate.join("external/libd3dshared.dylib").exists()
            || candidate.join("D3DMetal.framework").exists()
            || candidate.join("libd3dshared.dylib").exists()
        {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn bundled_engine_dropin_candidates(
    app: &tauri::AppHandle,
    folder: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join(folder));
        candidates.push(resource_dir.join("Engines").join(folder));
        candidates.push(resource_dir.join("_up_/Engines").join(folder));
        if let Some(parent) = resource_dir.parent() {
            candidates.push(parent.join("Resources").join(folder));
            candidates.push(parent.join(folder));
        }
    }

    if let Ok(root) = app_root(app) {
        candidates.push(root.join("Engines").join(folder));
        candidates.push(root.join(folder));
    }

    // Dev checkout fallback.
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("Engines")
            .join(folder),
    );

    Ok(candidates
        .into_iter()
        .filter(|path| path.is_dir())
        .collect())
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    // Frameworks and Wine's Unix bridges contain relative symlinks. Following
    // them changes dylib locations and breaks @loader_path dependencies.
    let status = Command::new("/usr/bin/ditto")
        .arg(source)
        .arg(destination)
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Could not copy runtime from {}: {status}",
            source.display()
        ))
    }
}

#[tauri::command]
fn create_bottle(app: tauri::AppHandle, name: Option<String>) -> Result<Bottle, String> {
    let bottles_dir = bottles_dir(&app)?;
    ensure_dir(&bottles_dir)?;
    let existing = list_bottles(app.clone())?;
    let name = name.unwrap_or_else(|| next_bottle_name(&existing));
    let path = bottles_dir.join(slug(&name));

    ensure_dir(&path)?;

    let bottle = Bottle {
        id: Uuid::new_v4(),
        name,
        path,
        windows_version: "Windows 11".to_string(),
        settings: default_bottle_settings(),
        app_settings: HashMap::new(),
        apps: vec![],
    };

    save_bottle(&bottle)?;

    let _ = run_wine_process(&app, &bottle, &["wineboot", "-u"]);
    Ok(bottle)
}

#[tauri::command]
fn steam_status(app: tauri::AppHandle) -> Result<SteamStatus, String> {
    match steam_bottle(&app)? {
        Some(bottle) => {
            let installed = steam_exe(&bottle).exists();
            Ok(SteamStatus {
                installed,
                running: installed && steam_is_running(),
                bottle_id: Some(bottle.id),
                bottle_path: Some(bottle.path.clone()),
                games: scan_steam_games(&bottle),
            })
        }
        None => Ok(SteamStatus {
            installed: false,
            running: false,
            bottle_id: None,
            bottle_path: None,
            games: vec![],
        }),
    }
}

#[tauri::command]
fn install_steam(app: tauri::AppHandle) -> Result<Bottle, String> {
    emit_install_progress(&app, "Steam", "Preparing Steam bottle", None, None, None);
    let mut bottle = match steam_bottle(&app)? {
        Some(bottle) => bottle,
        None => create_bottle(app.clone(), Some("Steam".to_string()))?,
    };
    bottle.windows_version = "Windows 10".to_string();
    bottle.settings.d3d_metal = true;
    bottle.settings.dxvk = false;
    bottle.settings.msync = true;
    bottle.settings.retina = false;
    bottle.settings.vsync = true;
    save_bottle(&bottle)?;

    let installer = installers_dir(&app)?.join("SteamSetup.exe");
    ensure_dir(
        installer
            .parent()
            .ok_or_else(|| "Could not resolve installer directory".to_string())?,
    )?;

    if !installer.exists() {
        emit_install_progress(
            &app,
            "Steam",
            "Downloading Steam installer from Valve",
            None,
            None,
            None,
        );
        download_file(
            "https://cdn.fastly.steamstatic.com/client/installer/SteamSetup.exe",
            &installer,
        )?;
    }

    add_steam_launcher(&mut bottle)?;
    install_steam_dependencies(&app, &bottle)?;
    apply_steam_registry_tweaks(&app, &bottle)?;

    if steam_exe(&bottle).exists() {
        emit_install_progress(
            &app,
            "Steam",
            "Steam is already installed",
            None,
            None,
            Some(100),
        );
        return Ok(bottle);
    }

    emit_install_progress(&app, "Steam", "Running Steam setup", None, None, None);
    run_wine_process_with_key(
        &app,
        &bottle,
        Some("steam-client"),
        &[path_string(&installer).as_str()],
    )?;
    if steam_exe(&bottle).exists() {
        emit_install_progress(&app, "Steam", "Launching Steam", None, None, Some(100));
        let _ = run_wine_process(&app, &bottle, &[steam_command().as_str()]);
    }
    Ok(bottle)
}

#[tauri::command]
fn launch_steam(app: tauri::AppHandle) -> Result<(), String> {
    let bottle = steam_bottle(&app)?.ok_or_else(|| "Steam is not installed yet.".to_string())?;
    run_wine_process_with_key(
        &app,
        &bottle,
        Some("steam-client"),
        &[steam_command().as_str()],
    )
}

#[tauri::command]
fn apply_steam_compatibility_mode(app: tauri::AppHandle) -> Result<Bottle, String> {
    let mut bottle =
        steam_bottle(&app)?.ok_or_else(|| "Install Steam first, then try again.".to_string())?;
    bottle.settings.d3d_metal = false;
    bottle.settings.dxvk = false;
    bottle.settings.msync = false;
    bottle.settings.retina = false;
    bottle.settings.vsync = true;
    save_bottle(&bottle)?;
    Ok(bottle)
}

#[tauri::command]
fn stop_steam(app: tauri::AppHandle) -> Result<(), String> {
    let bottle =
        steam_bottle(&app)?.ok_or_else(|| "Install Steam first, then try again.".to_string())?;
    stop_bottle_processes(&app, &bottle)
}

#[tauri::command]
fn repair_steam_web_cache(app: tauri::AppHandle) -> Result<(), String> {
    let bottle =
        steam_bottle(&app)?.ok_or_else(|| "Install Steam first, then try again.".to_string())?;
    stop_bottle_processes(&app, &bottle)?;

    for path in steam_web_cache_paths(&bottle) {
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }

    Ok(())
}

#[tauri::command]
fn set_bottle_vsync(
    app: tauri::AppHandle,
    bottle_id: Uuid,
    enabled: bool,
) -> Result<Bottle, String> {
    let mut bottle = find_bottle(&app, bottle_id)?;
    bottle.settings.vsync = enabled;
    save_bottle(&bottle)?;
    Ok(bottle)
}

#[tauri::command]
fn set_app_settings(
    app: tauri::AppHandle,
    bottle_id: Uuid,
    app_key: String,
    settings: AppSettings,
) -> Result<Bottle, String> {
    let mut bottle = find_bottle(&app, bottle_id)?;
    if settings.engine_executable.is_none()
        && settings.d3d_metal.is_none()
        && settings.dxvk.is_none()
        && settings.msync.is_none()
        && settings.retina.is_none()
        && settings.vsync.is_none()
    {
        bottle.app_settings.remove(&app_key);
    } else {
        bottle.app_settings.insert(app_key, settings);
    }
    save_bottle(&bottle)?;
    Ok(bottle)
}

#[tauri::command]
fn scan_steam_library(app: tauri::AppHandle) -> Result<Vec<InstalledSteamGame>, String> {
    let bottle =
        steam_bottle(&app)?.ok_or_else(|| "Install Steam first, then try again.".to_string())?;
    Ok(scan_steam_games(&bottle))
}

#[tauri::command]
fn open_steam_game(app: tauri::AppHandle, app_id: u64) -> Result<(), String> {
    let bottle =
        steam_bottle(&app)?.ok_or_else(|| "Install Steam first, then try again.".to_string())?;
    let url = format!("steam://store/{app_id}");
    run_wine_process_with_key(
        &app,
        &bottle,
        Some("steam-client"),
        &[steam_command().as_str(), url.as_str()],
    )
}

#[tauri::command]
fn run_steam_game(app: tauri::AppHandle, app_id: u64) -> Result<(), String> {
    let mut bottle =
        steam_bottle(&app)?.ok_or_else(|| "Install Steam first, then try again.".to_string())?;
    let game_name = steam_game_name(&bottle, app_id);
    let app_key = format!("steam:{app_id}");

    prepare_steam_game_parity(&app, &mut bottle, app_id)?;

    if app_id == UNTIL_DAWN_STEAM_APP_ID || app_id == CS2_STEAM_APP_ID {
        if steam_is_running() {
            stop_bottle_processes(&app, &bottle)?;
            wait_for_steam_exit()?;
        }

        let app_id_text = app_id.to_string();
        let command = steam_command();
        let mut args = vec![command.as_str(), "-applaunch", app_id_text.as_str()];
        let launch_options = steam_game_launch_options(app_id);
        args.extend(launch_options.iter().map(String::as_str));
        return run_wine_process_with_key_and_app_name(
            &app,
            &bottle,
            Some(&app_key),
            &args,
            game_name.as_deref(),
        );
    }

    let url = format!("steam://rungameid/{app_id}");
    let command = steam_command();
    run_wine_process_with_key_and_app_name(
        &app,
        &bottle,
        Some(&app_key),
        &[command.as_str(), url.as_str()],
        game_name.as_deref(),
    )
}

fn prepare_steam_game_parity(
    app: &tauri::AppHandle,
    bottle: &mut Bottle,
    app_id: u64,
) -> Result<(), String> {
    ensure_steam_game_app_settings(bottle, app_id)?;
    apply_direct3d_video_memory(app, bottle)?;

    if app_id == CS2_STEAM_APP_ID {
        ensure_cs2_d3dmetal_engine(app, bottle)?;
    }

    Ok(())
}

fn ensure_steam_game_app_settings(bottle: &mut Bottle, app_id: u64) -> Result<(), String> {
    let app_key = format!("steam:{app_id}");
    if bottle.app_settings.contains_key(&app_key) {
        return Ok(());
    }

    let defaults = default_steam_game_settings(app_id);
    if defaults.d3d_metal.is_none()
        && defaults.dxvk.is_none()
        && defaults.msync.is_none()
        && defaults.retina.is_none()
        && defaults.vsync.is_none()
        && defaults.engine_executable.is_none()
    {
        return Ok(());
    }

    bottle.app_settings.insert(app_key, defaults);
    save_bottle(bottle)
}

fn default_steam_game_settings(app_id: u64) -> AppSettings {
    if app_id == CS2_STEAM_APP_ID || app_id == UNTIL_DAWN_STEAM_APP_ID {
        return AppSettings {
            engine_executable: None,
            d3d_metal: Some(true),
            dxvk: Some(false),
            msync: Some(true),
            retina: Some(false),
            vsync: Some(true),
        };
    }

    AppSettings::default()
}

fn steam_game_launch_options(app_id: u64) -> Vec<String> {
    if app_id == CS2_STEAM_APP_ID {
        return vec![
            "-nojoy".to_string(),
            "-novid".to_string(),
            "-fullscreen".to_string(),
        ];
    }
    Vec::new()
}

fn apply_direct3d_video_memory(app: &tauri::AppHandle, bottle: &Bottle) -> Result<(), String> {
    let memory_mb = recommended_video_memory_mb().to_string();
    apply_registry_string(
        app,
        bottle,
        r#"HKCU\Software\Wine\Direct3D"#,
        "VideoMemorySize",
        &memory_mb,
    )
}

fn recommended_video_memory_mb() -> u32 {
    let bytes = mac_memory_bytes().unwrap_or(16 * 1024 * 1024 * 1024);
    let gib = bytes / (1024 * 1024 * 1024);
    if gib >= 32 {
        16_384
    } else if gib >= 16 {
        8_192
    } else if gib >= 8 {
        4_096
    } else {
        2_048
    }
}

fn mac_memory_bytes() -> Option<u64> {
    let output = Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn ensure_cs2_d3dmetal_engine(app: &tauri::AppHandle, bottle: &mut Bottle) -> Result<(), String> {
    let app_key = format!("steam:{CS2_STEAM_APP_ID}");
    let override_settings = bottle.app_settings.get(&app_key).cloned();
    let engine = select_engine(app, override_settings.as_ref())?;

    if engine.d3d_metal_capable {
        if bottle
            .app_settings
            .get(&app_key)
            .and_then(|settings| settings.engine_executable.as_ref())
            .is_none()
        {
            let mut settings = bottle
                .app_settings
                .get(&app_key)
                .cloned()
                .unwrap_or_else(|| default_steam_game_settings(CS2_STEAM_APP_ID));
            settings.engine_executable = Some(engine.executable.clone());
            settings.d3d_metal = Some(true);
            settings.dxvk = Some(false);
            settings.msync = Some(true);
            settings.retina = Some(false);
            bottle.app_settings.insert(app_key, settings);
            save_bottle(bottle)?;
        }
        return Ok(());
    }

    Err(
        "Counter-Strike 2 needs a D3DMetal engine. Install CrossOver, or ship/install kirito's redistributed KiritoD3DMetal runtime (Engines/gptk-wine + Engines/d3dmetal-redist), then reopen kirito."
            .to_string(),
    )
}

#[tauri::command]
fn proton_rating(app_id: u64) -> Result<Option<ProtonRating>, String> {
    let url = format!("https://www.protondb.com/api/v1/reports/summaries/{app_id}.json");
    let output = Command::new("curl")
        .args(["-L", "--silent", "--fail", url.as_str()])
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Ok(None);
    }

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let tier = value
        .get("tier")
        .or_else(|| value.get("bestReportedTier"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    Ok(Some(ProtonRating {
        app_id,
        tier,
        confidence: value
            .get("confidence")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        score: value.get("score").and_then(serde_json::Value::as_f64),
        total: value.get("total").and_then(serde_json::Value::as_u64),
        source_url: format!("https://www.protondb.com/app/{app_id}"),
    }))
}

#[tauri::command]
fn anti_cheat_reports(names: Vec<String>) -> Result<Vec<AntiCheatReport>, String> {
    let reports = cached_anti_cheat_reports()?;
    let mut matches = Vec::new();
    let mut seen = HashSet::new();

    for name in names {
        let key = normalize_match_key(&name);
        if key.is_empty() || !seen.insert(key.clone()) {
            continue;
        }

        if let Some(report) = reports
            .iter()
            .find(|report| normalize_match_key(&report.name) == key)
            .or_else(|| {
                reports.iter().find(|report| {
                    let report_key = normalize_match_key(&report.name);
                    key.contains(&report_key) || report_key.contains(&key)
                })
            })
        {
            let mut report = report.clone();
            report.matched_name = name;
            matches.push(report);
        }
    }

    Ok(matches)
}

#[tauri::command]
fn controller_diagnostics() -> Result<ControllerDiagnostics, String> {
    let cache = CONTROLLER_CACHE.get_or_init(|| Mutex::new(None));
    if let Some(cached) = cache.lock().unwrap().as_ref() {
        if cached.captured_at.elapsed() < CONTROLLER_TTL {
            return Ok(cached.diagnostics.clone());
        }
    }

    let diagnostics = scan_controller_diagnostics();
    *cache.lock().unwrap() = Some(CachedControllerDiagnostics {
        captured_at: Instant::now(),
        diagnostics: diagnostics.clone(),
    });
    Ok(diagnostics)
}

fn scan_controller_diagnostics() -> ControllerDiagnostics {
    let mut devices = Vec::new();
    collect_system_profiler_controllers("USB", "SPUSBDataType", &mut devices);
    collect_system_profiler_controllers("Bluetooth", "SPBluetoothDataType", &mut devices);
    dedupe_controller_devices(&mut devices);

    ControllerDiagnostics {
        checked_at: chrono_like_timestamp(),
        devices,
    }
}

fn collect_system_profiler_controllers(
    transport: &str,
    data_type: &str,
    devices: &mut Vec<ControllerDevice>,
) {
    let Ok(output) = Command::new("system_profiler")
        .args([data_type, "-json"])
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return;
    };
    collect_controller_devices_from_json(&value, transport, devices);
}

fn collect_controller_devices_from_json(
    value: &serde_json::Value,
    transport: &str,
    devices: &mut Vec<ControllerDevice>,
) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_controller_devices_from_json(item, transport, devices);
            }
        }
        serde_json::Value::Object(map) => {
            let name = map
                .get("_name")
                .or_else(|| map.get("name"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let vendor = map
                .get("manufacturer")
                .or_else(|| map.get("vendor_id"))
                .or_else(|| map.get("device_vendor"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);

            if let Some(name) = name {
                let vendor_hint = vendor.as_deref().unwrap_or_default();
                if looks_like_controller(name, vendor_hint) {
                    devices.push(ControllerDevice {
                        name: name.to_string(),
                        transport: transport.to_string(),
                        vendor,
                    });
                }
            }

            for child in map.values() {
                collect_controller_devices_from_json(child, transport, devices);
            }
        }
        _ => {}
    }
}

fn looks_like_controller(name: &str, vendor: &str) -> bool {
    let haystack = format!("{} {}", name, vendor).to_ascii_lowercase();
    let excluded = [
        "host controller",
        "usb controller",
        "bluetooth controller",
        "keyboard",
        "mouse",
        "trackpad",
        "touchpad",
        "audio",
        "headset",
    ];
    if excluded.iter().any(|token| haystack.contains(token)) {
        return false;
    }

    [
        "xbox",
        "dualshock",
        "dualsense",
        "playstation",
        "wireless controller",
        "gamepad",
        "joy-con",
        "joycon",
        "switch pro",
        "8bitdo",
        "gulikit",
        "backbone",
        "razer kishi",
        "steam controller",
        "controller",
    ]
    .iter()
    .any(|token| haystack.contains(token))
}

fn dedupe_controller_devices(devices: &mut Vec<ControllerDevice>) {
    let mut seen = HashSet::new();
    devices.retain(|device| {
        seen.insert((
            normalize_match_key(&device.name),
            device.transport.to_ascii_lowercase(),
        ))
    });
    devices.sort_by_key(|device| {
        (
            device.transport.to_ascii_lowercase(),
            device.name.to_ascii_lowercase(),
        )
    });
}

fn cached_anti_cheat_reports() -> Result<Vec<AntiCheatReport>, String> {
    let cache = ANTI_CHEAT_CACHE.get_or_init(|| Mutex::new(None));
    if let Some(cached) = cache.lock().unwrap().as_ref() {
        if cached.captured_at.elapsed() < ANTI_CHEAT_TTL {
            return Ok(cached.reports.clone());
        }
    }

    let output = Command::new("curl")
        .args([
            "-L",
            "--silent",
            "--fail",
            "https://areweanticheatyet.com/no-js",
        ])
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err("Anti-cheat database is unavailable right now.".to_string());
    }

    let html = String::from_utf8_lossy(&output.stdout);
    let reports = parse_anti_cheat_rows(&html);
    *cache.lock().unwrap() = Some(CachedAntiCheatReports {
        captured_at: Instant::now(),
        reports: reports.clone(),
    });
    Ok(reports)
}

fn parse_anti_cheat_rows(html: &str) -> Vec<AntiCheatReport> {
    let mut reports = Vec::new();

    for row in html.split("<tr").skip(1) {
        let Some(row) = row.split("</tr>").next() else {
            continue;
        };
        let cells = table_cells(row);
        if cells.len() < 5 {
            continue;
        }

        let name = html_text(&cells[0]);
        let status = html_text(&cells[1]);
        if name.is_empty() || status.is_empty() {
            continue;
        }

        let anti_cheats = image_alts(&cells[2])
            .into_iter()
            .filter_map(|value| {
                value
                    .strip_prefix("Image: ")
                    .map(str::to_string)
                    .or(Some(value))
            })
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>();
        let notes = non_empty_string(html_text(&cells[3]));
        let updated = non_empty_string(html_text(&cells[4]));

        reports.push(AntiCheatReport {
            matched_name: name.clone(),
            name,
            status,
            anti_cheats,
            notes,
            updated,
            source_url: "https://areweanticheatyet.com/no-js".to_string(),
        });
    }

    reports
}

fn table_cells(row: &str) -> Vec<String> {
    row.split("<td")
        .skip(1)
        .filter_map(|cell| {
            let after_open = cell.split_once('>')?.1;
            Some(
                after_open
                    .split("</td>")
                    .next()
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect()
}

fn image_alts(html: &str) -> Vec<String> {
    let mut alts = Vec::new();
    for part in html.split("alt=\"").skip(1) {
        if let Some(value) = part.split('"').next() {
            alts.push(html_decode(value));
        }
    }
    alts
}

fn html_text(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    html_decode(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn html_decode(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#039;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

fn non_empty_string(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn normalize_match_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[tauri::command]
fn search_steam(query: String) -> Result<Vec<SteamGame>, String> {
    let query = query.trim();
    if query.len() < 2 {
        return Ok(vec![]);
    }

    let url = format!(
        "https://store.steampowered.com/api/storesearch/?term={}&cc=US&l=en",
        percent_encode(query)
    );
    let output = Command::new("curl")
        .args(["-L", "--silent", "--fail", url.as_str()])
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err("Steam search is unavailable right now.".to_string());
    }

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let items = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "Steam returned an unexpected search response.".to_string())?;

    Ok(items.iter().filter_map(parse_steam_game).take(12).collect())
}

#[tauri::command]
fn run_wine_tool(app: tauri::AppHandle, bottle_id: Uuid, tool: String) -> Result<(), String> {
    let bottle = find_bottle(&app, bottle_id)?;
    run_wine_process(&app, &bottle, &[tool.as_str()])
}

#[tauri::command]
fn run_app(app: tauri::AppHandle, bottle_id: Uuid, app_id: Uuid) -> Result<(), String> {
    let bottle = find_bottle(&app, bottle_id)?;
    let launcher = bottle
        .apps
        .iter()
        .find(|launcher| launcher.id == app_id)
        .ok_or_else(|| "App not found".to_string())?;
    if launcher.command == bedrock::APP_COMMAND {
        return bedrock::launch(&app);
    }
    run_wine_process_with_key(
        &app,
        &bottle,
        Some(&format!("app:{app_id}")),
        &[launcher.command.as_str()],
    )
}

#[tauri::command]
fn bedrock_status(app: tauri::AppHandle) -> Result<bedrock::Status, String> {
    bedrock::status(&app)
}

#[tauri::command]
async fn install_bedrock(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || bedrock::install(&app))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn launch_bedrock(app: tauri::AppHandle) -> Result<(), String> {
    bedrock::launch(&app)
}

#[tauri::command]
fn install_app(app: tauri::AppHandle, bottle_id: Uuid) -> Result<(), String> {
    let bottle = find_bottle(&app, bottle_id)?;
    let Some(path) = rfd_pick_installer() else {
        return Ok(());
    };

    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("msi"))
    {
        run_wine_process(
            &app,
            &bottle,
            &["msiexec", "/i", path_string(&path).as_str()],
        )
    } else {
        run_wine_process(&app, &bottle, &[path_string(&path).as_str()])
    }
}

#[tauri::command]
fn open_path(path: PathBuf) -> Result<(), String> {
    Command::new("open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_feedback(
    kind: String,
    app_name: Option<String>,
    source: Option<String>,
    steam_app_id: Option<u64>,
    settings: Option<String>,
    notes: Option<String>,
) -> Result<(), String> {
    let (template, title, body) = feedback_template(&kind);
    let scoped_title = app_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|name| format!("{title}{name}"))
        .unwrap_or_else(|| title.to_string());
    let scoped_notes = compatibility_notes(steam_app_id, notes);
    let mut query = vec![
        ("template", template.to_string()),
        ("title", scoped_title),
        ("body", body),
    ];

    if kind == "compatibility" {
        push_issue_form_value(&mut query, "app-name", app_name);
        push_issue_form_value(&mut query, "source", source);
        push_issue_form_value(&mut query, "settings", settings);
        push_issue_form_value(&mut query, "notes", scoped_notes);
    }

    let url = format!(
        "https://github.com/ryumacodes/kirito/issues/new?{}",
        query
            .into_iter()
            .map(|(key, value)| format!("{key}={}", percent_encode(&value)))
            .collect::<Vec<_>>()
            .join("&")
    );

    Command::new("open")
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn push_issue_form_value(
    query: &mut Vec<(&'static str, String)>,
    key: &'static str,
    value: Option<String>,
) {
    if let Some(value) = value {
        if !value.trim().is_empty() {
            query.push((key, value));
        }
    }
}

fn compatibility_notes(steam_app_id: Option<u64>, notes: Option<String>) -> Option<String> {
    let mut lines = Vec::new();
    if let Some(app_id) = steam_app_id {
        lines.push(format!("Steam app id: {app_id}"));
    }
    if let Some(notes) = notes {
        if !notes.trim().is_empty() {
            lines.push(notes);
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn feedback_template(kind: &str) -> (&'static str, &'static str, String) {
    match kind {
        "issue" => (
            "bug_report.yml",
            "[Issue] ",
            [
                "What broke?",
                "",
                "Steps to reproduce:",
                "1. ",
                "2. ",
                "3. ",
                "",
                "What happened?",
                "",
                "What did you expect?",
                "",
                "Useful details:",
                "- macOS version:",
                "- Mac model / chip:",
                "- CrossOver/Wine engine:",
                "- Bottle/app/game:",
                "- Screenshot or log notes:",
            ]
            .join("\n"),
        ),
        "compatibility" => (
            "compatibility_report.yml",
            "[Compatibility] ",
            [
                "App or game name:",
                "",
                "Store/source:",
                "",
                "How well did it run?",
                "- Perfect / playable / launches but broken / does not launch",
                "",
                "Settings used:",
                "- D3DMetal:",
                "- DXVK:",
                "- MSync:",
                "- Retina:",
                "- VSync:",
                "",
                "What worked?",
                "",
                "What failed?",
                "",
                "Useful details:",
                "- Anti-cheat status if any:",
                "- Controller/input notes:",
                "- Launcher/login notes:",
            ]
            .join("\n"),
        ),
        "feature" => (
            "feature_request.yml",
            "[Feature] ",
            [
                "What should Kirito add or improve?",
                "",
                "Why does this matter?",
                "",
                "What would a good version look like?",
                "",
                "Related app/game/workflow:",
                "",
                "Any references or examples:",
            ]
            .join("\n"),
        ),
        _ => (
            "bug_report.yml",
            "[Feedback] ",
            [
                "What are you trying to tell us?",
                "",
                "Useful details:",
                "- App/game:",
                "- macOS version:",
                "- Mac model / chip:",
            ]
            .join("\n"),
        ),
    }
}

fn download_file(url: &str, destination: &Path) -> Result<(), String> {
    let temp = destination.with_extension("download");
    let status = Command::new("curl")
        .args([
            "-L",
            "--fail",
            "--retry",
            "3",
            "--silent",
            "--show-error",
            url,
            "-o",
        ])
        .arg(&temp)
        .status()
        .map_err(|error| error.to_string())?;

    if !status.success() {
        let _ = fs::remove_file(&temp);
        return Err("Download failed. Check your internet connection and try again.".to_string());
    }

    fs::rename(temp, destination).map_err(|error| error.to_string())
}

fn rfd_pick_installer() -> Option<PathBuf> {
    let script = r#"set pickedFile to choose file with prompt "Choose a Windows installer" of type {"exe", "msi"}
POSIX path of pickedFile"#;
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn run_wine_process(app: &tauri::AppHandle, bottle: &Bottle, args: &[&str]) -> Result<(), String> {
    run_wine_process_with_key(app, bottle, None, args)
}

fn run_wine_process_wait(
    app: &tauri::AppHandle,
    bottle: &Bottle,
    app_key: Option<&str>,
    args: &[&str],
) -> Result<(), String> {
    let override_settings = app_key.and_then(|key| bottle.app_settings.get(key));
    let engine = select_engine(app, override_settings)?;
    let effective_settings = effective_settings(&bottle.settings, override_settings);

    let logs_dir = logs_dir(app)?;
    ensure_dir(&logs_dir)?;

    let mut command = Command::new(&engine.executable);
    command.args(args);
    command.env("WINEPREFIX", &bottle.path);
    command.env("WINEDEBUG", "-all");
    command.env(
        "WINEESYNC",
        if effective_settings.msync { "0" } else { "1" },
    );
    command.env(
        "WINEMSYNC",
        if effective_settings.msync { "1" } else { "0" },
    );
    command.env(
        "WHINE_D3DMETAL",
        if effective_settings.d3d_metal {
            "1"
        } else {
            "0"
        },
    );
    command.env(
        "WHINE_DXVK",
        if effective_settings.dxvk { "1" } else { "0" },
    );
    command.env("DXVK_CONFIG", dxvk_config(&effective_settings));
    apply_graphics_backend_env(&mut command, &engine, &effective_settings);
    apply_wine_process_hints(&mut command, app_key, args, None);

    let log_file = fs::File::create(logs_dir.join(format!(
        "{}-{}.log",
        slug(&bottle.name),
        chrono_like_timestamp()
    )))
    .map_err(|error| error.to_string())?;
    let error_log = log_file.try_clone().map_err(|error| error.to_string())?;
    command.stdout(log_file);
    command.stderr(error_log);

    let status = command.status().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Wine command exited with {status}"))
    }
}

fn apply_wine_process_hints(
    command: &mut Command,
    app_key: Option<&str>,
    args: &[&str],
    preloader_app_name: Option<&str>,
) {
    if is_steam_command(app_key, args) {
        command.env("WINE_WAIT_CHILD_PIPE_IGNORE", "steam.exe");
        command.env(
            "WINEPRELOADERAPPNAME",
            preloader_app_name.unwrap_or("Steam"),
        );
    }

    if should_advertise_rosetta_avx(app_key, args) {
        command.env("ROSETTA_ADVERTISE_AVX", "1");
    }

    // CrossOver/GPTK-style CS2 path: avoid advertising AVX and keep ESync off when MSync is used.
    if steam_app_id_from_key(app_key) == Some(CS2_STEAM_APP_ID) {
        command.env_remove("ROSETTA_ADVERTISE_AVX");
    }
}

fn is_steam_command(app_key: Option<&str>, args: &[&str]) -> bool {
    app_key == Some("steam-client")
        || args.iter().any(|arg| {
            let arg = arg.to_ascii_lowercase();
            arg.contains("steam.exe") || arg.contains("steamsetup") || arg.contains("steaminstall")
        })
}

fn should_advertise_rosetta_avx(app_key: Option<&str>, args: &[&str]) -> bool {
    if steam_app_id_from_key(app_key) == Some(UNTIL_DAWN_STEAM_APP_ID) {
        return true;
    }

    args.iter().any(|arg| {
        arg.strip_prefix("steam://rungameid/")
            .and_then(|value| value.parse::<u64>().ok())
            == Some(UNTIL_DAWN_STEAM_APP_ID)
    })
}

fn steam_app_id_from_key(app_key: Option<&str>) -> Option<u64> {
    app_key
        .and_then(|key| key.strip_prefix("steam:"))
        .and_then(|value| value.parse::<u64>().ok())
}

fn run_wine_process_with_key(
    app: &tauri::AppHandle,
    bottle: &Bottle,
    app_key: Option<&str>,
    args: &[&str],
) -> Result<(), String> {
    run_wine_process_with_key_and_app_name(app, bottle, app_key, args, None)
}

fn run_wine_process_with_key_and_app_name(
    app: &tauri::AppHandle,
    bottle: &Bottle,
    app_key: Option<&str>,
    args: &[&str],
    preloader_app_name: Option<&str>,
) -> Result<(), String> {
    if bottle.name == "Steam" || app_key == Some("steam-client") {
        steam_logs::protect(&bottle.path)?;
    }
    let override_settings = app_key.and_then(|key| bottle.app_settings.get(key));
    let engine = select_engine(app, override_settings)?;
    let effective_settings = effective_settings(&bottle.settings, override_settings);

    let logs_dir = logs_dir(app)?;
    ensure_dir(&logs_dir)?;

    let mut command = Command::new(&engine.executable);
    command.args(args);
    command.env("WINEPREFIX", &bottle.path);
    command.env("WINEDEBUG", "-all");
    command.env(
        "WINEESYNC",
        if effective_settings.msync { "0" } else { "1" },
    );
    command.env(
        "WINEMSYNC",
        if effective_settings.msync { "1" } else { "0" },
    );
    command.env(
        "WHINE_D3DMETAL",
        if effective_settings.d3d_metal {
            "1"
        } else {
            "0"
        },
    );
    command.env(
        "WHINE_DXVK",
        if effective_settings.dxvk { "1" } else { "0" },
    );
    command.env("DXVK_CONFIG", dxvk_config(&effective_settings));
    apply_graphics_backend_env(&mut command, &engine, &effective_settings);
    apply_wine_process_hints(&mut command, app_key, args, preloader_app_name);

    let log_file = fs::File::create(logs_dir.join(format!(
        "{}-{}.log",
        slug(&bottle.name),
        chrono_like_timestamp()
    )))
    .map_err(|error| error.to_string())?;
    let error_log = log_file.try_clone().map_err(|error| error.to_string())?;
    command.stdout(log_file);
    command.stderr(error_log);

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn stop_bottle_processes(app: &tauri::AppHandle, bottle: &Bottle) -> Result<(), String> {
    let engine = select_engine(app, None)?;
    let wineserver = engine
        .executable
        .parent()
        .map(|parent| parent.join("wineserver"))
        .filter(|path| is_executable(path))
        .unwrap_or_else(|| PathBuf::from("wineserver"));

    Command::new(wineserver)
        .arg("-k")
        .env("WINEPREFIX", &bottle.path)
        .status()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn stop_all_bottle_processes(app: tauri::AppHandle) {
    let Ok(bottles) = list_bottles(app.clone()) else {
        return;
    };

    for bottle in bottles {
        let _ = stop_bottle_processes(&app, &bottle);
    }
}

fn wait_for_steam_exit() -> Result<(), String> {
    for _ in 0..40 {
        if !steam_is_running_fresh() {
            return Ok(());
        }
        thread::sleep(std::time::Duration::from_millis(250));
    }

    Err("Steam did not close in time. Quit Steam and try Until Dawn again.".to_string())
}

fn select_engine(
    app: &tauri::AppHandle,
    settings: Option<&AppSettings>,
) -> Result<WineEngine, String> {
    let engines = list_engines(app.clone())?;
    if let Some(path) = settings.and_then(|settings| settings.engine_executable.as_ref()) {
        if is_executable(path) {
            return Ok(WineEngine {
                display_name: engine_name(path),
                d3d_metal_capable: engine_supports_d3dmetal(path),
                executable: path.clone(),
            });
        }
    }

    let prefer_d3dmetal = settings
        .and_then(|settings| settings.d3d_metal)
        .unwrap_or(false);
    if prefer_d3dmetal {
        if let Some(engine) = engines
            .iter()
            .find(|engine| engine.d3d_metal_capable)
            .cloned()
        {
            return Ok(engine);
        }
    }

    engines.into_iter().next().ok_or_else(|| {
        "No Wine engine detected. Kirito will install Wine Stable on launch, or reopen the app to retry.".to_string()
    })
}

fn effective_settings(
    defaults: &BottleSettings,
    overrides: Option<&AppSettings>,
) -> BottleSettings {
    let Some(overrides) = overrides else {
        return defaults.clone();
    };

    BottleSettings {
        d3d_metal: overrides.d3d_metal.unwrap_or(defaults.d3d_metal),
        dxvk: overrides.dxvk.unwrap_or(defaults.dxvk),
        msync: overrides.msync.unwrap_or(defaults.msync),
        retina: overrides.retina.unwrap_or(defaults.retina),
        vsync: overrides.vsync.unwrap_or(defaults.vsync),
    }
}

fn default_bottle_settings() -> BottleSettings {
    BottleSettings {
        d3d_metal: true,
        dxvk: false,
        msync: true,
        retina: false,
        vsync: true,
    }
}

fn steam_bottle(app: &tauri::AppHandle) -> Result<Option<Bottle>, String> {
    Ok(list_bottles(app.clone())?
        .into_iter()
        .find(|bottle| bottle.name.eq_ignore_ascii_case("steam")))
}

fn add_steam_launcher(bottle: &mut Bottle) -> Result<(), String> {
    if !bottle.apps.iter().any(|app| app.name == "Steam") {
        bottle.apps.push(InstalledApp {
            id: Uuid::new_v4(),
            name: "Steam".to_string(),
            command: steam_command(),
        });
        save_bottle(bottle)?;
    }

    Ok(())
}

fn install_steam_dependencies(app: &tauri::AppHandle, bottle: &Bottle) -> Result<(), String> {
    let _ = run_wine_process_wait(app, bottle, None, &["wineboot", "-u"]);
    let recipes = steam_dependency_recipes();
    let total = recipes.len();

    for (index, recipe) in recipes.iter().enumerate() {
        let step = index + 1;
        if dependency_marker(bottle, recipe.id).exists() {
            append_install_log(
                app,
                bottle,
                &format!("{} already prepared; skipping.", recipe.name),
            );
            emit_install_progress(
                app,
                "Steam",
                &format!("Step {step}/{total}: {} already installed", recipe.name),
                Some(step),
                Some(total),
                Some(step_percent(step, total)),
            );
            continue;
        }

        append_install_log(
            app,
            bottle,
            &format!("Preparing {} from {}.", recipe.name, recipe.source),
        );
        emit_install_progress(
            app,
            "Steam",
            &format!("Step {step}/{total}: Preparing {}", recipe.name),
            Some(step),
            Some(total),
            Some(step_percent(index, total)),
        );

        match install_dependency_recipe(app, bottle, recipe, step, total) {
            Ok(()) => {
                mark_dependency_ready(bottle, recipe.id)?;
                append_install_log(app, bottle, &format!("{} is ready.", recipe.name));
                emit_install_progress(
                    app,
                    "Steam",
                    &format!("Step {step}/{total}: {} ready", recipe.name),
                    Some(step),
                    Some(total),
                    Some(step_percent(step, total)),
                );
            }
            Err(error) => {
                append_install_log(
                    app,
                    bottle,
                    &format!("{} was skipped or failed: {error}", recipe.name),
                );
                emit_install_progress(
                    app,
                    "Steam",
                    &format!("Step {step}/{total}: {} skipped", recipe.name),
                    Some(step),
                    Some(total),
                    Some(step_percent(step, total)),
                );
            }
        }
    }

    Ok(())
}

fn apply_steam_registry_tweaks(app: &tauri::AppHandle, bottle: &Bottle) -> Result<(), String> {
    emit_install_progress(
        app,
        "Steam",
        "Applying Steam bottle tweaks",
        None,
        None,
        None,
    );

    let string_values = [
        (
            r#"HKCU\Software\Wine\AppDefaults\steam.exe\DllOverrides"#,
            "wineoss.drv",
            "d",
        ),
        (
            r#"HKCU\Software\Wine\Fonts\Replacements"#,
            "Lucida Console",
            "MS Sans Serif",
        ),
        (
            r#"HKCU\Software\Wine\AppDefaults\steamwebhelper.exe"#,
            "Version",
            "win10",
        ),
    ];

    for (key, name, value) in string_values {
        apply_registry_string(app, bottle, key, name, value)?;
    }

    if prefers_cjk_fonts() {
        apply_registry_dword(
            app,
            bottle,
            r#"HKCU\Software\Valve\Steam"#,
            "DWriteEnable",
            0,
        )?;
        apply_registry_string(
            app,
            bottle,
            r#"HKCU\Software\Wine\Fonts\Replacements"#,
            "Meiryo",
            "Source Han Sans SC",
        )?;
        apply_registry_string(
            app,
            bottle,
            r#"HKCU\Software\Wine\Fonts\Replacements"#,
            "Meiryo UI",
            "Source Han Sans SC",
        )?;
    }

    apply_direct3d_video_memory(app, bottle)?;

    Ok(())
}

fn apply_registry_string(
    app: &tauri::AppHandle,
    bottle: &Bottle,
    key: &str,
    name: &str,
    value: &str,
) -> Result<(), String> {
    run_wine_process_wait(
        app,
        bottle,
        None,
        &["reg", "add", key, "/v", name, "/d", value, "/f"],
    )
    .or_else(|error| {
        append_install_log(
            app,
            bottle,
            &format!("Could not set registry value {key}\\{name}: {error}"),
        );
        Ok::<(), String>(())
    })
}

fn apply_registry_dword(
    app: &tauri::AppHandle,
    bottle: &Bottle,
    key: &str,
    name: &str,
    value: u32,
) -> Result<(), String> {
    let value = value.to_string();
    run_wine_process_wait(
        app,
        bottle,
        None,
        &[
            "reg",
            "add",
            key,
            "/v",
            name,
            "/t",
            "REG_DWORD",
            "/d",
            value.as_str(),
            "/f",
        ],
    )
    .or_else(|error| {
        append_install_log(
            app,
            bottle,
            &format!("Could not set registry value {key}\\{name}: {error}"),
        );
        Ok::<(), String>(())
    })
}

fn prefers_cjk_fonts() -> bool {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .filter_map(|name| env::var(name).ok())
        .map(|value| value.to_ascii_lowercase())
        .any(|value| value.starts_with("ja") || value.starts_with("ko") || value.starts_with("zh"))
}

fn steam_dependency_recipes() -> &'static [DependencyRecipe] {
    &[
        DependencyRecipe {
            id: "vcredist-14-x86",
            name: "Microsoft Visual C++ v14 Redistributable (x86)",
            source: "Microsoft",
            url: Some("https://aka.ms/vc14/vc_redist.x86.exe"),
            file_name: Some("vc_redist.x86.exe"),
            sha256: None,
            action: DependencyAction::Exe {
                args: &["/install", "/quiet", "/norestart"],
            },
        },
        DependencyRecipe {
            id: "vcredist-14-x64",
            name: "Microsoft Visual C++ v14 Redistributable (x64)",
            source: "Microsoft",
            url: Some("https://aka.ms/vc14/vc_redist.x64.exe"),
            file_name: Some("vc_redist.x64.exe"),
            sha256: None,
            action: DependencyAction::Exe {
                args: &["/install", "/quiet", "/norestart"],
            },
        },
        DependencyRecipe {
            id: "wine-mono-11-1-0",
            name: "Wine Mono",
            source: "WineHQ",
            url: Some("https://dl.winehq.org/wine/wine-mono/11.1.0/wine-mono-11.1.0-x86.msi"),
            file_name: Some("wine-mono-11.1.0-x86.msi"),
            sha256: None,
            action: DependencyAction::Msi {
                args: &["/qn", "/norestart"],
            },
        },
        DependencyRecipe {
            id: "source-han-sans-super-otc-2-005r",
            name: "Source Han Sans CJK fonts",
            source: "Adobe Fonts",
            url: Some("https://github.com/adobe-fonts/source-han-sans/releases/download/2.005R/01_SourceHanSans.ttc.zip"),
            file_name: Some("01_SourceHanSans.ttc.zip"),
            sha256: Some("a024cf1759494847cd47aae4379bcb3dc530017c709f3f503ee0ed918dd92952"),
            action: DependencyAction::FontZip,
        },
        DependencyRecipe {
            id: "wine-bundled-richedit-msls",
            name: "Legacy RichEdit / Microsoft Line Services",
            source: "Bundled Wine runtime",
            url: None,
            file_name: None,
            sha256: None,
            action: DependencyAction::WineDlls {
                dlls: &["msls31.dll", "riched20.dll"],
            },
        },
    ]
}

fn install_dependency_recipe(
    app: &tauri::AppHandle,
    bottle: &Bottle,
    recipe: &DependencyRecipe,
    step: usize,
    total: usize,
) -> Result<(), String> {
    match recipe.action {
        DependencyAction::Exe { args } => {
            let installer = dependency_installer_path(app, recipe, step, total)?;
            let installer = path_string(&installer);
            let mut command_args = vec![installer.as_str()];
            command_args.extend(args);
            emit_install_progress(
                app,
                "Steam",
                &format!("Step {step}/{total}: Running {}", recipe.name),
                Some(step),
                Some(total),
                Some(step_percent(step.saturating_sub(1), total)),
            );
            run_wine_process_wait(app, bottle, None, &command_args)
        }
        DependencyAction::Msi { args } => {
            let installer = dependency_installer_path(app, recipe, step, total)?;
            let installer = path_string(&installer);
            let mut command_args = vec!["msiexec", "/i", installer.as_str()];
            command_args.extend(args);
            emit_install_progress(
                app,
                "Steam",
                &format!("Step {step}/{total}: Running {}", recipe.name),
                Some(step),
                Some(total),
                Some(step_percent(step.saturating_sub(1), total)),
            );
            run_wine_process_wait(app, bottle, None, &command_args)
        }
        DependencyAction::FontZip => {
            let archive = dependency_installer_path(app, recipe, step, total)?;
            emit_install_progress(
                app,
                "Steam",
                &format!("Step {step}/{total}: Installing {}", recipe.name),
                Some(step),
                Some(total),
                Some(step_percent(step.saturating_sub(1), total)),
            );
            install_font_zip(app, bottle, recipe, &archive)?;
            let _ = run_wine_process_wait(app, bottle, None, &["wineboot", "-u"]);
            Ok(())
        }
        DependencyAction::WineDlls { dlls } => {
            emit_install_progress(
                app,
                "Steam",
                &format!("Step {step}/{total}: Installing {}", recipe.name),
                Some(step),
                Some(total),
                Some(step_percent(step.saturating_sub(1), total)),
            );
            install_wine_bundled_dlls(app, bottle, dlls)?;
            let _ = run_wine_process_wait(app, bottle, None, &["wineboot", "-u"]);
            Ok(())
        }
    }
}

fn dependency_installer_path(
    app: &tauri::AppHandle,
    recipe: &DependencyRecipe,
    step: usize,
    total: usize,
) -> Result<PathBuf, String> {
    let url = recipe
        .url
        .ok_or_else(|| format!("{} does not have a download URL.", recipe.name))?;
    ensure_authorized_dependency_url(url)?;

    let file_name = recipe
        .file_name
        .ok_or_else(|| format!("{} does not define a local file name.", recipe.name))?;
    let dir = installers_dir(app)?.join("Dependencies");
    ensure_dir(&dir)?;
    let destination = dir.join(file_name);

    if !destination.exists() {
        emit_install_progress(
            app,
            "Steam",
            &format!("Step {step}/{total}: Downloading {}", recipe.name),
            Some(step),
            Some(total),
            Some(step_percent(step.saturating_sub(1), total)),
        );
        download_file(url, &destination)?;
    } else {
        emit_install_progress(
            app,
            "Steam",
            &format!("Step {step}/{total}: Using cached {}", recipe.name),
            Some(step),
            Some(total),
            Some(step_percent(step.saturating_sub(1), total)),
        );
    }

    if let Some(expected) = recipe.sha256 {
        verify_sha256(&destination, expected)?;
    }

    Ok(destination)
}

fn step_percent(completed_steps: usize, total_steps: usize) -> u8 {
    if total_steps == 0 {
        return 0;
    }

    ((completed_steps.saturating_mul(100)) / total_steps).min(100) as u8
}

fn ensure_authorized_dependency_url(url: &str) -> Result<(), String> {
    let allowed = [
        "https://aka.ms/",
        "https://cdn.fastly.steamstatic.com/",
        "https://dl.winehq.org/",
        "https://github.com/adobe-fonts/",
    ];

    if allowed.iter().any(|prefix| url.starts_with(prefix)) {
        Ok(())
    } else {
        Err(format!("Refusing unauthorized dependency source: {url}"))
    }
}

fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .map_err(|error| error.to_string())?;

    if !output.status.success() {
        return Err(format!("Could not verify checksum for {}", path.display()));
    }

    let actual = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();

    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "Checksum mismatch for {}: expected {expected}, got {actual}",
            path.display()
        ))
    }
}

fn install_font_zip(
    app: &tauri::AppHandle,
    bottle: &Bottle,
    recipe: &DependencyRecipe,
    archive: &Path,
) -> Result<(), String> {
    let extract_dir = installers_dir(app)?
        .join("Dependencies")
        .join("Extracted")
        .join(recipe.id);
    if extract_dir.exists() {
        let _ = fs::remove_dir_all(&extract_dir);
    }
    ensure_dir(&extract_dir)?;

    let status = Command::new("unzip")
        .args(["-q", "-o"])
        .arg(archive)
        .arg("-d")
        .arg(&extract_dir)
        .status()
        .map_err(|error| error.to_string())?;

    if !status.success() {
        return Err(format!("Could not extract {}", archive.display()));
    }

    let fonts_dir = bottle.path.join("drive_c/windows/Fonts");
    ensure_dir(&fonts_dir)?;

    let mut fonts = Vec::new();
    collect_font_files(&extract_dir, &mut fonts);
    if fonts.is_empty() {
        return Err(format!("No fonts found in {}", archive.display()));
    }

    for font in fonts {
        let Some(name) = font.file_name() else {
            continue;
        };
        fs::copy(&font, fonts_dir.join(name)).map_err(|error| error.to_string())?;
    }

    Ok(())
}

fn collect_font_files(dir: &Path, fonts: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_font_files(&path, fonts);
            continue;
        }

        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                ["otf", "ttf", "ttc"]
                    .iter()
                    .any(|allowed| extension.eq_ignore_ascii_case(allowed))
            })
        {
            fonts.push(path);
        }
    }
}

fn install_wine_bundled_dlls(
    app: &tauri::AppHandle,
    bottle: &Bottle,
    dlls: &[&str],
) -> Result<(), String> {
    let engine = select_engine(app, None)?;
    let Some(wine_root) = wine_runtime_root(&engine.executable) else {
        return Err(format!(
            "Could not resolve Wine runtime root from {}",
            engine.executable.display()
        ));
    };

    let pairs = [
        (
            wine_root.join("lib/wine/i386-windows"),
            bottle.path.join("drive_c/windows/syswow64"),
        ),
        (
            wine_root.join("lib/wine/x86_64-windows"),
            bottle.path.join("drive_c/windows/system32"),
        ),
    ];

    let mut copied = 0;
    for (source_dir, destination_dir) in pairs {
        ensure_dir(&destination_dir)?;
        for dll in dlls {
            let source = source_dir.join(dll);
            if !source.exists() {
                append_install_log(
                    app,
                    bottle,
                    &format!("{} was not found in {}.", dll, source_dir.display()),
                );
                continue;
            }
            fs::copy(&source, destination_dir.join(dll)).map_err(|error| error.to_string())?;
            copied += 1;
        }
    }

    if copied == 0 {
        Err("No bundled Wine DLLs were found to copy.".to_string())
    } else {
        set_native_builtin_overrides(app, bottle, dlls)?;
        Ok(())
    }
}

fn wine_runtime_root(wine_executable: &Path) -> Option<PathBuf> {
    let bin_parent = wine_executable.parent()?;
    if bin_parent.file_name().and_then(|name| name.to_str()) == Some("bin") {
        return bin_parent.parent().map(Path::to_path_buf);
    }

    for ancestor in wine_executable.ancestors() {
        if ancestor.join("lib/wine/i386-windows").is_dir()
            || ancestor.join("lib/wine/x86_64-windows").is_dir()
        {
            return Some(ancestor.to_path_buf());
        }
    }

    None
}

fn set_native_builtin_overrides(
    app: &tauri::AppHandle,
    bottle: &Bottle,
    dlls: &[&str],
) -> Result<(), String> {
    for dll in dlls {
        let Some(name) = dll.strip_suffix(".dll") else {
            continue;
        };
        let override_value =
            format!(r#"HKEY_CURRENT_USER\Software\Wine\DllOverrides\{name}=native,builtin"#);
        run_wine_process_wait(
            app,
            bottle,
            None,
            &[
                "reg",
                "add",
                r#"HKCU\Software\Wine\DllOverrides"#,
                "/v",
                name,
                "/d",
                "native,builtin",
                "/f",
            ],
        )
        .or_else(|error| {
            append_install_log(
                app,
                bottle,
                &format!("Could not set DLL override {override_value}: {error}"),
            );
            Ok::<(), String>(())
        })?;
    }

    Ok(())
}

fn dependency_marker(bottle: &Bottle, id: &str) -> PathBuf {
    bottle
        .path
        .join("whine-dependencies")
        .join(format!("{id}.ready"))
}

fn mark_dependency_ready(bottle: &Bottle, id: &str) -> Result<(), String> {
    let marker = dependency_marker(bottle, id);
    ensure_dir(
        marker
            .parent()
            .ok_or_else(|| "Could not resolve dependency marker directory".to_string())?,
    )?;
    fs::write(marker, chrono_like_timestamp().to_string()).map_err(|error| error.to_string())
}

fn emit_install_progress(
    app: &tauri::AppHandle,
    app_name: &str,
    message: &str,
    step_index: Option<usize>,
    step_total: Option<usize>,
    percent: Option<u8>,
) {
    let _ = app.emit(
        "install-progress",
        InstallProgress {
            app_name: app_name.to_string(),
            message: message.to_string(),
            step_index,
            step_total,
            percent,
        },
    );
}

fn append_install_log(app: &tauri::AppHandle, bottle: &Bottle, message: &str) {
    let Ok(logs_dir) = logs_dir(app) else {
        return;
    };
    if ensure_dir(&logs_dir).is_err() {
        return;
    }

    let line = format!("[{}] {}\n", chrono_like_timestamp(), message);
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs_dir.join(format!("{}-install.log", slug(&bottle.name))))
        .and_then(|mut file| std::io::Write::write_all(&mut file, line.as_bytes()));
}

fn steam_command() -> String {
    "C:\\Program Files (x86)\\Steam\\steam.exe".to_string()
}

fn steam_exe(bottle: &Bottle) -> PathBuf {
    bottle
        .path
        .join("drive_c/Program Files (x86)/Steam/steam.exe")
}

fn steam_web_cache_paths(bottle: &Bottle) -> Vec<PathBuf> {
    let local_app_data = bottle
        .path
        .join("drive_c/users/steamuser/AppData/Local/Steam");
    let steam_dir = bottle.path.join("drive_c/Program Files (x86)/Steam");
    vec![
        local_app_data.join("htmlcache"),
        local_app_data.join("browsercache"),
        local_app_data.join("steamwebhelper"),
        steam_dir.join("config/htmlcache"),
        steam_dir.join("config/browsercache"),
        steam_dir.join("appcache/httpcache"),
    ]
}

fn steam_is_running() -> bool {
    steam_is_running_in(&process_list())
}

fn steam_is_running_fresh() -> bool {
    steam_is_running_in(&process_list_fresh())
}

fn steam_is_running_in(process_list: &[String]) -> bool {
    let patterns = ["steam.exe", "steamwebhelper.exe"];
    process_list
        .iter()
        .any(|process| patterns.iter().any(|pattern| process.contains(pattern)))
}

fn steamapps_dir(bottle: &Bottle) -> PathBuf {
    bottle
        .path
        .join("drive_c/Program Files (x86)/Steam/steamapps")
}

fn scan_steam_games(bottle: &Bottle) -> Vec<InstalledSteamGame> {
    let signature = steam_library_signature(bottle);
    let cache = STEAM_LIBRARY_CACHE.get_or_init(|| Mutex::new(None));
    if let Some(cached) = cache.lock().unwrap().as_ref() {
        if cached.bottle_path == bottle.path && cached.signature == signature {
            return cached.games.clone();
        }
    }

    let mut games = Vec::new();

    for steamapps in &signature.steamapps_dirs {
        let Ok(entries) = fs::read_dir(steamapps) else {
            continue;
        };

        for manifest in entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("appmanifest_") && name.ends_with(".acf"))
            })
        {
            if let Some(game) = parse_app_manifest(&manifest) {
                if !games
                    .iter()
                    .any(|existing: &InstalledSteamGame| existing.app_id == game.app_id)
                {
                    games.push(game);
                }
            }
        }
    }

    games.sort_by_key(|game| game.name.to_lowercase());
    *cache.lock().unwrap() = Some(CachedSteamLibrary {
        bottle_path: bottle.path.clone(),
        signature,
        games: games.clone(),
    });
    games
}

fn steam_library_signature(bottle: &Bottle) -> SteamLibrarySignature {
    let mut steamapps_dirs = vec![steamapps_dir(bottle)];
    steamapps_dirs.extend(extra_steam_library_dirs(bottle));
    steamapps_dirs.sort();
    steamapps_dirs.dedup();

    let mut manifests = Vec::new();
    for steamapps in &steamapps_dirs {
        let Ok(entries) = fs::read_dir(steamapps) else {
            continue;
        };

        manifests.extend(
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| {
                            name.starts_with("appmanifest_") && name.ends_with(".acf")
                        })
                })
                .map(|path| {
                    let modified = fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok();
                    (path, modified)
                }),
        );
    }
    manifests.sort_by(|left, right| left.0.cmp(&right.0));

    SteamLibrarySignature {
        steamapps_dirs,
        manifests,
    }
}

fn steam_game_name(bottle: &Bottle, app_id: u64) -> Option<String> {
    scan_steam_games(bottle)
        .into_iter()
        .find(|game| game.app_id == app_id)
        .map(|game| game.name)
}

fn app_is_running(app: &InstalledApp, process_list: &[String]) -> bool {
    let command_name = windows_executable_name(&app.command);
    let normalized_name = normalize_process_token(&app.name);

    process_list.iter().any(|process| {
        command_name
            .as_ref()
            .is_some_and(|name| process.contains(name))
            || (!normalized_name.is_empty() && process.contains(&normalized_name))
    })
}

fn steam_game_is_running(
    bottle: &Bottle,
    game: &InstalledSteamGame,
    process_list: &[String],
) -> bool {
    let Some(install_dir) = game.install_dir.as_ref() else {
        return false;
    };

    let game_dir = steamapps_dir(bottle).join("common").join(install_dir);
    let normalized_game_dir = game_dir.to_string_lossy().to_lowercase();
    if process_list
        .iter()
        .any(|process| process.contains(&normalized_game_dir))
    {
        return true;
    }

    let executable_names = steam_game_executables(&game_dir);
    !executable_names.is_empty()
        && process_list
            .iter()
            .any(|process| executable_names.iter().any(|name| process.contains(name)))
}

fn steam_game_executables(game_dir: &Path) -> Vec<String> {
    let cache = STEAM_EXECUTABLE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(names) = cache.lock().unwrap().get(game_dir) {
        return names.clone();
    }

    let mut names = Vec::new();
    collect_executables(game_dir, 0, &mut names);

    cache
        .lock()
        .unwrap()
        .insert(game_dir.to_path_buf(), names.clone());

    names
}

fn collect_executables(dir: &Path, depth: usize, names: &mut Vec<String>) {
    if depth > 3 || names.len() >= 80 {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_executables(&path, depth + 1, names);
            continue;
        }

        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                names.push(name.to_lowercase());
            }
        }
    }
}

fn windows_executable_name(command: &str) -> Option<String> {
    command
        .replace('\\', "/")
        .split('/')
        .next_back()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| name.trim_matches('"').to_lowercase())
}

fn process_list() -> Vec<String> {
    let cache = PROCESS_LIST_CACHE.get_or_init(|| Mutex::new(None));
    if let Some(cached) = cache.lock().unwrap().as_ref() {
        if cached.captured_at.elapsed() < PROCESS_LIST_TTL {
            return cached.processes.clone();
        }
    }

    let processes = process_list_fresh();
    *cache.lock().unwrap() = Some(CachedProcessList {
        captured_at: Instant::now(),
        processes: processes.clone(),
    });
    processes
}

fn process_list_fresh() -> Vec<String> {
    Command::new("ps")
        .args(["-axo", "command="])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.to_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_process_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn extra_steam_library_dirs(bottle: &Bottle) -> Vec<PathBuf> {
    let library_file = steamapps_dir(bottle).join("libraryfolders.vdf");
    let Ok(contents) = fs::read_to_string(library_file) else {
        return vec![];
    };

    parse_vdf_pairs(&contents)
        .into_iter()
        .filter(|(key, _)| key == "path")
        .map(|(_, value)| windows_path_to_prefix_path(bottle, &value).join("steamapps"))
        .collect()
}

fn parse_app_manifest(path: &Path) -> Option<InstalledSteamGame> {
    let contents = fs::read_to_string(path).ok()?;
    let pairs = parse_vdf_pairs(&contents);
    let app_id = pairs
        .iter()
        .find(|(key, _)| key == "appid")
        .and_then(|(_, value)| value.parse::<u64>().ok())?;
    let name = pairs
        .iter()
        .find(|(key, _)| key == "name")
        .map(|(_, value)| value.clone())
        .filter(|value| !value.trim().is_empty())?;
    let install_dir = pairs
        .iter()
        .find(|(key, _)| key == "installdir")
        .map(|(_, value)| value.clone());

    Some(InstalledSteamGame {
        app_id,
        name,
        install_dir,
        image: format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg"),
    })
}

fn parse_vdf_pairs(contents: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed == "{" || trimmed == "}" {
            continue;
        }

        let parts = trimmed
            .split('"')
            .filter(|part| !part.trim().is_empty())
            .map(str::trim)
            .collect::<Vec<_>>();

        if parts.len() >= 2 {
            pairs.push((parts[0].to_string(), parts[1].to_string()));
        }
    }

    pairs
}

fn windows_path_to_prefix_path(bottle: &Bottle, value: &str) -> PathBuf {
    let normalized = value.replace('\\', "/");
    if let Some(rest) = normalized.strip_prefix("C:/") {
        bottle.path.join("drive_c").join(rest)
    } else {
        PathBuf::from(normalized)
    }
}

fn parse_steam_game(value: &serde_json::Value) -> Option<SteamGame> {
    let app_id = value.get("id")?.as_u64()?;
    let name = value.get("name")?.as_str()?.to_string();
    let image = value
        .get("tiny_image")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let price = value
        .get("price")
        .and_then(serde_json::Value::as_object)
        .and_then(|price| {
            price
                .get("final")
                .and_then(serde_json::Value::as_u64)
                .map(format_cents)
                .or_else(|| {
                    price
                        .get("final_formatted")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
        });

    Some(SteamGame {
        app_id,
        name,
        image,
        price,
        store_url: format!("https://store.steampowered.com/app/{app_id}"),
    })
}

fn format_cents(value: u64) -> String {
    if value == 0 {
        "Free".to_string()
    } else {
        format!("${}.{:02}", value / 100, value % 100)
    }
}

fn find_bottle(app: &tauri::AppHandle, id: Uuid) -> Result<Bottle, String> {
    list_bottles(app.clone())?
        .into_iter()
        .find(|bottle| bottle.id == id)
        .ok_or_else(|| "Bottle not found".to_string())
}

fn read_bottle_at(path: &Path) -> Result<Bottle, String> {
    let mut bottle = read_bottle(path)?;
    if let Some(parent) = path.parent() {
        if bottle.path != parent {
            bottle.path = parent.to_path_buf();
            let _ = save_bottle(&bottle);
        }
    }
    Ok(bottle)
}

fn read_bottle(path: &Path) -> Result<Bottle, String> {
    let data = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&data).map_err(|error| error.to_string())
}

fn save_bottle(bottle: &Bottle) -> Result<(), String> {
    let data = serde_json::to_string_pretty(bottle).map_err(|error| error.to_string())?;
    fs::write(bottle.path.join("whine.json"), data).map_err(|error| error.to_string())
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| error.to_string())
}

fn default_true() -> bool {
    true
}

fn dxvk_config(settings: &BottleSettings) -> &'static str {
    if settings.vsync {
        "dxgi.syncInterval = 1; d3d9.presentInterval = 1; dxvk.tearFree = True"
    } else {
        "dxgi.syncInterval = 0; d3d9.presentInterval = 0; dxvk.tearFree = False"
    }
}

fn app_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resource_dir()
        .ok()
        .and_then(|resource_dir| resource_dir.parent().map(Path::to_path_buf))
        .or_else(|| env::current_dir().ok())
        .ok_or_else(|| "Could not resolve application root".to_string())
}

fn bottles_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app_data_root(app)?.join("Bottles");
    migrate_legacy_dir(app, "Bottles", &dir)?;
    Ok(dir)
}

fn logs_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app_data_root(app)?.join("Logs");
    migrate_legacy_dir(app, "Logs", &dir)?;
    Ok(dir)
}

fn engines_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app_data_root(app)?.join("Engines");
    migrate_legacy_dir(app, "Engines", &dir)?;
    Ok(dir)
}

fn installers_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app_data_root(app)?.join("Installers");
    migrate_legacy_dir(app, "Installers", &dir)?;
    Ok(dir)
}

fn app_data_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| error.to_string())
        .or_else(|_| app_root(app).map(|root| root.join("Data")))
}

fn migrate_legacy_dir(
    app: &tauri::AppHandle,
    name: &str,
    destination: &Path,
) -> Result<(), String> {
    if destination.exists()
        && fs::read_dir(destination).is_ok_and(|mut entries| entries.next().is_some())
    {
        return Ok(());
    }

    for legacy in legacy_data_dirs(app, name)? {
        if legacy == destination || !legacy.is_dir() {
            continue;
        }

        copy_dir_missing(&legacy, destination)?;
        break;
    }

    Ok(())
}

fn legacy_data_dirs(app: &tauri::AppHandle, name: &str) -> Result<Vec<PathBuf>, String> {
    let mut dirs = vec![app_root(app)?.join(name)];

    if let Ok(executable) = env::current_exe() {
        for ancestor in executable.ancestors() {
            dirs.push(ancestor.join(name));
        }
    }

    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join("Library/Application Support/kirito").join(name));
        dirs.push(home.join("Library/Application Support/Whine").join(name));
        dirs.push(
            home.join("Library/Application Support/app.kirito")
                .join(name),
        );
    }

    dirs.dedup();
    Ok(dirs)
}

fn copy_dir_missing(source: &Path, destination: &Path) -> Result<(), String> {
    ensure_dir(destination)?;

    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_missing(&source_path, &destination_path)?;
        } else if !destination_path.exists() {
            fs::copy(&source_path, &destination_path).map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn apply_graphics_backend_env(
    command: &mut Command,
    engine: &WineEngine,
    settings: &BottleSettings,
) {
    if settings.d3d_metal && engine.d3d_metal_capable {
        if let Some(lib) = d3dmetal_library_dir(&engine.executable) {
            // Load Apple's Wine bridges, even if the prefix has native DXVK DLLs.
            command.env("WINEDLLPATH", lib.join("wine"));
            let inherited = env::var("WINEDLLOVERRIDES").unwrap_or_default();
            command.env(
                "WINEDLLOVERRIDES",
                format!("{inherited};d3d10,d3d10core,d3d11,d3d12,dxgi=b"),
            );
            command.env("WINED3DMETAL", "1");
            command.env("CX_GRAPHICS_BACKEND", "d3dmetal");
            command.env(
                "CX_APPLEGPTK_LIBD3DSHARED_PATH",
                lib.join("external/libd3dshared.dylib"),
            );
            command.env("WHINE_DXVK", "0");
            command.env_remove("DXVK_HUD");
        }
    }
}

fn add_crossover_engines(candidates: &mut Vec<PathBuf>, applications_dir: &Path) {
    let crossover = applications_dir.join("CrossOver.app");
    if !crossover.is_dir() {
        return;
    }

    let shared = crossover.join("Contents/SharedSupport/CrossOver");
    let crossover_candidates = [
        shared.join("CrossOver-Hosted Application/bin/wine"),
        shared.join("CrossOver-Hosted Application/wine"),
        shared.join("bin/wine"),
        shared.join("lib/wine/x86_64-unix/wine"),
        shared.join("lib64/wine/x86_64-unix/wine"),
    ];
    candidates.extend(crossover_candidates);

    if let Ok(entries) = fs::read_dir(shared.join("lib")) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            candidates.push(path.join("wine/x86_64-unix/wine"));
        }
    }
}

fn add_gptk_engines(candidates: &mut Vec<PathBuf>, root: &Path) {
    let gptk_candidates = [
        root.join("opt/game-porting-toolkit/bin/wine64"),
        root.join("opt/game-porting-toolkit/bin/wine"),
        root.join("Cellar/game-porting-toolkit"),
        root.join("game-porting-toolkit/bin/wine64"),
        root.join("game-porting-toolkit/bin/wine"),
    ];

    for path in gptk_candidates {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "game-porting-toolkit")
            && path.is_dir()
        {
            if let Ok(entries) = fs::read_dir(&path) {
                for entry in entries.filter_map(Result::ok) {
                    candidates.push(entry.path().join("bin/wine64"));
                    candidates.push(entry.path().join("bin/wine"));
                }
            }
            continue;
        }
        candidates.push(path);
    }
}

fn d3dmetal_library_dir(path: &Path) -> Option<PathBuf> {
    let path = fs::canonicalize(path).ok()?;
    // Inspect only the Wine tree, not unrelated libraries in parent directories.
    for ancestor in path.parent()?.ancestors().take(5) {
        for relative in ["lib", "lib64", "lib64/apple_gptk", "lib/apple_gptk"] {
            let lib = ancestor.join(relative);
            if [
                "external/D3DMetal.framework/D3DMetal",
                "external/libd3dshared.dylib",
                "wine/x86_64-windows/d3d11.dll",
                "wine/x86_64-windows/d3d12.dll",
                "wine/x86_64-windows/dxgi.dll",
                "wine/x86_64-unix/d3d11.so",
                "wine/x86_64-unix/d3d12.so",
                "wine/x86_64-unix/dxgi.so",
            ]
            .iter()
            .all(|file| lib.join(file).is_file())
            {
                return Some(lib);
            }
        }
        if ancestor
            .extension()
            .is_some_and(|extension| extension == "app")
        {
            break;
        }
    }
    None
}

fn engine_supports_d3dmetal(path: &Path) -> bool {
    is_executable(path) && d3dmetal_library_dir(path).is_some()
}

fn add_app_bundle_engines(candidates: &mut Vec<PathBuf>, applications_dir: &Path) {
    let Ok(entries) = fs::read_dir(applications_dir) else {
        return;
    };

    for app_dir in entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
        })
    {
        candidates.push(app_dir.join("Contents/Resources/wine/bin/wine64"));
        candidates.push(app_dir.join("Contents/Resources/wine/bin/wine"));
        let shared_support = app_dir.join("Contents/SharedSupport");
        let Ok(support_entries) = fs::read_dir(shared_support) else {
            continue;
        };

        for support_dir in support_entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
        {
            candidates.push(support_dir.join("lib/wine/x86_64-unix/wine"));
        }
    }
}

fn engine_name(path: &Path) -> String {
    let value = path.to_string_lossy();
    if value.contains("KiritoD3DMetal") {
        "D3DMetal (kirito)".to_string()
    } else if value.to_ascii_lowercase().contains("crossover") {
        "CrossOver".to_string()
    } else if value.to_ascii_lowercase().contains("game-porting-toolkit")
        || value.to_ascii_lowercase().contains("gptk")
    {
        "Game Porting Toolkit".to_string()
    } else if value.contains("Application Support") && value.contains("Engines/Wine Stable.app") {
        "Wine Stable (kirito)".to_string()
    } else if value.contains(".app/Contents/SharedSupport/") {
        "App bundle".to_string()
    } else if value.contains("Wine Stable.app") {
        "Wine Stable".to_string()
    } else if value.contains("wine-11.0") {
        "Bundled 11.0".to_string()
    } else if value.contains("homebrew") {
        "Homebrew".to_string()
    } else {
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Wine")
            .to_string()
    }
}

fn next_bottle_name(existing: &[Bottle]) -> String {
    let base = "Windows 11";
    if !existing.iter().any(|bottle| bottle.name == base) {
        return base.to_string();
    }

    let mut index = 2;
    loop {
        let candidate = format!("{base} {index}");
        if !existing.iter().any(|bottle| bottle.name == candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;

    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            b' ' => encoded.push('+'),
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

fn chrono_like_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

fn close_to_idle_tray(app: &tauri::AppHandle) {
    hide_main_window(app);
    if steam_client_is_idle(app) {
        stop_all_bottle_processes(app.clone());
    }
}

fn steam_client_is_idle(app: &tauri::AppHandle) -> bool {
    let process_list = process_list_fresh();
    if !steam_is_running_in(&process_list) {
        return false;
    }

    let Ok(bottles) = list_bottles(app.clone()) else {
        return false;
    };

    !bottles
        .iter()
        .filter(|bottle| bottle.name.eq_ignore_ascii_case("steam"))
        .flat_map(scan_steam_games)
        .any(|game| {
            bottles
                .iter()
                .filter(|bottle| bottle.name.eq_ignore_ascii_case("steam"))
                .any(|bottle| steam_game_is_running(bottle, &game, &process_list))
        })
}

fn quit_kirito(app: &tauri::AppHandle) {
    app.exit(0);
}

fn tray_icon_image() -> tauri::image::Image<'static> {
    // Template crossed-swords mark for the macOS menu bar (18×18).
    const SIZE: u32 = 18;
    let mut rgba = vec![0; (SIZE * SIZE * 4) as usize];

    // Main blade: bottom-left → top-right
    for i in 0..=12 {
        let x = 3 + i;
        let y = 14 - i;
        set_tray_pixel(&mut rgba, SIZE, x, y, 255);
        set_tray_pixel(&mut rgba, SIZE, x + 1, y, 220);
        set_tray_pixel(&mut rgba, SIZE, x, y.saturating_sub(1), 180);
    }
    // Crossguard on main blade
    for dx in 0..=3 {
        set_tray_pixel(&mut rgba, SIZE, 5 + dx, 13, 240);
        set_tray_pixel(&mut rgba, SIZE, 5 + dx, 12, 200);
    }
    // Pommel
    set_tray_pixel(&mut rgba, SIZE, 3, 15, 230);
    set_tray_pixel(&mut rgba, SIZE, 2, 15, 180);

    // Second blade: top-left → bottom-right
    for i in 0..=12 {
        let x = 3 + i;
        let y = 3 + i;
        set_tray_pixel(&mut rgba, SIZE, x, y, 255);
        set_tray_pixel(&mut rgba, SIZE, x + 1, y, 200);
        set_tray_pixel(&mut rgba, SIZE, x, y + 1, 160);
    }
    // Crossguard on second blade
    for dx in 0..=3 {
        set_tray_pixel(&mut rgba, SIZE, 4 + dx, 5, 230);
    }
    set_tray_pixel(&mut rgba, SIZE, 3, 3, 210);
    set_tray_pixel(&mut rgba, SIZE, 15, 15, 210);

    tauri::image::Image::new_owned(rgba, SIZE, SIZE)
}

fn set_tray_pixel(rgba: &mut [u8], size: u32, x: u32, y: u32, alpha: u8) {
    if x >= size || y >= size {
        return;
    }
    let index = ((y * size + x) * 4) as usize;
    rgba[index] = 0;
    rgba[index + 1] = 0;
    rgba[index + 2] = 0;
    rgba[index + 3] = alpha;
}

fn setup_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text("show", "Show Kirito")
        .separator()
        .text("quit", "Quit Kirito")
        .build()?;
    TrayIconBuilder::with_id("kirito")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("Kirito")
        .icon(tray_icon_image())
        .icon_as_template(true)
        .on_menu_event(|app, event| match event.id().0.as_str() {
            "show" => show_main_window(app),
            "quit" => quit_kirito(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(&tray.app_handle());
            }
        })
        .build(app)
        .map(|_| ())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            setup_tray(app)?;
            let handle = app.handle().clone();
            let args = env::args().collect::<Vec<_>>();
            thread::spawn(move || {
                let _ = handle_startup_args(handle, &args);
            });
            let engine_handle = app.handle().clone();
            thread::spawn(move || match ensure_engine(engine_handle.clone()) {
                Ok(engines) if !engines.is_empty() => {
                    emit_install_progress(
                        &engine_handle,
                        "Wine",
                        "Wine engine ready",
                        Some(3),
                        Some(3),
                        Some(100),
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    emit_install_progress(&engine_handle, "Wine", &error, None, None, None);
                    eprintln!("kirito: Wine engine setup failed: {error}");
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                close_to_idle_tray(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_bottles,
            list_engines,
            ensure_engine,
            runtime_status,
            create_bottle,
            steam_status,
            install_steam,
            launch_steam,
            apply_steam_compatibility_mode,
            stop_steam,
            repair_steam_web_cache,
            set_bottle_vsync,
            set_app_settings,
            scan_steam_library,
            open_steam_game,
            run_steam_game,
            proton_rating,
            anti_cheat_reports,
            controller_diagnostics,
            search_steam,
            run_wine_tool,
            run_app,
            install_app,
            bedrock_status,
            install_bedrock,
            launch_bedrock,
            open_path,
            open_feedback
        ])
        .build(tauri::generate_context!())
        .expect("error while building kirito")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                stop_all_bottle_processes(app.clone());
            }
        });
}

fn handle_startup_args(app: tauri::AppHandle, args: &[String]) -> Result<(), String> {
    let Some(app_id) = startup_steam_app_id(args) else {
        return Ok(());
    };

    run_steam_game(app, app_id)
}

fn startup_steam_app_id(args: &[String]) -> Option<u64> {
    for pair in args.windows(2) {
        if pair[0] == "--steam-app" {
            return pair[1].parse::<u64>().ok();
        }
    }

    args.iter().find_map(|arg| {
        arg.strip_prefix("kirito://steam/")
            .or_else(|| arg.strip_prefix("kirito:steam:"))
            .and_then(|value| value.parse::<u64>().ok())
    })
}

#[cfg(test)]
mod metal_tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

    struct Runtime(PathBuf);
    impl Runtime {
        fn new() -> Self {
            let root = env::temp_dir().join(format!("kirito-metal-{}", Uuid::new_v4()));
            fs::create_dir_all(root.join("bin")).unwrap();
            fs::write(root.join("bin/wine64"), "#!/bin/sh\n").unwrap();
            fs::set_permissions(root.join("bin/wine64"), fs::Permissions::from_mode(0o755))
                .unwrap();
            Self(root)
        }
        fn metal(&self) {
            for file in [
                "external/D3DMetal.framework/Versions/A/D3DMetal",
                "external/libd3dshared.dylib",
                "wine/x86_64-windows/d3d11.dll",
                "wine/x86_64-windows/d3d12.dll",
                "wine/x86_64-windows/dxgi.dll",
            ] {
                let path = self.0.join("lib").join(file);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, "fixture").unwrap();
            }
            let framework = self.0.join("lib/external/D3DMetal.framework");
            symlink("A", framework.join("Versions/Current")).unwrap();
            symlink("Versions/Current/D3DMetal", framework.join("D3DMetal")).unwrap();
            fs::create_dir_all(self.0.join("lib/wine/x86_64-unix")).unwrap();
            for dll in ["d3d11", "d3d12", "dxgi"] {
                symlink(
                    "../../external/libd3dshared.dylib",
                    self.0.join(format!("lib/wine/x86_64-unix/{dll}.so")),
                )
                .unwrap();
            }
        }
    }
    impl Drop for Runtime {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn capability_requires_complete_runtime_and_resolves_wine_symlink() {
        let runtime = Runtime::new();
        let wine = runtime.0.join("bin/wine64");
        assert!(!engine_supports_d3dmetal(&wine));
        runtime.metal();
        assert!(engine_supports_d3dmetal(&wine));
        let alias = runtime.0.join("gptk");
        symlink(&wine, &alias).unwrap();
        assert!(engine_supports_d3dmetal(&alias));
        fs::remove_file(runtime.0.join("lib/external/libd3dshared.dylib")).unwrap();
        assert!(!engine_supports_d3dmetal(&alias));
    }

    #[test]
    fn runtime_copy_preserves_framework_and_bridge_symlinks() {
        let source = Runtime::new();
        source.metal();
        let destination = Runtime::new();
        copy_dir_recursive(&source.0, &destination.0).unwrap();
        assert_eq!(
            fs::read_link(destination.0.join("lib/wine/x86_64-unix/d3d11.so")).unwrap(),
            PathBuf::from("../../external/libd3dshared.dylib")
        );
        assert!(engine_supports_d3dmetal(&destination.0.join("bin/wine64")));
    }

    #[test]
    fn metal_selects_apple_bridges_without_forcing_msync() {
        let runtime = Runtime::new();
        runtime.metal();
        let engine = WineEngine {
            display_name: "Test".into(),
            executable: runtime.0.join("bin/wine64"),
            d3d_metal_capable: true,
        };
        let mut settings = default_bottle_settings();
        settings.msync = false;
        settings.dxvk = true;
        let mut command = Command::new(&engine.executable);
        command.env("WINEMSYNC", "0");
        apply_graphics_backend_env(&mut command, &engine, &settings);
        let vars: HashMap<_, _> = command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(vars["WINEMSYNC"].as_deref(), Some("0"));
        assert_eq!(vars["WHINE_DXVK"].as_deref(), Some("0"));
        assert!(vars["WINEDLLOVERRIDES"]
            .as_ref()
            .unwrap()
            .ends_with("d3d10,d3d10core,d3d11,d3d12,dxgi=b"));
        assert!(vars["WINEDLLPATH"].as_ref().unwrap().ends_with("/lib/wine"));
    }
}
