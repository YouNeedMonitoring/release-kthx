mod changelog;
mod cli;
mod config;
mod git;
mod release;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use config::ReleaseKthxConfig;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init { path, force } => {
            let destination = path.join("release-kthx.toml");
            config::init_config(&destination, force)?;
            println!("wrote {}", destination.display());
        }
        Command::Check { path } => {
            let config_path = path.join("release-kthx.toml");
            let cfg = ReleaseKthxConfig::from_path(&config_path)?;
            cfg.validate()?;
            println!("config valid: {}", config_path.display());
        }
        Command::Plan { path, from_tag } => {
            let config_path = path.join("release-kthx.toml");
            let cfg = ReleaseKthxConfig::from_path(&config_path)?;
            cfg.validate()?;

            let plan = release::build_release_plan(&path, from_tag.as_deref())?;
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
        }
        Command::Release {
            path,
            from_tag,
            dry_run,
            push,
        } => {
            let config_path = path.join("release-kthx.toml");
            let cfg = ReleaseKthxConfig::from_path(&config_path)?;
            cfg.validate()?;

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
        }
    }

    Ok(())
}
