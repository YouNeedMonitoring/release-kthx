mod changelog;
mod cli;
mod config;
mod git;
mod github;
mod release;

use anyhow::{Result, bail};
use clap::Parser;
use cli::{Cli, Command};
use config::ReleaseKthxConfig;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init { path, force } => {
            let destination = path.join("release-kthx.toml");
            config::init_config(&destination, force)?;
            println!("wrote {}", destination.display());
        }
        Command::Check { path } => {
            let cfg = load_config(&path)?;
            cfg.validate()?;
            println!("config valid: {}", path.join("release-kthx.toml").display());
        }
        Command::Plan { path, from_tag } => {
            run_plan(path, from_tag)?;
        }
        Command::ReleasePr {
            path,
            from_tag,
            base_branch,
            pr_branch,
        } => {
            run_release_pr(path, from_tag, &base_branch, &pr_branch)?;
        }
        Command::Release {
            path,
            from_tag,
            dry_run,
            push,
        } => {
            run_release(path, from_tag, dry_run, push)?;
        }
        Command::Publish {
            path,
            dry_run,
            push,
        } => {
            run_publish(path, dry_run, push)?;
        }
    }

    Ok(())
}

fn load_config(path: &Path) -> Result<ReleaseKthxConfig> {
    let config_path = path.join("release-kthx.toml");
    let cfg = ReleaseKthxConfig::from_path(&config_path)?;
    cfg.validate()?;
    Ok(cfg)
}

fn run_plan(path: PathBuf, from_tag: Option<String>) -> Result<()> {
    let cfg = load_config(&path)?;
    let Some(plan) = release::build_release_plan_optional(&path, from_tag.as_deref())? else {
        println!("no releasable changes detected");
        return Ok(());
    };

    println!("repo: {}", path.display());
    println!("base-version: {}", plan.current_version);
    println!("next-version: {}", plan.next_version);
    println!("bump: {}", plan.bump_level);
    println!("commits: {}", plan.commits.len());
    println!(
        "github-release: {}",
        if cfg.github.create_release {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("\n{}", changelog::render_markdown(&plan));
    Ok(())
}

fn run_release_pr(
    path: PathBuf,
    from_tag: Option<String>,
    base_branch: &str,
    pr_branch: &str,
) -> Result<()> {
    let cfg = load_config(&path)?;
    let Some(plan) = release::build_release_plan_optional(&path, from_tag.as_deref())? else {
        println!("no releasable changes detected; skipping release PR");
        return Ok(());
    };

    git::checkout_new_branch(&path, pr_branch)?;
    let changed_manifests = release::set_workspace_versions(&path, &plan.next_version)?;
    if changed_manifests.is_empty() {
        bail!("no Cargo.toml version fields found to update");
    }

    git::ensure_identity(&path)?;
    git::add_files(&path, &changed_manifests)?;

    let title = format!("chore(release): v{}", plan.next_version);
    if git::has_staged_changes(&path)? {
        git::commit(&path, &title)?;
    } else {
        println!(
            "release versions already up to date on branch {}",
            pr_branch
        );
    }

    git::push_branch(&path, pr_branch)?;

    let body = release_pr_body(&plan, &changed_manifests);
    let pr_url = github::create_or_update_release_pr(
        &path,
        &cfg.github.token_env,
        base_branch,
        pr_branch,
        &title,
        &body,
    )?;

    println!("release pr: {}", pr_url);
    Ok(())
}

fn release_pr_body(plan: &release::ReleasePlan, changed_manifests: &[PathBuf]) -> String {
    let mut body = String::new();
    body.push_str("## Release PR\n\n");
    body.push_str(&format!("- Current version: `{}`\n", plan.current_version));
    body.push_str(&format!("- Next version: `{}`\n", plan.next_version));
    body.push_str(&format!("- Bump: `{}`\n\n", plan.bump_level));
    body.push_str("## Changed manifests\n");
    for manifest in changed_manifests {
        body.push_str(&format!("- `{}`\n", manifest.display()));
    }
    body.push('\n');
    body.push_str(&changelog::render_markdown(plan));
    body
}

fn run_release(path: PathBuf, from_tag: Option<String>, dry_run: bool, push: bool) -> Result<()> {
    let cfg = load_config(&path)?;
    let plan = release::build_release_plan(&path, from_tag.as_deref())?;
    let tag_name = cfg
        .release
        .tag_template
        .replace("{{ version }}", &plan.next_version.to_string());

    println!("release plan:");
    println!("- next-version: {}", plan.next_version);
    println!("- tag: {}", tag_name);
    println!("- bump: {}", plan.bump_level);
    println!("- commits: {}", plan.commits.len());

    if dry_run {
        println!("dry-run enabled, no git tag created");
        return Ok(());
    }

    git::create_annotated_tag(&path, &tag_name, &format!("Release {}", plan.next_version))?;
    println!("created tag {}", tag_name);
    if push {
        git::push_tag(&path, &tag_name)?;
        println!("pushed tag {} to origin", tag_name);
    } else {
        println!(
            "push with: git -C {} push origin {}",
            path.display(),
            tag_name
        );
    }

    if cfg.github.create_release {
        println!(
            "github release enabled in config. use your CI token ({}) to publish release notes.",
            cfg.github.token_env
        );
    }
    Ok(())
}

fn run_publish(path: PathBuf, dry_run: bool, push: bool) -> Result<()> {
    let cfg = load_config(&path)?;
    let version = release::current_version(&path)?;
    let tag_name = cfg
        .release
        .tag_template
        .replace("{{ version }}", &version.to_string());

    println!("publish plan:");
    println!("- version: {}", version);
    println!("- tag: {}", tag_name);

    if dry_run {
        println!("dry-run enabled, no tag or github release created");
        return Ok(());
    }

    git::create_annotated_tag(&path, &tag_name, &format!("Release {}", version))?;
    println!("created tag {}", tag_name);

    if push {
        git::push_tag(&path, &tag_name)?;
        println!("pushed tag {} to origin", tag_name);
    }

    if cfg.github.create_release {
        let title = format!("Release {}", version);
        let notes = format!(
            "Automated publish for `{}`.\n\nThis release is created after the release PR merge.",
            version
        );
        let release_url = github::create_or_update_release(
            &path,
            &cfg.github.token_env,
            &tag_name,
            &title,
            &notes,
        )?;
        println!("github release: {}", release_url);
    }

    Ok(())
}
