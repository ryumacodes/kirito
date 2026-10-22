//! Experimental PC Bedrock integration. Keep GDK Wine isolated from other bottles.
use super::*;
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;

const RELEASE_URL: &str = "https://github.com/amha0270/Minecraft-Bedrock-GDK-Launcher/releases/download/BETA/Minecraft-Bedrock-Launcher_MacOS_v0.1.0b.zip";
const RELEASE_SHA256: &str = "10165c17c992547b95dd873256a9c9fb4b7ed1c590c87b1c1366bc2d17be74d1";
const RELEASE_DIR: &str = "Minecraft-Bedrock-Launcher_MacOS_v0.1.0b";
const PRODUCT: &str = "9NBLGGH2JHXJ";
pub const APP_COMMAND: &str = "kirito:bedrock";
static INSTALL_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    runtime_ready: bool,
    installed: bool,
}

fn runtime(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(engines_dir(app)?.join("BedrockGDK"))
}
fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_root(app)?.join("Bedrock"))
}
fn prefix(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(bottles_dir(app)?.join("minecraft-bedrock"))
}
fn valid_runtime(root: &Path) -> bool {
    is_executable(&root.join("dist/bin/wine"))
        && is_executable(&root.join("dist/bin/xodus-cli"))
        && root
            .join("dist/lib/wine/x86_64-windows/xgameruntime.dll")
            .is_file()
        && root
            .join("d3dmetal/gptk/external/D3DMetal.framework/D3DMetal")
            .is_file()
}
fn ready_game(root: &Path) -> bool {
    root.join("game/Minecraft.Windows.exe").is_file() && root.join("game/.kirito-ready").is_file()
}
pub fn status(app: &tauri::AppHandle) -> Result<Status, String> {
    Ok(Status {
        runtime_ready: valid_runtime(&runtime(app)?),
        installed: ready_game(&root(app)?) && prefix(app)?.join("whine.json").is_file(),
    })
}
fn progress(app: &tauri::AppHandle, message: &str, percent: u64) {
    emit_install_progress(
        app,
        "Minecraft Bedrock",
        message,
        None,
        None,
        Some(percent.min(100) as u8),
    );
}

pub fn prepare(app: &tauri::AppHandle) -> Result<(), String> {
    let destination = runtime(app)?;
    if valid_runtime(&destination) {
        return Ok(());
    }
    let downloads = root(app)?;
    ensure_dir(&downloads)?;
    let archive = downloads.join("runtime-0.1.0b.zip");
    progress(app, "Downloading experimental Bedrock runtime…", 5);
    if !archive.is_file() || verify_sha256(&archive, RELEASE_SHA256).is_err() {
        download_file(RELEASE_URL, &archive)?;
    }
    verify_sha256(&archive, RELEASE_SHA256)?;
    let stage = downloads.join(format!("runtime-stage-{}", Uuid::new_v4()));
    ensure_dir(&stage)?;
    let result = (|| {
        let output = Command::new("/usr/bin/ditto")
            .args(["-xk"])
            .arg(&archive)
            .arg(&stage)
            .status()
            .map_err(|error| error.to_string())?;
        let unpacked = stage.join(RELEASE_DIR);
        if !output.success() || !valid_runtime(&unpacked) {
            return Err("Bedrock runtime extraction failed validation.".to_string());
        }
        if destination.exists() {
            return Err("An incomplete Bedrock runtime already exists. Move Engines/BedrockGDK aside and retry.".to_string());
        }
        fs::rename(unpacked, &destination).map_err(|error| error.to_string())?;
        Ok(())
    })();
    let _ = fs::remove_dir_all(stage);
    result
}

fn xodus(app: &tauri::AppHandle) -> Result<Command, String> {
    let mut command = Command::new(runtime(app)?.join("dist/bin/xodus-cli"));
    command.env("RUST_LOG", "error");
    Ok(command)
}
fn signed_in(app: &tauri::AppHandle) -> Result<bool, String> {
    let output = xodus(app)?
        .arg("status")
        .output()
        .map_err(|error| error.to_string())?;
    Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).contains("Signed in as"))
}
fn checked(mut command: Command, phase: &str) -> Result<(), String> {
    // Authentication and license responses can contain secrets: do not surface them in UI/logs.
    let output = command
        .output()
        .map_err(|error| format!("{phase}: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{phase} failed ({}). Retry from Minecraft Bedrock in Install.",
            output.status
        ))
    }
}

