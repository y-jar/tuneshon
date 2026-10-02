use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Persistent user configuration for tuneshon.
///
/// `config_dir` is the Nix configuration directory used both as the
/// repository root (`repodir`) for `git add` and as the working directory
/// where `git`, `nix`, and `nixos-rebuild` are executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub config_dir: PathBuf,
    pub log_file: PathBuf,
    pub boot_loader: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".local/share"));
        let log_file = data_dir.join("tuneshon").join("tuneshon.log");
        Self {
            config_dir: PathBuf::from("/etc/nixos"),
            log_file,
            boot_loader: "systemd-boot".to_string(),
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("tuneshon")
            .join("config.json")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let cfg: AppConfig = serde_json::from_str(&raw)
                .with_context(|| format!("parsing {}", path.display()))?;
            Ok(cfg)
        } else {
            let cfg = AppConfig::default();
            // Best-effort persist a fresh default, non-fatal.
            let _ = cfg.save();
            Ok(cfg)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, raw).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Path to the lock file used for flake input selection.
    pub fn flake_lock_path(&self) -> PathBuf {
        self.config_dir.join("flake.lock")
    }
}

/// Ensure the parent directory of the log file exists.
pub fn ensure_log_dir(cfg: &AppConfig) -> Result<()> {
    if let Some(dir) = cfg.log_file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _ = Path::new(&cfg.config_dir);
    Ok(())
}