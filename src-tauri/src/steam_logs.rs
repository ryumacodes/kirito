//! Discard Chromium's unbounded diagnostic stream, not Steam game/account data.
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};
static WATCHED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

pub fn protect(bottle: &Path) -> Result<(), String> {
    let logs = bottle.join("drive_c/Program Files (x86)/Steam/logs");
    let mut watched = WATCHED
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| e.to_string())?;
    if watched.contains(&logs) {
        return Ok(());
    }
    let mut retired = Vec::new();
    discard_cef_logs(&logs, &mut retired)?;
    if watched.insert(logs.clone()) {
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(1));
            // Steam can rotate or recreate its log during updates.
            let _ = discard_cef_logs(&logs, &mut retired);
            for file in &retired {
                let _ = file.set_len(0);
            }
        });
    }
    Ok(())
}

fn discard_cef_logs(logs: &Path, retired: &mut Vec<fs::File>) -> Result<(), String> {
    fs::create_dir_all(logs).map_err(|e| e.to_string())?;
    for name in ["cef_log.txt", "cef_log.previous.txt"] {
        let path = logs.join(name);
        if fs::read_link(&path).ok().as_deref() == Some(Path::new("/dev/null")) {
            continue;
        }
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if meta.is_dir() {
                return Err(format!("Expected a Steam log file: {}", path.display()));
            }
            // Truncate first: an already-open writer otherwise retains the
            // unlinked multi-gigabyte inode until it exits.
            if meta.is_file() {
                let file = fs::OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&path)
                    .map_err(|e| e.to_string())?;
                retired.push(file);
            }
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        std::os::unix::fs::symlink("/dev/null", &path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn discards_flood_and_reprotects_rotated_log() {
        let dir = std::env::temp_dir().join(format!("kirito-cef-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let mut retired = Vec::new();
        let log = dir.join("cef_log.txt");
        fs::write(&log, b"old diagnostics").unwrap();
        let mut old_writer = fs::OpenOptions::new().append(true).open(&log).unwrap();
        discard_cef_logs(&dir, &mut retired).unwrap();
        assert_eq!(old_writer.metadata().unwrap().len(), 0);
        old_writer.write_all(b"late write").unwrap();
        for file in &retired {
            file.set_len(0).unwrap();
        }
        assert_eq!(old_writer.metadata().unwrap().len(), 0);
        let mut writer = fs::OpenOptions::new().append(true).open(&log).unwrap();
        writer.write_all(&vec![b'x'; 1024 * 1024]).unwrap();
        assert_eq!(fs::metadata(&log).unwrap().len(), 0);
        fs::remove_file(&log).unwrap();
        fs::write(&log, b"recreated by Steam").unwrap();
        discard_cef_logs(&dir, &mut retired).unwrap();
        assert_eq!(fs::read_link(&log).unwrap(), Path::new("/dev/null"));
        drop(old_writer);
        drop(writer);
        fs::remove_dir_all(dir).unwrap();
    }
}
