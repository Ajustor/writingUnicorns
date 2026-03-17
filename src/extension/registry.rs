use super::manifest::ExtensionManifest;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct InstalledExtension {
    pub manifest: ExtensionManifest,
    pub path: PathBuf,
    pub lib_path: Option<PathBuf>,
    pub enabled: bool,
}

pub struct ExtensionRegistry {
    pub installed: Vec<InstalledExtension>,
    pub extensions_dir: PathBuf,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        let extensions_dir = Self::extensions_dir();
        Self {
            installed: Vec::new(),
            extensions_dir,
        }
    }

    pub fn extensions_dir() -> PathBuf {
        dirs_next::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("writing-unicorns")
            .join("extensions")
    }

    pub fn load_installed(&mut self) {
        self.installed.clear();
        let dir = self.extensions_dir.clone();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let manifest_path = path.join("manifest.toml");
            let Ok(content) = std::fs::read_to_string(&manifest_path) else {
                continue;
            };
            let Ok(manifest) = toml::from_str::<ExtensionManifest>(&content) else {
                continue;
            };
            let lib_path = Self::find_lib(&path);
            self.installed.push(InstalledExtension {
                manifest,
                path,
                lib_path,
                enabled: true,
            });
        }
    }

    fn find_lib(dir: &std::path::Path) -> Option<PathBuf> {
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let p = entry.path();
            if let Some(ext) = p.extension() {
                if ext == "so" || ext == "dll" || ext == "dylib" {
                    return Some(p);
                }
            }
        }
        None
    }

    pub fn is_installed(&self, id: &str) -> bool {
        self.installed.iter().any(|e| e.manifest.extension.id == id)
    }

    pub fn uninstall(&mut self, id: &str) -> anyhow::Result<()> {
        if let Some(pos) = self
            .installed
            .iter()
            .position(|e| e.manifest.extension.id == id)
        {
            let ext = self.installed.remove(pos);
            std::fs::remove_dir_all(&ext.path)?;
        }
        Ok(())
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