pub fn install(app: &tauri::AppHandle) -> Result<(), String> {
    let _guard = INSTALL_LOCK
        .try_lock()
        .map_err(|_| "Bedrock setup is already running.")?;
    prepare(app)?;
    if !signed_in(app)? {
        progress(
            app,
            "Sign in to the Microsoft account that owns Minecraft in the Xodus window.",
            15,
        );
        let mut login = xodus(app)?;
        login.arg("login");
        checked(login, "Microsoft sign-in")?;
        if !signed_in(app)? {
            return Err("Microsoft sign-in was cancelled or did not finish. Your PC Minecraft purchase is required.".into());
        }
    }
    let data = root(app)?;
    if !ready_game(&data) {
        let stage = data.join("game-download");
        ensure_dir(&stage)?;
        progress(app, "Downloading your Minecraft for Windows purchase…", 20);
        let mut command = xodus(app)?;
        command
            .args(["streaming", PRODUCT])
            .arg(&stage)
            .args(["--market", "neutral", "--numbers"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().map_err(|error| error.to_string())?;
        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(value) = line.trim().parse::<u64>() {
                    progress(
                        app,
                        "Downloading Minecraft for Windows…",
                        20 + value.min(100) * 50 / 100,
                    );
                }
            }
        }
        if !child.wait().map_err(|error| error.to_string())?.success() {
            return Err("Minecraft download failed. Check that the signed-in Microsoft account owns Minecraft for Windows and retry.".into());
        }
        progress(app, "Preparing licensed game files…", 75);
        let decrypted = data.join("licensed-executable");
        ensure_dir(&decrypted)?;
        let mut decrypt = xodus(app)?;
        decrypt
            .arg("decrypt")
            .arg(&stage)
            .arg(&decrypted)
            .args(["--market", "neutral"]);
        checked(decrypt, "Preparing licensed game executable")?;
        let exe = decrypted.join("Minecraft.Windows.exe");
        if !exe.is_file() {
            return Err(
                "The licensed Bedrock executable was not produced; installation is incomplete."
                    .into(),
            );
        }
        fs::copy(exe, stage.join("Minecraft.Windows.exe")).map_err(|error| error.to_string())?;
        fs::write(stage.join(".kirito-ready"), PRODUCT).map_err(|error| error.to_string())?;
        let game = data.join("game");
        if game.exists() {
            return Err(
                "An existing Bedrock game folder needs inspection before replacement.".into(),
            );
        }
        fs::rename(stage, game).map_err(|error| error.to_string())?;
    }
    progress(app, "Preparing the dedicated Bedrock bottle…", 85);
    let bottle_path = prefix(app)?;
    ensure_dir(&bottle_path)?;
    if !bottle_path.join("system.reg").exists() {
        let mut boot = wine_command(app)?;
        boot.args(["wineboot", "-u"])
            .env("WINEDLLOVERRIDES", "mscoree,mshtml=");
        checked(boot, "Bedrock bottle setup")?;
    }
    for file in [
        "GameInputBridge.dll",
        "GameInputRawInputProxy.exe",
        "GameInputRedist.dll",
        "GameInputRedistService.exe",
    ] {
        fs::copy(
            runtime(app)?.join("dist/x64").join(file),
            bottle_path.join("drive_c/windows/system32").join(file),
        )
        .map_err(|error| format!("GameInput setup: {error}"))?;
    }
    let mut bottle = if bottle_path.join("whine.json").exists() {
        read_bottle_at(&bottle_path.join("whine.json"))?
    } else {
        Bottle {
            id: Uuid::new_v4(),
            name: "Minecraft Bedrock".into(),
            path: bottle_path,
            windows_version: "Windows 10".into(),
            settings: default_bottle_settings(),
            app_settings: HashMap::new(),
            apps: vec![],
        }
    };
    if !bottle.apps.iter().any(|app| app.command == APP_COMMAND) {
        bottle.apps.push(InstalledApp {
            id: Uuid::new_v4(),
            name: "Minecraft Bedrock".into(),
            command: APP_COMMAND.into(),
        });
    }
    save_bottle(&bottle)?;
    progress(
        app,
        "Bedrock installed. Choose Play; Xbox sign-in may be required in game.",
        100,
    );
    Ok(())
}
fn wine_command(app: &tauri::AppHandle) -> Result<Command, String> {
    let runtime = runtime(app)?;
    let mut command = Command::new(runtime.join("dist/bin/wine"));
    command
        .env("WINEPREFIX", prefix(app)?)
        .env("GRAPHICS_BACKEND", "d3dmetal")
        .env("D3DMETAL_RUNTIME_DIR", runtime.join("d3dmetal/gptk"))
        .env("D3DMETAL_UPSCALER_PROFILE", "amd")
        .env("D3DM_VENDOR_ID", "4098")
        .env("D3DM_DEVICE_ID", "29631")
        .env("D3DM_DEVICE_DESCRIPTION", "AMD Radeon RX 6800 XT")
        .env(
            "WINEDLLOVERRIDES",
            "cryptbase=n,b;vrclient=;vrclient_x64=;openvr_api=;wineopenxr=;amd_ags_x64=",
        )
        // WineGDK reports its interactive Xbox device code on this channel.
        .env("WINEDEBUG", "-all,+gdkc");
    Ok(command)
}
pub fn launch(app: &tauri::AppHandle) -> Result<(), String> {
    if !status(app)?.installed
        || !valid_runtime(&runtime(app)?)
        || !prefix(app)?.join("whine.json").is_file()
    {
        return Err("Finish Minecraft Bedrock setup from the Install catalog first.".into());
    }
    let game = root(app)?.join("game");
    let mut command = wine_command(app)?;
    command
        .current_dir(&game)
        .arg(game.join("Minecraft.Windows.exe"));
    let logs = logs_dir(app)?;
    ensure_dir(&logs)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join("minecraft-bedrock.log"))
        .map_err(|error| error.to_string())?;
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let output = child.stdout.take().unwrap();
    let errors = child.stderr.take().unwrap();
    let log = std::sync::Arc::new(Mutex::new(file));
    fn monitor(
        stream: impl std::io::Read + Send + 'static,
        app: tauri::AppHandle,
        log: std::sync::Arc<Mutex<fs::File>>,
    ) {
        thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if let Ok(mut file) = log.lock() {
                    let _ = writeln!(file, "{line}");
                }
                if let Some(code) = xbox_sign_in_code(&line) {
                    let _ = app.emit("bedrock-status", format!(
                        "Xbox sign-in required: open https://www.microsoft.com/link and enter {code}."));
                }
            }
        });
    }
    monitor(output, app.clone(), log.clone());
    monitor(errors, app.clone(), log);
    let app = app.clone();
    thread::spawn(move || {
        if let Ok(status) = child.wait() {
            let _ = app.emit(
                "bedrock-status",
                format!("Minecraft Bedrock exited ({status})."),
            );
        }
    });
    Ok(())
}

