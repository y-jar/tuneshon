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

    /// Turn multi-step commands into a preview (build) followed by an elevated
    /// apply (switch/boot). Pure `nix flake update` needs no apply/confirm step.
    pub fn needs_confirm(self) -> bool {
        !matches!(self, Action::FlakeLockUpdate)
    }
}

/// Common `nh` logging flags so output streams cleanly line-by-line instead of
/// being clobbered by nix-output-monitor's ANSI redraw frames.
const NH_LOG_FLAGS: &str = "--no-nom --show-activation-logs";

/// `nh os build` used as a non-elevated preview: compiles the config, prints
/// the result, and confirms the flake is valid — all without a password.
fn preview_build(cfg: &AppConfig) -> String {
    format!(
        "nh os build -e none {NH_LOG_FLAGS} --flake {}",
        shell_escape(&cfg.config_dir.to_string_lossy())
    )
}

/// `nh os switch|boot` elevated through pkexec (pops the polkit GUI dialog).
fn apply_os(flag: &str, cfg: &AppConfig) -> String {
    format!(
        "nh os {flag} -e pkexec {NH_LOG_FLAGS} --flake {}",
        shell_escape(&cfg.config_dir.to_string_lossy())
    )
}

/// Build the non-elevated preview command: user-only git/update step followed
/// by `nh os build`. `inputs` is an optional space-separated list of flake
/// inputs to update.
pub fn build_preview(action: Action, cfg: &AppConfig, inputs: Option<&str>) -> String {
    let build = preview_build(cfg);
    match action {
        Action::PullSwitchBoot => format!("git pull && {build}"),
        Action::AddSwitchBoot | Action::AddSwitch | Action::AddBoot => {
            format!("git add . && {build}")
        }
        Action::FlakeLockUpdate => {
            let targets = inputs.unwrap_or("").trim();
            format!("nix flake update {targets}")
        }
        Action::FlakeLockUpdateSwitchBoot => {
            let targets = inputs.unwrap_or("").trim();
            format!("nix flake update {targets} && {build}")
        }
    }
}

/// Build the elevated apply command(s) run only after the user confirms. Returns
/// `None` for actions that don't switch/boot (e.g. plain flake lock update).
pub fn build_apply(action: Action, cfg: &AppConfig) -> Option<String> {
    match action {
        Action::PullSwitchBoot | Action::AddSwitchBoot => Some(format!(
            "{} && {}",
            apply_os("switch", cfg),
            apply_os("boot", cfg)
        )),
        Action::AddSwitch => Some(apply_os("switch", cfg)),
        Action::AddBoot => Some(apply_os("boot", cfg)),
        Action::FlakeLockUpdateSwitchBoot => Some(apply_os("switch", cfg)),
        Action::FlakeLockUpdate => None,
    }
}

/// Full headless command: preview (build) then apply. Used by the CLI, which
/// cannot show the interactive confirmation gate.
pub fn build_command(action: Action, cfg: &AppConfig, inputs: Option<&str>) -> String {
    let preview = build_preview(action, cfg, inputs);
    match build_apply(action, cfg) {
        Some(apply) => format!("{preview} && {apply}"),
        None => preview,
    }
}

fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_alphanumeric() || "/_.-".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    fn cfg() -> AppConfig {
        let mut c = AppConfig::default();
        c.config_dir = std::path::PathBuf::from("/home/jar/nix-config");
        c
    }

    #[test]
    fn preview_is_nonelevated_build() {
        let cmd = build_preview(Action::AddSwitch, &cfg(), None);
        assert!(cmd.starts_with("git add . && nh os build -e none --no-nom"));
        assert!(cmd.contains("--flake /home/jar/nix-config"));
    }

    #[test]
    fn apply_uses_pkexec() {
        let cmd = build_apply(Action::AddSwitch, &cfg()).unwrap();
        assert!(cmd.starts_with("nh os switch -e pkexec --no-nom"));
    }

    #[test]
    fn flake_update_has_no_apply() {
        assert_eq!(build_apply(Action::FlakeLockUpdate, &cfg()), None);
        assert!(!Action::FlakeLockUpdate.needs_confirm());
    }

    #[test]
    fn switch_boot_chains_both() {
        let cmd = build_apply(Action::AddSwitchBoot, &cfg()).unwrap();
        assert!(cmd.contains("nh os switch -e pkexec"));
        assert!(cmd.contains("nh os boot -e pkexec"));
    }
}
