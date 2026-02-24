use anyhow::{Context, Result, bail};
use std::path::Path;
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
    let mut args = vec!["log", "--pretty=format:%H%x1f%s%x1f%b%x1e"];
    let range_owned = from_tag.map(|tag| format!("{tag}..HEAD"));
    if let Some(range) = &range_owned {
        args.push(range.as_str());
    }

    let raw = run_git(path, &args)?;
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
