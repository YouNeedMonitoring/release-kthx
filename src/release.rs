use crate::git::{CliCommitHistoryService, CommitHistoryService};
use anyhow::{Context, Result, bail};
pub use release_kthx_domain::{CommitKind, ReleasePlan};
use release_kthx_domain::{WorkspaceCrate, WorkspaceGraph};
use semver::Version;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use toml::Value;
use toml_edit::{DocumentMut, Item, value};

#[derive(Debug, Clone)]
pub struct CrateInfo {
    pub name: String,
    pub manifest_path: PathBuf,
    pub crate_dir: PathBuf,
    pub version: Version,
    pub local_dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CrateReleasePlan {
    pub crate_name: String,
    pub manifest_path: PathBuf,
    pub plan: ReleasePlan,
}

pub fn collect_crates(path: &Path) -> Result<Vec<CrateInfo>> {
    let mut manifests = Vec::new();
    collect_cargo_manifests(path, &mut manifests)?;

    let mut crates = Vec::new();
    for manifest in manifests {
        if let Some(info) = parse_crate_info(path, &manifest)? {
            crates.push(info);
        }
    }

    crates.sort_by(|a, b| a.manifest_path.cmp(&b.manifest_path));
    populate_local_dependencies(path, &mut crates)?;
    Ok(crates)
}

pub fn build_crate_release_plans(
    path: &Path,
    from_tag: Option<&str>,
    tag_template: &str,
) -> Result<Vec<CrateReleasePlan>> {
    let history = CliCommitHistoryService;
    build_crate_release_plans_with_history(&history, path, from_tag, tag_template)
}

pub fn build_crate_release_plans_with_history(
    history: &impl CommitHistoryService,
    path: &Path,
    from_tag: Option<&str>,
    tag_template: &str,
) -> Result<Vec<CrateReleasePlan>> {
    let crates = collect_crates(path)?;
    if crates.is_empty() {
        bail!("no Cargo package manifests found");
    }
    let workspace_graph =
        WorkspaceGraph::from_crates(crates.iter().map(|crate_info| WorkspaceCrate {
            name: crate_info.name.clone(),
            local_dependencies: crate_info.local_dependencies.iter().cloned().collect(),
        }));

    let crate_count = crates.len();
    let mut plans = Vec::new();
    for crate_info in crates.iter() {
        let base_ref = resolve_base_reference(
            history,
            path,
            from_tag,
            tag_template,
            crate_info,
            crate_count,
        )?;

        let raw_commits = history.collect_commits(path, base_ref.as_deref())?;
        if raw_commits.is_empty() {
            continue;
        }

        let mut commit_inputs = Vec::new();
        for commit in raw_commits {
            let directly_affected = directly_affected_crates(&crates, &commit.files);
            let topology = workspace_graph.release_topology(directly_affected.iter());
            if !topology.includes(&crate_info.name) {
                continue;
            }

            commit_inputs.push(release_kthx_domain::CommitInput {
                hash: commit.hash,
                subject: commit.subject,
                body: commit.body,
            });
        }

        let Some(plan) = release_kthx_domain::plan_release(
            crate_info.version.clone(),
            base_ref.clone(),
            commit_inputs,
        ) else {
            continue;
        };

        plans.push(CrateReleasePlan {
            crate_name: crate_info.name.clone(),
            manifest_path: crate_info.manifest_path.clone(),
            plan,
        });
    }

    plans.sort_by(|a, b| a.manifest_path.cmp(&b.manifest_path));
    Ok(plans)
}

fn resolve_base_reference(
    history: &impl CommitHistoryService,
    path: &Path,
    from_tag: Option<&str>,
    tag_template: &str,
    crate_info: &CrateInfo,
    crate_count: usize,
) -> Result<Option<String>> {
    if let Some(explicit) = from_tag {
        return Ok(Some(explicit.to_string()));
    }

    let expected_tag = render_tag_name(
        tag_template,
        &crate_info.name,
        &crate_info.version,
        crate_count,
    )?;

    if history.tag_exists(path, &expected_tag)? {
        return Ok(Some(expected_tag));
    }

    let version = crate_info.version.to_string();
    if let Some(commit) = history.find_version_commit(path, &crate_info.manifest_path, &version)? {
        return Ok(Some(commit));
    }

    history.latest_tag(path)
}

pub fn set_crate_versions(path: &Path, plans: &[CrateReleasePlan]) -> Result<Vec<PathBuf>> {
    let mut changed = Vec::new();
    for plan in plans {
        let manifest_abs = path.join(&plan.manifest_path);
        if set_manifest_version(&manifest_abs, &plan.plan.next_version)? {
            changed.push(plan.manifest_path.clone());
        }
    }
    changed.sort();
    changed.dedup();
    Ok(changed)
}

pub fn set_lockfile_versions(path: &Path, plans: &[CrateReleasePlan]) -> Result<bool> {
    let lock_path = path.join("Cargo.lock");
    if !lock_path.exists() {
        return Ok(false);
    }

    let original = fs::read_to_string(&lock_path)
        .with_context(|| format!("failed reading {}", lock_path.display()))?;
    let mut doc = original
        .parse::<DocumentMut>()
        .with_context(|| format!("failed parsing {}", lock_path.display()))?;

    let mut changed = false;

    let package_item = match doc.get_mut("package") {
        Some(item) => item,
        None => return Ok(false),
    };
    let Some(packages) = package_item.as_array_of_tables_mut() else {
        return Ok(false);
    };

    for package in packages.iter_mut() {
        let Some(name) = package.get("name").and_then(|item| item.as_str()) else {
            continue;
        };

        let Some(plan) = plans.iter().find(|plan| plan.crate_name == name) else {
            continue;
        };

        let next_version = plan.plan.next_version.to_string();
        let current = package.get("version").and_then(|item| item.as_str());
        if current == Some(next_version.as_str()) {
            continue;
        }

        package.insert("version", value(next_version));
        changed = true;
    }

    if !changed {
        return Ok(false);
    }

    let rendered = doc.to_string();
    if rendered == original {
        return Ok(false);
    }

    fs::write(&lock_path, rendered)
        .with_context(|| format!("failed writing {}", lock_path.display()))?;
    Ok(true)
}

pub fn render_tag_name(
    tag_template: &str,
    crate_name: &str,
    version: &Version,
    crate_count: usize,
) -> Result<String> {
    if crate_count > 1 && !tag_template.contains("{{ crate }}") {
        bail!("release.tag_template must include '{{ crate }}' when multiple crates are released");
    }

    Ok(tag_template
        .replace("{{ crate }}", crate_name)
        .replace("{{ version }}", &version.to_string()))
}

pub fn is_release_merge_payload(files: &[PathBuf]) -> bool {
    let mut has_manifest = false;

    for file in files {
        let as_string = file.to_string_lossy();

        if as_string.ends_with("Cargo.toml") {
            has_manifest = true;
            continue;
        }

        if as_string == "Cargo.lock" {
            continue;
        }

        return false;
    }

    has_manifest
}

fn parse_crate_info(repo_root: &Path, manifest_abs: &Path) -> Result<Option<CrateInfo>> {
    let raw = fs::read_to_string(manifest_abs)
        .with_context(|| format!("failed reading {}", manifest_abs.display()))?;
    let value = raw
        .parse::<Value>()
        .with_context(|| format!("failed parsing {}", manifest_abs.display()))?;

    let Some(package) = value.get("package") else {
        return Ok(None);
    };
    let Some(name) = package.get("name").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(version_str) = package.get("version").and_then(Value::as_str) else {
        return Ok(None);
    };

    let version = Version::parse(version_str)
        .with_context(|| format!("invalid semver version in {}", manifest_abs.display()))?;

    let manifest_path = manifest_abs
        .strip_prefix(repo_root)
        .unwrap_or(manifest_abs)
        .to_path_buf();

    let crate_dir_abs = manifest_abs.parent().unwrap_or(repo_root);
    let crate_dir = if crate_dir_abs == repo_root {
        PathBuf::from(".")
    } else {
        crate_dir_abs
            .strip_prefix(repo_root)
            .unwrap_or(crate_dir_abs)
            .to_path_buf()
    };

    Ok(Some(CrateInfo {
        name: name.to_string(),
        manifest_path,
        crate_dir,
        version,
        local_dependencies: Vec::new(),
    }))
}

fn populate_local_dependencies(repo_root: &Path, crates: &mut [CrateInfo]) -> Result<()> {
    let crate_names = crates
        .iter()
        .map(|crate_info| crate_info.name.clone())
        .collect::<HashSet<_>>();

    for crate_info in crates.iter_mut() {
        let manifest_abs = repo_root.join(&crate_info.manifest_path);
        let raw = fs::read_to_string(&manifest_abs)
            .with_context(|| format!("failed reading {}", manifest_abs.display()))?;
        let value = raw
            .parse::<Value>()
            .with_context(|| format!("failed parsing {}", manifest_abs.display()))?;

        let mut deps = BTreeSet::new();
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(table) = value.get(section).and_then(Value::as_table) {
                collect_local_dependency_names(table, &crate_names, &mut deps);
            }
        }

        if let Some(targets) = value.get("target").and_then(Value::as_table) {
            for target in targets.values().filter_map(Value::as_table) {
                for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    if let Some(table) = target.get(section).and_then(Value::as_table) {
                        collect_local_dependency_names(table, &crate_names, &mut deps);
                    }
                }
            }
        }

