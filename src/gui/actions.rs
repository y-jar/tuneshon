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

/// Absolute path to `nixos-rebuild`. pkexec forbids PATH-relative execution,
/// so we always invoke the system binary by full path.
const NIXOS_REBUILD: &str = "/run/current-system/sw/bin/nixos-rebuild";

/// Root-level `nixos-rebuild` invocations run through `pkexec`, which pops the
/// polkit GUI auth dialog (the running GNOME agent) instead of needing a TTY.
fn rebuild(flag: &str, cfg: &AppConfig) -> String {
    let dir = shell_escape(&cfg.config_dir.to_string_lossy());
    format!("pkexec {NIXOS_REBUILD} {flag} --flake {}", dir)
}

/// Build the shell command string for an action based on the current config.
/// `inputs` is an optional space-separated list of flake inputs to update.
pub fn build_command(action: Action, cfg: &AppConfig, inputs: Option<&str>) -> String {
    match action {
        Action::PullSwitchBoot => format!(
            "git pull && {} && {}",
            rebuild("switch", cfg),
            rebuild("boot", cfg)
        ),
        Action::AddSwitchBoot => format!(
            "git add . && {} && {}",
            rebuild("switch", cfg),
            rebuild("boot", cfg)
        ),
        Action::AddSwitch => format!("git add . && {}", rebuild("switch", cfg)),
        Action::AddBoot => format!("git add . && {}", rebuild("boot", cfg)),
        Action::FlakeLockUpdate => {
            let targets = inputs.unwrap_or("").trim();
            format!("nix flake update {}", targets)
        }
        Action::FlakeLockUpdateSwitchBoot => {
            let targets = inputs.unwrap_or("").trim();
            format!("nix flake update {} && {}", targets, rebuild("switch", cfg))
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