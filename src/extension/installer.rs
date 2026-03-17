use std::path::PathBuf;
use std::sync::mpsc;

#[derive(Debug, Clone, PartialEq)]
pub enum InstallStatus {
    Idle,
    Cloning,
    Building,
    Installing,
    Done,
    Failed(String),
}

pub struct InstallJob {
    pub repo_url: String,
    pub status: InstallStatus,
    pub log: Vec<String>,
}

impl InstallJob {
    pub fn new(repo_url: String) -> Self {
        Self {
            repo_url,
            status: InstallStatus::Idle,
            log: Vec::new(),
        }
    }

    /// Run installation in a background thread, sending status updates via a channel.
    pub fn start(repo_url: String, extensions_dir: PathBuf) -> mpsc::Receiver<InstallStatus> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            // 1. Clone
            let _ = tx.send(InstallStatus::Cloning);
            let tmp_dir = match tempdir_for_clone(&repo_url) {
                Ok(d) => d,
                Err(e) => {
                    let _ = tx.send(InstallStatus::Failed(format!("Clone failed: {e}")));
                    return;
                }
            };
            match git2::Repository::clone(&repo_url, &tmp_dir) {
                Ok(_) => {}
                Err(e) => {
                    let _ = tx.send(InstallStatus::Failed(format!("Clone failed: {e}")));
                    return;
                }
            }

            // 2. Read manifest
            let manifest_path = tmp_dir.join("manifest.toml");
            let manifest_str = match std::fs::read_to_string(&manifest_path) {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(InstallStatus::Failed(format!("No manifest.toml: {e}")));
                    return;
                }
            };
            let manifest: super::manifest::ExtensionManifest = match toml::from_str(&manifest_str) {
                Ok(m) => m,
                Err(e) => {
                    let _ = tx.send(InstallStatus::Failed(format!("Invalid manifest: {e}")));
                    return;
                }
            };

            // 3. Build
            let _ = tx.send(InstallStatus::Building);
            let build_result = std::process::Command::new("cargo")
                .args(["build", "--release"])
                .current_dir(&tmp_dir)
                .output();
            match build_result {
                Ok(out) if out.status.success() => {}
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let _ = tx.send(InstallStatus::Failed(format!("Build failed: {stderr}")));
                    return;
                }
                Err(e) => {
                    let _ = tx.send(InstallStatus::Failed(format!("Build error: {e}")));
                    return;
                }
            }

            // 4. Copy artifact + manifest
            let _ = tx.send(InstallStatus::Installing);
            let ext_id = &manifest.extension.id;
            let dest = extensions_dir.join(ext_id);
            if let Err(e) = std::fs::create_dir_all(&dest) {
                let _ = tx.send(InstallStatus::Failed(format!("mkdir failed: {e}")));
                return;
            }

            // Copy manifest
            if let Err(e) = std::fs::copy(&manifest_path, dest.join("manifest.toml")) {
                let _ = tx.send(InstallStatus::Failed(format!("Copy manifest failed: {e}")));
                return;
            }

            // Find and copy .so/.dll
            let release_dir = tmp_dir.join("target").join("release");
            if let Some(lib_file) = find_lib_file(&release_dir) {
                let dest_lib = dest.join(lib_file.file_name().unwrap_or_default());
                if let Err(e) = std::fs::copy(&lib_file, &dest_lib) {
                    let _ = tx.send(InstallStatus::Failed(format!("Copy lib failed: {e}")));
                    return;
                }
            }

            let _ = tx.send(InstallStatus::Done);
        });
        rx
    }
}

