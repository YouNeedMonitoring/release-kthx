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
        bail!("{}", format_gh_error(args, stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn format_gh_error(args: &[&str], stderr: &str) -> String {
    if stderr.contains("GitHub Actions is not permitted to create or approve pull requests")
        || stderr.contains("createPullRequest")
    {
        return format!(
            "Cannot create release PR from this workflow run. GitHub is blocking pull request creation for this token.\n\n\
             What to do:\n\
             1) In repository settings, set Actions -> General -> Workflow permissions to 'Read and write permissions'.\n\
             2) In repository settings, enable 'Allow GitHub Actions to create and approve pull requests'.\n\
             3) Keep workflow job permissions including `pull-requests: write` and `contents: write`.\n\n\
             Original gh args: {:?}\n\
             Original error: {}",
            args, stderr
        );
    }

    if stderr.contains("repository.pullRequest.projectCards")
        || stderr.contains("Projects (classic) is being deprecated")
    {
        return format!(
            "GitHub CLI failed due to a legacy Projects(classic) GraphQL field.\n\
             This usually comes from older gh flows using projectCards.\n\
             release-kthx now uses REST for PR updates to avoid this, so rerun after updating to the latest action commit.\n\n\
             Original gh args: {:?}\n\
             Original error: {}",
            args, stderr
        );
    }

    format!("gh {:?} failed: {}", args, stderr)
}

fn repository_slug() -> Result<String> {
    env::var("GITHUB_REPOSITORY").with_context(|| "missing GITHUB_REPOSITORY environment variable")
}

fn update_pull_request(
    path: &Path,
    token_env: &str,
    pr_number: &str,
    title: &str,
    body: &str,
) -> Result<()> {
    let repo = repository_slug()?;
    let endpoint = format!("repos/{repo}/pulls/{pr_number}");
    run_gh(
        path,
        token_env,
        &[
            "api",
            "--method",
            "PATCH",
            endpoint.as_str(),
            "-f",
            &format!("title={title}"),
            "-f",
            &format!("body={body}"),
        ],
    )?;
    Ok(())
}

fn pull_request_url(path: &Path, token_env: &str, pr_number: &str) -> Result<String> {
    let repo = repository_slug()?;
    let endpoint = format!("repos/{repo}/pulls/{pr_number}");
    run_gh(
        path,
        token_env,
        &["api", endpoint.as_str(), "--jq", ".html_url"],
    )
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
        update_pull_request(path, token_env, &number, title, body)?;
        let pr_url = pull_request_url(path, token_env, &number)?;
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
