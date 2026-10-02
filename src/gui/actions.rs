use crate::config::AppConfig;

/// The six action buttons present in the left control grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    PullSwitchBoot,
    AddSwitchBoot,
    AddSwitch,
    AddBoot,
    FlakeLockUpdate,
    FlakeLockUpdateSwitchBoot,
}

impl Action {
    pub const ALL: [Action; 6] = [
        Action::PullSwitchBoot,
        Action::AddSwitchBoot,
        Action::AddSwitch,
        Action::AddBoot,
        Action::FlakeLockUpdate,
        Action::FlakeLockUpdateSwitchBoot,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::PullSwitchBoot => "pull + switch + boot",
            Action::AddSwitchBoot => "git add {repodir} + switch + boot",
            Action::AddSwitch => "git add {repodir} + switch",
            Action::AddBoot => "git add {repodir} + boot",
            Action::FlakeLockUpdate => "flake lock update",
            Action::FlakeLockUpdateSwitchBoot => "flake lock update + switch + boot",
        }
    }

    pub fn needs_flake_inputs(self) -> bool {
        matches!(
            self,
            Action::FlakeLockUpdate | Action::FlakeLockUpdateSwitchBoot
        )
    }
}

/// Build the shell command string for an action based on the current config.
/// `inputs` is an optional space-separated list of flake inputs to update.
pub fn build_command(action: Action, cfg: &AppConfig, inputs: Option<&str>) -> String {
    let dir = shell_escape(&cfg.config_dir.to_string_lossy());
    match action {
        Action::PullSwitchBoot => format!(
            "git pull && sudo nixos-rebuild switch --flake {} && sudo nixos-rebuild boot --flake {}",
            dir, dir
        ),
        Action::AddSwitchBoot => format!(
            "git add . && sudo nixos-rebuild switch --flake {} && sudo nixos-rebuild boot --flake {}",
            dir, dir
        ),
        Action::AddSwitch => format!(
            "git add . && sudo nixos-rebuild switch --flake {}",
            dir
        ),
        Action::AddBoot => format!(
            "git add . && sudo nixos-rebuild boot --flake {}",
            dir
        ),
        Action::FlakeLockUpdate => {
            let targets = inputs.unwrap_or("").trim();
            format!("nix flake update {}", targets)
        }
        Action::FlakeLockUpdateSwitchBoot => {
            let targets = inputs.unwrap_or("").trim();
            format!(
                "nix flake update {} && sudo nixos-rebuild switch --flake {}",
                targets, dir
            )
        }
    }
}

fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_alphanumeric() || "/_.-".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}