/// Install an extension from a local directory.
/// The directory must contain a `manifest.toml`.
/// If it contains a `target/release/lib*.so` (or `.dll` / `.dylib`), use it directly.
/// Otherwise, try to build with `cargo build --release` first.
pub fn install_from_folder(
    folder: PathBuf,
    extensions_dir: PathBuf,
) -> mpsc::Receiver<InstallStatus> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        // 1. Read manifest.toml
        let manifest_path = folder.join("manifest.toml");
        if !manifest_path.exists() {
            let _ = tx.send(InstallStatus::Failed(
                "No manifest.toml found in folder".to_string(),
            ));
            return;
        }

        let manifest_str = match std::fs::read_to_string(&manifest_path) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(InstallStatus::Failed(format!("Cannot read manifest: {e}")));
                return;
            }
        };

        let manifest: super::manifest::ExtensionManifest = match toml::from_str(&manifest_str) {
            Ok(m) => m,
            Err(e) => {
                let _ = tx.send(InstallStatus::Failed(format!("Invalid manifest.toml: {e}")));
                return;
            }
        };

        let ext_id = manifest.extension.id.clone();

        // 2. Look for pre-built library
        let lib_name = folder
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("extension")
            .replace('-', "_");

        let lib_path = if let Some(p) = find_prebuilt_lib(&folder, &lib_name) {
            p
        } else {
            // 3. Build with cargo
            let _ = tx.send(InstallStatus::Building);
            match std::process::Command::new("cargo")
                .args(["build", "--release"])
                .current_dir(&folder)
                .output()
            {
                Ok(out) if out.status.success() => match find_prebuilt_lib(&folder, &lib_name) {
                    Some(p) => p,
                    None => {
                        let _ = tx.send(InstallStatus::Failed(
                            "Build succeeded but no library found".to_string(),
                        ));
                        return;
                    }
                },
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    let _ = tx.send(InstallStatus::Failed(format!(
                        "Build failed: {}",
                        &err[..err.len().min(200)]
                    )));
                    return;
                }
                Err(e) => {
                    let _ = tx.send(InstallStatus::Failed(format!("Cannot run cargo: {e}")));
                    return;
                }
            }
        };

        // 4. Copy to extensions dir
        let _ = tx.send(InstallStatus::Installing);
        let dest_dir = extensions_dir.join(&ext_id);
        if let Err(e) = std::fs::create_dir_all(&dest_dir) {
            let _ = tx.send(InstallStatus::Failed(format!("Cannot create dir: {e}")));
            return;
        }

        let dest_lib = dest_dir.join(lib_path.file_name().unwrap_or_default());
        if let Err(e) = std::fs::copy(&lib_path, &dest_lib) {
            let _ = tx.send(InstallStatus::Failed(format!("Cannot copy library: {e}")));
            return;
        }
        if let Err(e) = std::fs::copy(&manifest_path, dest_dir.join("manifest.toml")) {
            let _ = tx.send(InstallStatus::Failed(format!("Cannot copy manifest: {e}")));
            return;
        }

        let _ = tx.send(InstallStatus::Done);
    });
    rx
}

fn find_prebuilt_lib(folder: &std::path::Path, lib_name: &str) -> Option<PathBuf> {
    let release_dir = folder.join("target").join("release");

    let candidates = [
        format!("lib{lib_name}.so"),    // Linux
        format!("lib{lib_name}.dylib"), // macOS
        format!("{lib_name}.dll"),      // Windows
    ];

    for name in &candidates {
        let path = release_dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn tempdir_for_clone(repo_url: &str) -> anyhow::Result<PathBuf> {
    let name = repo_url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("extension")
        .trim_end_matches(".git")
        .to_string();
    let tmp = std::env::temp_dir().join(format!("wu-ext-{name}-{}", uuid::Uuid::new_v4()));
    Ok(tmp)
}

fn find_lib_file(release_dir: &std::path::Path) -> Option<PathBuf> {
    for entry in std::fs::read_dir(release_dir).ok()?.flatten() {
        let p = entry.path();
        if let Some(ext) = p.extension() {
            if ext == "so" || ext == "dll" || ext == "dylib" {
                return Some(p);
            }
        }
    }
    None
}