        deps.remove(crate_info.name.as_str());
        crate_info.local_dependencies = deps.into_iter().collect();
    }

    Ok(())
}

fn collect_local_dependency_names(
    dependency_table: &toml::map::Map<String, Value>,
    crate_names: &HashSet<String>,
    result: &mut BTreeSet<String>,
) {
    for (key, value) in dependency_table {
        let crate_name = dependency_crate_name(key, value);
        if crate_names.contains(crate_name) {
            result.insert(crate_name.to_string());
        }
    }
}

fn dependency_crate_name<'a>(dependency_key: &'a str, dependency_value: &'a Value) -> &'a str {
    dependency_value
        .as_table()
        .and_then(|table| table.get("package"))
        .and_then(Value::as_str)
        .unwrap_or(dependency_key)
}

fn directly_affected_crates(crates: &[CrateInfo], files: &[PathBuf]) -> Vec<String> {
    let mut affected = HashSet::new();

    for file in files {
        let mut winner: Option<(usize, usize)> = None;

        for (index, crate_info) in crates.iter().enumerate() {
            if !crate_contains_file(&crate_info.crate_dir, file) {
                continue;
            }

            let rank = if crate_info.crate_dir == Path::new(".") {
                0
            } else {
                crate_info.crate_dir.components().count()
            };

            match winner {
                Some((_, current_rank)) if current_rank >= rank => {}
                _ => winner = Some((index, rank)),
            }
        }

        if let Some((index, _)) = winner {
            affected.insert(crates[index].name.clone());
        }
    }

    let mut ordered = affected.into_iter().collect::<Vec<_>>();
    ordered.sort();
    ordered
}

