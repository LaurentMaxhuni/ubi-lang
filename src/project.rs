use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use serde::Deserialize;

const MAX_CONFIG_BYTES: u64 = 64 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Project {
    schema_version: u32,
    name: String,
    entry: String,
    targets: Vec<String>,
}

pub(crate) fn valid_target(target: &str) -> bool {
    matches!(target, "web" | "mobile" | "desktop" | "cli")
}

pub(crate) fn select(root: &Path, target: Option<&str>) -> Result<(String, String), String> {
    let path = fs::canonicalize(root.join("ubi.json"))
        .map_err(|error| format!("cannot open ubi.json: {error}"))?;
    if !path.starts_with(root) {
        return Err("ubi.json escapes the canonical project root".to_owned());
    }
    let metadata =
        fs::metadata(&path).map_err(|error| format!("cannot inspect ubi.json: {error}"))?;
    if !metadata.is_file() {
        return Err("ubi.json must be a regular file".to_owned());
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err("ubi.json exceeds the 64 KiB config limit".to_owned());
    }
    let file = File::open(&path).map_err(|error| format!("cannot read ubi.json: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read ubi.json: {error}"))?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err("ubi.json exceeds the 64 KiB config limit".to_owned());
    }
    let project: Project =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid ubi.json: {error}"))?;
    if project.schema_version != 1 {
        return Err("ubi.json schemaVersion must be 1".to_owned());
    }
    if project.name.trim().is_empty() || project.name.chars().any(char::is_control) {
        return Err("ubi.json name must be nonblank and contain no control characters".to_owned());
    }
    if !crate::cli::valid_source_id(&project.entry) {
        return Err("ubi.json entry must be a canonical root-relative .ubi source ID".to_owned());
    }
    if project.targets.is_empty() {
        return Err("ubi.json targets must contain at least one target".to_owned());
    }
    let mut seen = BTreeSet::new();
    for platform in &project.targets {
        if !valid_target(platform) {
            return Err(format!(
                "unknown ubi.json target: {platform}; expected web, mobile, desktop, or cli"
            ));
        }
        if !seen.insert(platform) {
            return Err(format!("duplicate ubi.json target: {platform}"));
        }
    }
    let selected = match target {
        Some(target) if project.targets.iter().any(|platform| platform == target) => target,
        Some(target) => return Err(format!("target `{target}` is not declared in ubi.json")),
        None if project.targets.len() == 1 => &project.targets[0],
        None => return Err("multiple targets declared; select one with --target".to_owned()),
    };
    Ok((project.entry, selected.to_owned()))
}
