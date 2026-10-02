//! Read flake input names from a `flake.lock` file.

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// Parse a `flake.lock` file and return the set of flake input names.
///
/// Input names are the keys of `root.inputs`. We map each to a last-modified
/// stamp when available (best-effort), sorted by name.
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
}