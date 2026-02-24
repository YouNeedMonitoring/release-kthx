use crate::{BumpLevel, PlannedCommit};
use semver::Version;

#[derive(Debug, Clone)]
pub struct ReleasePlan {
    pub base_tag: Option<String>,
    pub current_version: Version,
    pub next_version: Version,
    pub bump_level: BumpLevel,
    pub commits: Vec<PlannedCommit>,
}
