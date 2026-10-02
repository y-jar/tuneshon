use std::path::Path;

/// One NixOS system generation row, from `nh os info`.
#[derive(Debug, Clone)]
pub struct Generation {
    pub id: u64,
    pub date: String,
    pub nixos_version: String,
    pub kernel: String,
    pub size: String,
    /// True when this is the current (profile-target) generation.
    pub current: bool,
}

/// Strip ANSI escape sequences from a captured line.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_esc = false;
    for c in s.chars() {
        if in_esc {
            if c == 'm' {
                in_esc = false;
            }
            continue;
        }
        if c == '\x1b' {
            in_esc = true;
            continue;
        }
        out.push(c);
    }
    out
}

/// Current generation id from the profile symlink,
/// e.g. `/nix/var/nix/profiles/system -> system-474-link`.
fn current_id_from_profile(profile: &Path) -> Option<u64> {
    let raw = std::fs::read_link(profile)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| profile.to_string_lossy().into_owned());
    let name = raw
        .rsplit('/')
        .next()
        .unwrap_or(&raw)
        .to_string();
    name.trim_start_matches("system-")
        .trim_end_matches("-link")
        .parse::<u64>()
        .ok()
}

/// Parse the `nh os info` output (newest-first rows) into [Generation]s, marking
/// the one matching `profile`'s symlink target as current.
pub fn parse(output: &str, profile: &Path) -> Vec<Generation> {
    let current = current_id_from_profile(profile);
    let mut rows = Vec::new();
    for raw in output.lines() {
        let line = strip_ansi(raw);
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
            || line.starts_with("Generation No")
        {
            continue;
        }
        let mut it = trimmed.split_whitespace();
        let id = match it.next().and_then(|s| s.parse::<u64>().ok()) {
            Some(v) => v,
            None => continue,
        };
        // Build Date is two tokens (date + time).
        let date = match (it.next(), it.next()) {
            (Some(d), Some(t)) => format!("{d} {t}"),
            _ => continue,
        };
        let nixos_version = it.next().unwrap_or("").to_string();
        let kernel = it.next().unwrap_or("").to_string();
        // Closure Size is "32.0" followed by a unit token ("GB"/"MiB"/...).
        let size = match (it.next(), it.next()) {
            (Some(n), Some(unit)) => format!("{n} {unit}"),
            (Some(n), None) => n.to_string(),
            _ => String::new(),
        };
        rows.push(Generation {
            id,
            date,
            nixos_version,
            kernel,
            size,
            current: Some(id) == current,
        });
    }
    rows
}

/// Run `nh os info` and parse the generations for `config_dir`. Returns an
/// empty list on any failure (caller shows a placeholder).
pub fn fetch(config_dir: &Path) -> Vec<Generation> {
    let profile = Path::new("/nix/var/nix/profiles/system");
    let output = std::process::Command::new("nh")
        .args(["os", "info"])
        .current_dir(config_dir)
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout).into_owned();
            parse(&text, profile)
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> &'static str {
        "\u{1b}[33m!\u{1b}[0m Profile is out of sync warning\n\
         NixOS 26.05.2026 (profile may need sync)\n\
         Generation No Build Date           NixOS Version          Kernel        Closure Size\n\
         474           2026-10-02 10:15:35  26.05.20260903.a5cc6f2 7.2.3-cachyos 32.0 GB\n\
         473           2026-10-01 22:48:04  26.05.20260903.a5cc6f2 7.2.3-cachyos 32.0 GB\n"
    }

    #[test]
    fn parses_rows_and_strips_ansi() {
        let gens = parse(sample(), Path::new("/nix/var/nix/profiles/system-474-link"));
        assert_eq!(gens.len(), 2);
        assert_eq!(gens[0].id, 474);
        assert_eq!(gens[0].kernel, "7.2.3-cachyos");
        assert_eq!(gens[0].size, "32.0 GB");
    }

    #[test]
    fn marks_current_from_profile() {
        let g = parse(sample(), Path::new("system-474-link"));
        assert!(g[0].current);
        assert!(!g[1].current);
    }
}