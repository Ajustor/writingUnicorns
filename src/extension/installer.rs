use std::path::PathBuf;
use std::sync::mpsc;

use super::manifest::{ExtensionSource, SourceKind};

fn write_source(dest_dir: &std::path::Path, source: &ExtensionSource) {
    if let Ok(toml_str) = toml::to_string(source) {
        let _ = std::fs::write(dest_dir.join("source.toml"), toml_str);
    }
}

// ── Workspace installer ───────────────────────────────────────────────────────

/// Status events emitted by `install_from_workspace`.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceStatus {
    Idle,
    /// Running `cargo build --release` on the whole workspace.
    Building,
    /// Copying one module into the extensions directory.
    Installing { current: String, done: usize, total: usize },
    /// Installing an external dependency for a module.
    InstallingDep { module: String, step: String },
    /// One module failed (non-fatal — install continues for the rest).
    ModuleFailed { name: String, reason: String },
    /// All done — `installed` out of `total` modules were installed.
    Done { installed: usize, total: usize },
    /// Fatal error (workspace-level).
    Failed(String),
}

/// Build every member of a Cargo workspace that has a `manifest.toml` and
/// install it into `extensions_dir`.
///
/// Progress is streamed via the returned channel so the UI can update live.
pub fn install_from_workspace(
    workspace_path: PathBuf,
    extensions_dir: PathBuf,
) -> mpsc::Receiver<WorkspaceStatus> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        // 1. Parse workspace Cargo.toml
        let cargo_toml_path = workspace_path.join("Cargo.toml");
        let cargo_toml_str = match std::fs::read_to_string(&cargo_toml_path) {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(WorkspaceStatus::Failed(format!("Cannot read Cargo.toml: {e}")));
                return;
            }
        };
        let members = match parse_workspace_members(&cargo_toml_str) {
            Ok(m) => m,
            Err(e) => {
                let _ = tx.send(WorkspaceStatus::Failed(format!("Invalid workspace: {e}")));
                return;
            }
        };

        // 2. Keep only members that have a manifest.toml
        let modules: Vec<String> = members
            .into_iter()
            .filter(|m| workspace_path.join(m).join("manifest.toml").exists())
            .collect();

        if modules.is_empty() {
            let _ = tx.send(WorkspaceStatus::Failed(
                "No modules with manifest.toml found in this workspace.".to_string(),
            ));
            return;
        }

        // 3. Build the whole workspace once
        let _ = tx.send(WorkspaceStatus::Building);
        let build_out = std::process::Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&workspace_path)
            .output();

        match build_out {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                let err: String = String::from_utf8_lossy(&out.stderr).chars().take(400).collect();
                let _ = tx.send(WorkspaceStatus::Failed(format!("Build failed:\n{err}")));
                return;
            }
            Err(e) => {
                let _ = tx.send(WorkspaceStatus::Failed(format!("Cannot run cargo: {e}")));
                return;
            }
        }

        // 4. Install each module
        let total = modules.len();
        let release_dir = workspace_path.join("target").join("release");
        let mut installed = 0;

        for (i, member) in modules.iter().enumerate() {
            let _ = tx.send(WorkspaceStatus::Installing {
                current: member.clone(),
                done: i,
                total,
            });

            let member_path = workspace_path.join(member);
            let manifest_path = member_path.join("manifest.toml");

            // Read manifest
            let manifest_str = match std::fs::read_to_string(&manifest_path) {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(WorkspaceStatus::ModuleFailed {
                        name: member.clone(),
                        reason: format!("manifest.toml unreadable: {e}"),
                    });
                    continue;
                }
            };
            let manifest: super::manifest::ExtensionManifest = match toml::from_str(&manifest_str) {
                Ok(m) => m,
                Err(e) => {
                    let _ = tx.send(WorkspaceStatus::ModuleFailed {
                        name: member.clone(),
                        reason: format!("Invalid manifest: {e}"),
                    });
                    continue;
                }
            };

            // Find the compiled library in the shared workspace target/release/
            let lib_name = member.replace('-', "_");
            let lib_path = match find_lib_in_release_dir(&release_dir, &lib_name) {
                Some(p) => p,
                None => {
                    let _ = tx.send(WorkspaceStatus::ModuleFailed {
                        name: member.clone(),
                        reason: format!("lib{lib_name}.so not found in target/release"),
                    });
                    continue;
                }
            };

            // Copy lib + manifest to extensions dir
            let dest_dir = extensions_dir.join(&manifest.extension.id);
            if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                let _ = tx.send(WorkspaceStatus::ModuleFailed {
                    name: member.clone(),
                    reason: format!("mkdir: {e}"),
                });
                continue;
            }
            if let Err(e) = std::fs::copy(
                &lib_path,
                dest_dir.join(lib_path.file_name().unwrap_or_default()),
            ) {
                let _ = tx.send(WorkspaceStatus::ModuleFailed {
                    name: member.clone(),
                    reason: format!("Copy library: {e}"),
                });
                continue;
            }
            if let Err(e) = std::fs::copy(&manifest_path, dest_dir.join("manifest.toml")) {
                let _ = tx.send(WorkspaceStatus::ModuleFailed {
                    name: member.clone(),
                    reason: format!("Copy manifest: {e}"),
                });
                continue;
            }
            write_source(&dest_dir, &ExtensionSource {
                kind: SourceKind::Workspace,
                path: Some(workspace_path.to_string_lossy().to_string()),
                member: Some(member.clone()),
                url: None,
            });

            // Install external dependencies declared in the manifest.
            let member_name = member.clone();
            let dep_errors = install_deps(&manifest.dependencies, |step| {
                let _ = tx.send(WorkspaceStatus::InstallingDep {
                    module: member_name.clone(),
                    step,
                });
            });
            for err in dep_errors {
                let _ = tx.send(WorkspaceStatus::ModuleFailed {
                    name: member.clone(),
                    reason: format!("dependency error: {err}"),
                });
            }

            installed += 1;
        }

        let _ = tx.send(WorkspaceStatus::Done { installed, total });
    });
    rx
}

