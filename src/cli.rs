use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "release-kthx",
    about = "Private-repo-first release automation for Rust repositories"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Init {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    Check {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    Plan {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        from_tag: Option<String>,
    },
    Release {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        from_tag: Option<String>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        push: bool,
    },
}
