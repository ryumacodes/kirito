use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let engines = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../Engines");
    let archive = engines.join("metal-runtime.tar.gz");
    let checksum = engines.join("metal-runtime.sha256");
    println!("cargo:rerun-if-changed={}", archive.display());
    println!("cargo:rerun-if-changed={}", checksum.display());
    let metadata = fs::metadata(&archive)
        .expect("Metal runtime missing: install Git LFS and run git lfs pull");
    assert!(
        metadata.len() > 1024,
        "Metal runtime is an LFS pointer: run git lfs install && git lfs pull before building"
    );
    let expected = fs::read_to_string(checksum).expect("Metal checksum missing");
    let result = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(&archive)
        .output()
        .expect("could not verify Metal runtime");
    assert!(
        result.status.success(),
        "Metal runtime checksum command failed"
    );
    let actual = String::from_utf8_lossy(&result.stdout);
    assert_eq!(
        actual.split_whitespace().next(),
        expected.split_whitespace().next(),
        "Metal runtime checksum mismatch; restore it with git lfs pull"
    );
    // Preserve Wine/framework symlinks inside the archive; Tauri copies it intact.
    tauri_build::build()
}
