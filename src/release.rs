use crate::git;
use anyhow::{Context, Result};
pub use release_kthx_domain::{CommitKind, ReleasePlan};
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value;
use toml_edit::{DocumentMut, Item, value};

pub fn build_release_plan(path: &Path, from_tag: Option<&str>) -> Result<ReleasePlan> {
    build_release_plan_optional(path, from_tag)?
        .ok_or_else(|| anyhow::anyhow!("no releasable changes found"))
}

pub fn build_release_plan_optional(
    path: &Path,
    from_tag: Option<&str>,
) -> Result<Option<ReleasePlan>> {
    let current_version = current_version(path)?;
    let base_tag = if let Some(explicit) = from_tag {
        Some(explicit.to_string())
    } else {
        git::latest_tag(path)?
    };

    let raw_commits = git::collect_commits(path, base_tag.as_deref())?;
    let commits = raw_commits
        .into_iter()
        .map(|item| release_kthx_domain::CommitInput {
            hash: item.hash,
            subject: item.subject,
            body: item.body,
        })
        .collect::<Vec<_>>();

    Ok(release_kthx_domain::plan_release(
        current_version,
        base_tag,
        commits,
    ))
}

pub fn current_version(path: &Path) -> Result<Version> {
    let cargo_toml = path.join("Cargo.toml");
    let raw = fs::read_to_string(&cargo_toml)
        .with_context(|| format!("failed reading {}", cargo_toml.display()))?;
    let value = raw
        .parse::<Value>()
        .with_context(|| format!("failed parsing {}", cargo_toml.display()))?;

    let version_str = value
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("workspace")
                .and_then(|workspace| workspace.get("package"))
                .and_then(|package| package.get("version"))
                .and_then(Value::as_str)
        })
        .ok_or_else(|| {
            anyhow::anyhow!("cannot find package.version or workspace.package.version")
        })?;

    let version = Version::parse(version_str)
        .with_context(|| format!("invalid semver version in {}", cargo_toml.display()))?;
    Ok(version)
}

pub fn set_workspace_versions(path: &Path, next_version: &Version) -> Result<Vec<PathBuf>> {
    let mut manifests = Vec::new();
    collect_cargo_manifests(path, &mut manifests)?;
    manifests.sort();

    let mut changed = Vec::new();
    for manifest in manifests {
        if set_manifest_version(&manifest, next_version)? {
            let relative = manifest
                .strip_prefix(path)
                .unwrap_or(manifest.as_path())
                .to_path_buf();
            changed.push(relative);
        }
    }

    Ok(changed)
}

fn collect_cargo_manifests(dir: &Path, manifests: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(dir).with_context(|| format!("failed reading {}", dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("failed reading entry in {}", dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed reading entry type in {}", dir.display()))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let path = entry.path();

        if file_type.is_dir() {
            if name == ".git" || name == "target" {
                continue;
            }
            collect_cargo_manifests(&path, manifests)?;
            continue;
        }

        if file_type.is_file() && name == "Cargo.toml" {
            manifests.push(path);
        }
    }

    Ok(())
}

fn set_manifest_version(manifest_path: &Path, next_version: &Version) -> Result<bool> {
    let original = fs::read_to_string(manifest_path)
        .with_context(|| format!("failed reading {}", manifest_path.display()))?;
    let mut doc = original
        .parse::<DocumentMut>()
        .with_context(|| format!("failed parsing {}", manifest_path.display()))?;

    let mut changed = false;

    if set_item_version(&mut doc, "package", next_version)? {
        changed = true;
    }

    if let Some(workspace) = doc.get_mut("workspace")
        && workspace.is_table_like()
        && let Some(workspace_package) = workspace
            .as_table_like_mut()
            .and_then(|table| table.get_mut("package"))
    {
        if set_table_item_version(workspace_package, next_version)? {
            changed = true;
        }
    }

    if !changed {
        return Ok(false);
    }

    let rendered = doc.to_string();
    if rendered == original {
        return Ok(false);
    }

    fs::write(manifest_path, rendered)
        .with_context(|| format!("failed writing {}", manifest_path.display()))?;
    Ok(true)
}

fn set_item_version(
    doc: &mut DocumentMut,
    item_name: &str,
    next_version: &Version,
) -> Result<bool> {
    let Some(item) = doc.get_mut(item_name) else {
        return Ok(false);
    };
    set_table_item_version(item, next_version)
}

fn set_table_item_version(item: &mut Item, next_version: &Version) -> Result<bool> {
    let Some(table) = item.as_table_like_mut() else {
        return Ok(false);
    };

    if !table.contains_key("version") {
        return Ok(false);
    }

    table.insert("version", value(next_version.to_string()));
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_package_version() {
        let version = current_version(Path::new(".")).expect("version should parse");
        assert_eq!(version.to_string(), "0.1.0");
    }

    #[test]
    fn updates_manifest_string() {
        let mut doc = "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n"
            .parse::<DocumentMut>()
            .expect("doc parses");
        let changed = set_item_version(
            &mut doc,
            "package",
            &Version::parse("0.2.0").expect("valid semver"),
        )
        .expect("set version");
        assert!(changed);
        assert!(doc.to_string().contains("0.2.0"));
    }
}