/// Parse `[workspace].members` from a Cargo.toml string.
fn parse_workspace_members(cargo_toml: &str) -> anyhow::Result<Vec<String>> {
    let value: toml::Value = toml::from_str(cargo_toml)?;
    let members = value
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .ok_or_else(|| anyhow::anyhow!("No [workspace].members found"))?;
    Ok(members
        .iter()
        .filter_map(|m| m.as_str().map(|s| s.to_string()))
        .collect())
}

/// Find `lib{name}.so` / `.dylib` / `.dll` in a release directory.
fn find_lib_in_release_dir(release_dir: &std::path::Path, lib_name: &str) -> Option<PathBuf> {
    let candidates = [
        format!("lib{lib_name}.so"),
        format!("lib{lib_name}.dylib"),
        format!("{lib_name}.dll"),
    ];
    for name in &candidates {
        let path = release_dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstallStatus {
    Idle,
    Cloning,
    Building,
    Installing,
    /// Installing an external dependency (e.g. "npm install -g pylsp").
    InstallingDep(String),
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

            write_source(&dest, &ExtensionSource {
                kind: SourceKind::Git,
                url: Some(repo_url.clone()),
                path: None,
                member: None,
            });

            // 5. Install external dependencies.
            install_deps(&manifest.dependencies, |step| {
                let _ = tx.send(InstallStatus::InstallingDep(step));
            });

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
                    let truncated: String = err.chars().take(200).collect();
                    let _ = tx.send(InstallStatus::Failed(format!(
                        "Build failed: {truncated}"
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
        write_source(&dest_dir, &ExtensionSource {
            kind: SourceKind::Folder,
            path: Some(folder.to_string_lossy().to_string()),
            member: None,
            url: None,
        });

        // Install external dependencies.
        install_deps(&manifest.dependencies, |step| {
            let _ = tx.send(InstallStatus::InstallingDep(step));
        });

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

/// Install all external dependencies declared in a module's manifest.
/// Calls `progress(step_description)` before each command.
/// Returns a list of error strings (non-fatal — the caller decides what to do).
fn install_deps(
    deps: &super::manifest::Dependencies,
    mut progress: impl FnMut(String),
) -> Vec<String> {
    let mut errors = Vec::new();

    // npm packages
    if !deps.npm.is_empty() {
        let step = format!("npm install -g {}", deps.npm.join(" "));
        progress(step.clone());
        let mut cmd = std::process::Command::new("npm");
        cmd.arg("install").arg("-g");
        for pkg in &deps.npm {
            cmd.arg(pkg);
        }
        match cmd.output() {
            Ok(out) if out.status.success() => {}
            Ok(out) => {
                errors.push(format!(
                    "npm: {}",
                    String::from_utf8_lossy(&out.stderr).chars().take(200).collect::<String>()
                ));
            }
            Err(e) => errors.push(format!("npm not found: {e}")),
        }
    }

    // pip packages
    if !deps.pip.is_empty() {
        for pkg in &deps.pip {
            let step = format!("pip3 install {pkg}");
            progress(step.clone());
            match std::process::Command::new("pip3")
                .args(["install", pkg])
                .output()
            {
                Ok(out) if out.status.success() => {}
                Ok(out) => errors.push(format!(
                    "pip3 {pkg}: {}",
                    String::from_utf8_lossy(&out.stderr).chars().take(200).collect::<String>()
                )),
                Err(e) => errors.push(format!("pip3 not found: {e}")),
            }
        }
    }

    // cargo packages
    if !deps.cargo.is_empty() {
        for pkg in &deps.cargo {
            let step = format!("cargo install {pkg}");
            progress(step.clone());
            match std::process::Command::new("cargo")
                .args(["install", pkg])
                .output()
            {
                Ok(out) if out.status.success() => {}
                Ok(out) => errors.push(format!(
                    "cargo install {pkg}: {}",
                    String::from_utf8_lossy(&out.stderr).chars().take(200).collect::<String>()
                )),
                Err(e) => errors.push(format!("cargo not found: {e}")),
            }
        }
    }

    // go packages
    if !deps.go.is_empty() {
        for pkg in &deps.go {
            let step = format!("go install {pkg}");
            progress(step.clone());
            match std::process::Command::new("go")
                .args(["install", pkg])
                .env("GOPATH", {
                    // Prefer $GOPATH, fall back to ~/go
                    std::env::var("GOPATH").unwrap_or_else(|_| {
                        dirs_next::home_dir()
                            .unwrap_or_else(|| PathBuf::from("."))
                            .join("go")
                            .to_string_lossy()
                            .to_string()
                    })
                })
                .output()
            {
                Ok(out) if out.status.success() => {}
                Ok(out) => errors.push(format!(
                    "go install {pkg}: {}",
                    String::from_utf8_lossy(&out.stderr).chars().take(200).collect::<String>()
                )),
                Err(e) => errors.push(format!("go not found: {e}")),
            }
        }
    }

    errors
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
