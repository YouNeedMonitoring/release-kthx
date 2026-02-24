use anyhow::{Context, Result, bail};
use std::env;
use std::path::Path;
use std::process::Command;

fn build_gh_command(path: &Path, token_env: &str, args: &[&str]) -> Command {
    let mut cmd = Command::new("gh");
    cmd.current_dir(path);
    cmd.args(args);

    if env::var("GH_TOKEN").is_err() {
        if let Ok(token) = env::var(token_env) {
            cmd.env("GH_TOKEN", token);
        }
    }

    cmd
}

fn run_gh(path: &Path, token_env: &str, args: &[&str]) -> Result<String> {
    let output = build_gh_command(path, token_env, args)
        .output()
        .with_context(|| format!("failed running gh {:?}", args))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("gh {:?} failed: {}", args, stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_gh_optional(path: &Path, token_env: &str, args: &[&str]) -> Result<Option<String>> {
    let output = build_gh_command(path, token_env, args)
        .output()
        .with_context(|| format!("failed running gh {:?}", args))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() || stdout == "null" {
        Ok(None)
    } else {
        Ok(Some(stdout))
    }
}

pub fn create_or_update_release_pr(
    path: &Path,
    token_env: &str,
    base_branch: &str,
    pr_branch: &str,
    title: &str,
    body: &str,
) -> Result<String> {
    let existing_number = run_gh_optional(
        path,
        token_env,
        &[
            "pr",
            "list",
            "--head",
            pr_branch,
            "--base",
            base_branch,
            "--state",
            "open",
            "--json",
            "number",
            "--jq",
            ".[0].number",
        ],
    )?;

    if let Some(number) = existing_number {
        run_gh(
            path,
            token_env,
            &["pr", "edit", &number, "--title", title, "--body", body],
        )?;
        let pr_url = run_gh(
            path,
            token_env,
            &["pr", "view", &number, "--json", "url", "--jq", ".url"],
        )?;
        Ok(pr_url)
    } else {
        let pr_url = run_gh(
            path,
            token_env,
            &[
                "pr",
                "create",
                "--base",
                base_branch,
                "--head",
                pr_branch,
                "--title",
                title,
                "--body",
                body,
            ],
        )?;
        Ok(pr_url)
    }
}

pub fn create_or_update_release(
    path: &Path,
    token_env: &str,
    tag_name: &str,
    title: &str,
    notes: &str,
) -> Result<String> {
    let existing_url = run_gh_optional(
        path,
        token_env,
        &["release", "view", tag_name, "--json", "url", "--jq", ".url"],
    )?;

    if existing_url.is_some() {
        run_gh(
            path,
            token_env,
            &[
                "release", "edit", tag_name, "--title", title, "--notes", notes,
            ],
        )?;
        let updated_url = run_gh(
            path,
            token_env,
            &["release", "view", tag_name, "--json", "url", "--jq", ".url"],
        )?;
        Ok(updated_url)
    } else {
        let created_url = run_gh(
            path,
            token_env,
            &[
                "release", "create", tag_name, "--title", title, "--notes", notes,
            ],
        )?;
        Ok(created_url)
    }
}
