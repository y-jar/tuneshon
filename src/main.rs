mod config;
mod flakelock;
mod gui;
mod runner;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "tuneshon",
    version,
    about = "A lightweight, fast NixOS update tool (GUI + CLI)."
)]
pub struct Args {
    /// Launch the graphical interface.
    #[arg(long)]
    gui: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// git pull, then nixos-rebuild switch and boot.
    PullSwitchBoot,
    /// git add {repodir}, then switch and boot.
    #[command(name = "add-switch-boot")]
    AddSwitchBoot {
        /// Override the Nix config dir (default: from config).
        #[arg(long)]
        dir: Option<String>,
    },
    /// git add {repodir}, then switch.
    #[command(name = "add-switch")]
    AddSwitch {
        #[arg(long)]
        dir: Option<String>,
    },
    /// git add {repodir}, then boot.
    #[command(name = "add-boot")]
    AddBoot {
        #[arg(long)]
        dir: Option<String>,
    },
    /// Run `nix flake update` on optional inputs (from flake.lock).
    #[command(name = "flake-update")]
    FlakeUpdate {
        /// Flake inputs to update; if omitted, a filterable picker is shown
        /// when using the GUI, or all inputs in headless mode.
        inputs: Vec<String>,
    },
    /// `nix flake update` then nixos-rebuild switch.
    #[command(name = "flake-update-switch-boot")]
    FlakeUpdateSwitchBoot {
        inputs: Vec<String>,
    },
}

fn main() {
    match real_main() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("tuneshon: {e:#}");
            std::process::exit(1);
        }
    }
}

fn real_main() -> anyhow::Result<i32> {
    let args = Args::parse();
    let mut cfg = config::AppConfig::load()?;
    config::ensure_log_dir(&cfg)?;

    if args.gui || args.command.is_none() {
        return run_gui().map(|_| 0);
    }

    let code = match args.command.unwrap() {
        Command::PullSwitchBoot => {
            let cmd = gui::actions::build_command(
                gui::actions::Action::PullSwitchBoot,
                &cfg,
                None,
            );
            runner::run_cli(&cmd, &cfg.config_dir, &cfg.log_file)?
        }
        Command::AddSwitchBoot { dir } => {
            if let Some(d) = dir {
                cfg.config_dir = std::path::PathBuf::from(d);
            }
            let cmd = gui::actions::build_command(
                gui::actions::Action::AddSwitchBoot,
                &cfg,
                None,
            );
            runner::run_cli(&cmd, &cfg.config_dir, &cfg.log_file)?
        }
        Command::AddSwitch { dir } => {
            if let Some(d) = dir {
                cfg.config_dir = std::path::PathBuf::from(d);
            }
            let cmd =
                gui::actions::build_command(gui::actions::Action::AddSwitch, &cfg, None);
            runner::run_cli(&cmd, &cfg.config_dir, &cfg.log_file)?
        }
        Command::AddBoot { dir } => {
            if let Some(d) = dir {
                cfg.config_dir = std::path::PathBuf::from(d);
            }
            let cmd = gui::actions::build_command(gui::actions::Action::AddBoot, &cfg, None);
            runner::run_cli(&cmd, &cfg.config_dir, &cfg.log_file)?
        }
        Command::FlakeUpdate { inputs } => {
            let inputs = resolve_inputs(&inputs, &cfg);
            let cmd = gui::actions::build_command(
                gui::actions::Action::FlakeLockUpdate,
                &cfg,
                Some(&inputs),
            );
            runner::run_cli(&cmd, &cfg.config_dir, &cfg.log_file)?
        }
        Command::FlakeUpdateSwitchBoot { inputs } => {
            let inputs = resolve_inputs(&inputs, &cfg);
            let cmd = gui::actions::build_command(
                gui::actions::Action::FlakeLockUpdateSwitchBoot,
                &cfg,
                Some(&inputs),
            );
            runner::run_cli(&cmd, &cfg.config_dir, &cfg.log_file)?
        }
    };
    Ok(code)
}

/// If no inputs were given, default to updating every input present in flake.lock.
fn resolve_inputs(provided: &[String], cfg: &config::AppConfig) -> String {
    if !provided.is_empty() {
        return provided.join(" ");
    }
    match flakelock::parse_inputs(&cfg.flake_lock_path()) {
        Ok(map) => map.into_keys().collect::<Vec<_>>().join(" "),
        Err(_) => String::new(),
    }
}

fn run_gui() -> Result<()> {
    gui::Gui::launch()
}