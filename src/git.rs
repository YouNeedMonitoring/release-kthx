use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRecord {
    pub hash: String,
    pub subject: String,
    pub body: String,
}

fn run_git(path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .with_context(|| format!("failed executing git {:?}", args))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {:?} failed: {}", args, stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_owned(path: &Path, args: &[String]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .with_context(|| format!("failed executing git {:?}", args))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {:?} failed: {}", args, stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn latest_tag(path: &Path) -> Result<Option<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("describe")
        .arg("--tags")
        .arg("--abbrev=0")
        .output()
        .with_context(|| format!("failed finding latest tag in {}", path.display()))?;

    if !output.status.success() {
        return Ok(None);
    }

    let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if tag.is_empty() {
        Ok(None)
    } else {
        Ok(Some(tag))
    }
}

pub fn collect_commits(path: &Path, from_tag: Option<&str>) -> Result<Vec<CommitRecord>> {
    let mut args = vec![
        "log".to_string(),
        "--no-merges".to_string(),
        "--invert-grep".to_string(),
        "--grep=^chore(release):".to_string(),
        "--pretty=format:%H%x1f%s%x1f%b%x1e".to_string(),
    ];
    if let Some(tag) = from_tag {
        args.push(format!("{tag}..HEAD"));
    }

    let raw = run_git_owned(path, &args)?;
    let mut commits = Vec::new();

    for row in raw.split('\u{1e}') {
        let row = row.trim();
        if row.is_empty() {
            continue;
        }
        let mut parts = row.split('\u{1f}');
        let hash = parts.next().unwrap_or_default().trim().to_string();
        let subject = parts.next().unwrap_or_default().trim().to_string();
        let body = parts.next().unwrap_or_default().trim().to_string();

        if hash.is_empty() || subject.is_empty() {
            continue;
        }

        commits.push(CommitRecord {
            hash,
            subject,
            body,
        });
    }

    Ok(commits)
}

pub fn tag_exists(path: &Path, tag_name: &str) -> Result<bool> {
    let spec = format!("refs/tags/{tag_name}");
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--verify")
        .arg("--quiet")
        .arg(spec)
        .output()
        .with_context(|| format!("failed checking tag {tag_name}"))?;
    Ok(output.status.success())
}

pub fn create_annotated_tag(path: &Path, tag_name: &str, message: &str) -> Result<()> {
    if tag_exists(path, tag_name)? {
        bail!("tag {tag_name} already exists");
    }
    run_git(path, &["tag", "-a", tag_name, "-m", message])?;
    Ok(())
}

pub fn push_tag(path: &Path, tag_name: &str) -> Result<()> {
    run_git(path, &["push", "origin", tag_name])?;
    Ok(())
}

pub fn checkout_new_branch(path: &Path, branch: &str) -> Result<()> {
    run_git(path, &["checkout", "-B", branch])?;
    Ok(())
}

pub fn ensure_identity(path: &Path) -> Result<()> {
    let name_output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("config")
        .arg("user.name")
        .output()
        .context("failed reading git user.name")?;

    if !name_output.status.success()
        || String::from_utf8_lossy(&name_output.stdout)
            .trim()
            .is_empty()
    {
        run_git(path, &["config", "user.name", "release-kthx[bot]"])?;
    }

    let email_output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("config")
        .arg("user.email")
        .output()
        .context("failed reading git user.email")?;

    if !email_output.status.success()
        || String::from_utf8_lossy(&email_output.stdout)
            .trim()
            .is_empty()
    {
        run_git(
            path,
            &[
                "config",
                "user.email",
                "release-kthx[bot]@users.noreply.github.com",
            ],
        )?;
    }

    Ok(())
}

pub fn add_files(path: &Path, files: &[PathBuf]) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    let mut args = vec!["add".to_string()];
    for file in files {
        args.push(file.to_string_lossy().to_string());
    }
    run_git_owned(path, &args)?;
    Ok(())
}

pub fn has_staged_changes(path: &Path) -> Result<bool> {
    let status = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("diff")
        .arg("--cached")
        .arg("--quiet")
        .status()
        .context("failed checking staged changes")?;

    match status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => bail!("failed checking staged changes"),
    }
}

pub fn commit(path: &Path, message: &str) -> Result<()> {
    run_git(path, &["commit", "-m", message])?;
    Ok(())
}

pub fn push_branch(path: &Path, branch: &str) -> Result<()> {
    run_git(
        path,
        &[
            "push",
            "--force-with-lease",
            "--set-upstream",
            "origin",
            branch,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_empty_log_output() {
        let mut commits = Vec::new();
        for row in "".split('\u{1e}') {
            let row = row.trim();
            if !row.is_empty() {
                commits.push(row.to_string());
            }
        }
        assert!(commits.is_empty());
    }
}
