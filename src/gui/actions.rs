use crate::config::AppConfig;

/// The system action buttons present in the left control grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// git pull, then build + switch + boot (for non-tinkering users).
    FullUpdate,
    /// git add ., then build + switch + boot.
    Update,
    /// git add ., build, then test-activate (no boot default change).
    Test,
    /// git add ., build, then boot (activate on next restart).
    BootNext,
    /// Pick flake inputs, then `nix flake update` only.
    SpecifyUpdate,
    /// Pick flake inputs, `nix flake update`, build, then switch.
    SpecifyUpdateFull,
}

impl Action {
    pub const ALL: [Action; 6] = [
        Action::FullUpdate,
        Action::Update,
        Action::Test,
        Action::BootNext,
        Action::SpecifyUpdate,
        Action::SpecifyUpdateFull,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::FullUpdate => "full update",
            Action::Update => "update",
            Action::Test => "test",
            Action::BootNext => "update after restart",
            Action::SpecifyUpdate => "specify update?",
            Action::SpecifyUpdateFull => "specify update & update",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Action::FullUpdate => {
                "Pull the repo (git pull), build, then switch + boot so it applies \
                 on next reboot. For systems you don't tinker with."
            }
            Action::Update => {
                "Stage your repo changes (git add .), build, then switch + boot \
                 on next reboot."
            }
            Action::Test => {
                "Stage changes, build, then test-activate the new config. Does NOT \
                 change the boot default, so it's safe to try."
            }
            Action::BootNext => {
                "Stage changes, build, then boot — applies the new generation on \
                 your next reboot."
            }
            Action::SpecifyUpdate => {
                "Open a picker to choose which flake inputs to run `nix flake update` \
                 on (no switch, no activation)."
            }
            Action::SpecifyUpdateFull => {
                "Pick flake inputs, run `nix flake update`, build, then switch."
            }
        }
    }

    pub fn needs_flake_inputs(self) -> bool {
        matches!(self, Action::SpecifyUpdate | Action::SpecifyUpdateFull)
    }
}

/// `nh` flags for the non-elevated preview. `nh os build` does not accept
/// `--show-activation-logs`, so preview streams full build logs (`-L`) instead.
const NH_PREVIEW_FLAGS: &str = "--no-nom --print-build-logs";

/// Elevated `nh os switch|boot|test` accepts `--show-activation-logs`.
const NH_APPLY_FLAGS: &str = "--no-nom --show-activation-logs";

/// The config dir as a positional flake installable (nh has no `--flake` flag).
fn flake_arg(cfg: &AppConfig) -> String {
    shell_escape(&cfg.config_dir.to_string_lossy())
}

/// `nh os build` used as a non-elevated preview: compiles the config and
/// confirms it's valid — all without a password.
fn preview_build(cfg: &AppConfig) -> String {
    format!("nh os build -e none {NH_PREVIEW_FLAGS} {}", flake_arg(cfg))
}

/// `nh os switch|boot|test` elevated through pkexec (pops the polkit GUI dialog).
fn apply_os(flag: &str, cfg: &AppConfig) -> String {
    format!("nh os {flag} -e pkexec {NH_APPLY_FLAGS} {}", flake_arg(cfg))
}

/// Build the non-elevated preview command: user-only git/update step followed by
/// `nh os build`. `inputs` is an optional space-separated flake-input list.
pub fn build_preview(action: Action, cfg: &AppConfig, inputs: Option<&str>) -> String {
    let build = preview_build(cfg);
    match action {
        Action::FullUpdate => format!("git pull && {build}"),
        Action::Update | Action::Test | Action::BootNext => {
            format!("git add . && {build}")
        }
        Action::SpecifyUpdate => {
            let targets = inputs.unwrap_or("").trim();
            format!("nix flake update {targets}")
        }
        Action::SpecifyUpdateFull => {
            let targets = inputs.unwrap_or("").trim();
            format!("nix flake update {targets} && {build}")
        }
    }
}

/// Build the elevated apply command run only after the user confirms. Returns
/// `None` for pure-update actions that don't switch/boot/test.
pub fn build_apply(action: Action, cfg: &AppConfig) -> Option<String> {
    match action {
        Action::FullUpdate | Action::Update => Some(format!(
            "{} && {}",
            apply_os("switch", cfg),
            apply_os("boot", cfg)
        )),
        Action::Test => Some(apply_os("test", cfg)),
        Action::BootNext => Some(apply_os("boot", cfg)),
        Action::SpecifyUpdateFull => Some(apply_os("switch", cfg)),
        Action::SpecifyUpdate => None,
    }
}

/// Direct `nix flake update <inputs>` used by the quick input-section buttons.
/// These are non-elevated and don't go through the build/confirm gate.
pub fn build_input_update(inputs: &[&str]) -> String {
    format!("nix flake update {}", inputs.join(" "))
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

    fn cfg() -> AppConfig {
        let mut c = AppConfig::default();
        c.config_dir = std::path::PathBuf::from("/home/jar/nix-config");
        c
    }

    #[test]
    fn full_update_pulls_and_builds() {
        let cmd = build_preview(Action::FullUpdate, &cfg(), None);
        assert!(cmd.starts_with("git pull && nh os build -e none --no-nom --print-build-logs"));
        assert!(cmd.ends_with("/home/jar/nix-config"));
        assert!(!cmd.contains("--show-activation-logs"));
    }

    #[test]
    fn update_git_adds_then_builds() {
        let cmd = build_preview(Action::Update, &cfg(), None);
        assert!(cmd.starts_with("git add . && nh os build"));
    }

    #[test]
    fn apply_uses_pkexec_switch_boot() {
        let cmd = build_apply(Action::FullUpdate, &cfg()).unwrap();
        assert!(cmd.contains("nh os switch -e pkexec --no-nom --show-activation-logs"));
        assert!(cmd.contains("nh os boot -e pkexec"));
        assert!(cmd.ends_with("/home/jar/nix-config"));
    }

    #[test]
    fn test_apply_uses_nh_test() {
        let cmd = build_apply(Action::Test, &cfg()).unwrap();
        assert!(cmd.starts_with("nh os test -e pkexec --no-nom --show-activation-logs"));
    }

    #[test]
    fn boot_next_applies_boot_only() {
        let cmd = build_apply(Action::BootNext, &cfg()).unwrap();
        assert!(cmd.starts_with("nh os boot -e pkexec"));
        assert!(!cmd.contains("switch"));
    }

    #[test]
    fn specify_update_has_no_apply() {
        assert_eq!(build_apply(Action::SpecifyUpdate, &cfg()), None);
    }

    #[test]
    fn input_update_joins_targets() {
        assert_eq!(
            build_input_update(&["nixpkgs"]),
            "nix flake update nixpkgs"
        );
        assert_eq!(
            build_input_update(&["icon-jar", "shelljar"]),
            "nix flake update icon-jar shelljar"
        );
    }
}