fn xbox_sign_in_code(line: &str) -> Option<&str> {
    if !line.contains("https://www.microsoft.com/link") {
        return None;
    }
    let code = line
        .split("enter code ")
        .nth(1)?
        .split_whitespace()
        .next()?;
    (code.len() >= 6 && code.len() <= 12 && code.bytes().all(|b| b.is_ascii_alphanumeric()))
        .then_some(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xbox_prompt_is_recognized_without_accepting_arbitrary_output() {
        assert_eq!(
            xbox_sign_in_code(
                "WineGDK open https://www.microsoft.com/link and enter code ABCD1234"
            ),
            Some("ABCD1234")
        );
        assert_eq!(xbox_sign_in_code("enter code ABCD1234"), None);
        assert_eq!(
            xbox_sign_in_code("https://www.microsoft.com/link enter code <invalid>"),
            None
        );
    }
    #[test]
    fn partial_download_is_not_playable() {
        let root = env::temp_dir().join(format!("kirito-bedrock-test-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("game")).unwrap();
        fs::write(root.join("game/Minecraft.Windows.exe"), "partial").unwrap();
        assert!(!ready_game(&root));
        fs::write(root.join("game/.kirito-ready"), PRODUCT).unwrap();
        assert!(ready_game(&root));
        fs::remove_file(root.join("game/Minecraft.Windows.exe")).unwrap();
        assert!(!ready_game(&root));
        fs::remove_dir_all(root).unwrap();
    }
}