fn crate_contains_file(crate_dir: &Path, file: &Path) -> bool {
    if crate_dir == Path::new(".") {
        return true;
    }
    file.starts_with(crate_dir)
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
        && set_table_item_version(workspace_package, next_version)?
    {
        changed = true;
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
    use release_kthx_domain::BumpLevel;

    fn test_plan(crate_name: &str, current: &str, next: &str) -> CrateReleasePlan {
        CrateReleasePlan {
            crate_name: crate_name.to_string(),
            manifest_path: PathBuf::from(format!("crates/{crate_name}/Cargo.toml")),
            plan: ReleasePlan {
                base_tag: None,
                current_version: Version::parse(current).expect("valid semver"),
                next_version: Version::parse(next).expect("valid semver"),
                bump_level: BumpLevel::Patch,
                commits: Vec::new(),
            },
        }
    }

    #[test]
    fn tag_template_requires_crate_placeholder_for_multiple_crates() {
        let result = render_tag_name(
            "v{{ version }}",
            "my-crate",
            &Version::parse("0.2.0").expect("valid semver"),
            2,
        );
        assert!(result.is_err());
    }

    #[test]
    fn tag_template_renders_crate_and_version() {
        let tag = render_tag_name(
            "{{ crate }}-v{{ version }}",
            "my-crate",
            &Version::parse("0.2.0").expect("valid semver"),
            2,
        )
        .expect("tag renders");
        assert_eq!(tag, "my-crate-v0.2.0");
    }

    #[test]
    fn updates_workspace_package_versions_in_lockfile() {
        let temp = tempfile::tempdir().expect("temp dir");
        let lock_path = temp.path().join("Cargo.lock");
        fs::write(
            &lock_path,
            "version = 4\n\n[[package]]\nname = \"release-kthx\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"release-kthx-domain\"\nversion = \"0.1.0\"\n",
        )
        .expect("write lock");

        let plans = vec![
            test_plan("release-kthx", "0.1.0", "0.1.1"),
            test_plan("release-kthx-domain", "0.1.0", "0.2.0"),
        ];

        let changed = set_lockfile_versions(temp.path(), &plans).expect("lock update");
        assert!(changed);

        let updated = fs::read_to_string(&lock_path).expect("read updated lock");
        assert!(updated.contains("name = \"release-kthx\"\nversion = \"0.1.1\""));
        assert!(updated.contains("name = \"release-kthx-domain\"\nversion = \"0.2.0\""));
    }

    #[test]
    fn release_payload_accepts_manifest_and_lockfile() {
        let files = vec![
            PathBuf::from("Cargo.toml"),
            PathBuf::from("Cargo.lock"),
            PathBuf::from("crates/release-kthx-domain/Cargo.toml"),
        ];
        assert!(is_release_merge_payload(&files));
    }

    #[test]
    fn release_payload_rejects_non_release_files() {
        let files = vec![PathBuf::from("Cargo.toml"), PathBuf::from("README.md")];
        assert!(!is_release_merge_payload(&files));
    }

    #[test]
    fn collect_crates_populates_local_dependencies() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path();

        fs::create_dir_all(root.join("crates/app/src")).expect("create app dirs");
        fs::create_dir_all(root.join("crates/domain/src")).expect("create domain dirs");

        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/app\", \"crates/domain\"]\n",
        )
        .expect("write workspace manifest");

        fs::write(
            root.join("crates/domain/Cargo.toml"),
            "[package]\nname = \"domain\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .expect("write domain manifest");

        fs::write(
            root.join("crates/app/Cargo.toml"),
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\ndomain = { path = \"../domain\", version = \"0.1.0\" }\nserde = \"1\"\n",
        )
        .expect("write app manifest");

        let crates = collect_crates(root).expect("collect crates");

        let app = crates
            .iter()
            .find(|crate_info| crate_info.name == "app")
            .expect("app crate");
        assert_eq!(app.local_dependencies, vec!["domain"]);

        let domain = crates
            .iter()
            .find(|crate_info| crate_info.name == "domain")
            .expect("domain crate");
        assert!(domain.local_dependencies.is_empty());
    }
}
