use crate::git;
use anyhow::{Context, Result};
pub use release_kthx_domain::{CommitKind, ReleasePlan};
use semver::Version;
use std::fs;
use std::path::Path;
use toml::Value;

pub fn build_release_plan(path: &Path, from_tag: Option<&str>) -> Result<ReleasePlan> {
    let current_version = read_current_version(path)?;
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

    release_kthx_domain::plan_release(current_version, base_tag, commits)
        .ok_or_else(|| anyhow::anyhow!("no commits found in selected range"))
}

fn read_current_version(path: &Path) -> Result<Version> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_package_version() {
        let version = read_current_version(Path::new(".")).expect("version should parse");
        assert_eq!(version.to_string(), "0.1.0");
    }
}
