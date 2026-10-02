//! Persistent user configuration (`~/.config/tuneshon/config.json`).
//!
//! Stored as JSON: the Nix config dir (repo root), the log file path, and the
//! boot loader name. `TUNESHON_CONFIG_DIR`, baked into the installed binary's
//! wrapper by the flake, overrides the persisted config dir on load.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persistent user configuration for tuneshon.
///
/// `config_dir` is the Nix configuration directory used both as the
/// repository root (`repodir`) for `git add` and as the working directory
/// where `git`, `nix`, and `nh` are executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub config_dir: PathBuf,
    pub log_file: PathBuf,
    pub boot_loader: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        // TUNESHON_CONFIG_DIR lets a NixOS flake module pre-set the Nix config
        // dir before the app is first run, mirroring the CLI `--dir` override.
        let config_dir = std::env::var("TUNESHON_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/etc/nixos"));
        let xdg_config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
        let log_file = xdg_config_dir.join("tuneshon").join("tuneshon.log");
        Self {
            config_dir,
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
        let mut cfg = if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            serde_json::from_str::<AppConfig>(&raw)
                .with_context(|| format!("parsing {}", path.display()))?
        } else {
            let cfg = AppConfig::default();
            // Best-effort persist a fresh default, non-fatal.
            let _ = cfg.save();
            cfg
        };
        // TUNESHON_CONFIG_DIR (baked into the binary wrapper by the flake) takes
        // precedence over any on-disk value, so a packaged install always points
        // at the right Nix config repo regardless of stale/local config files.
        if let Ok(dir) = std::env::var("TUNESHON_CONFIG_DIR") {
            if !dir.is_empty() {
                cfg.config_dir = PathBuf::from(dir);
            }
        }
        Ok(cfg)
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
    Ok(())
}
