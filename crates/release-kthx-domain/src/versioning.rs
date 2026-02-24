use crate::{CommitInput, PlannedCommit, ReleasePlan};
use semver::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BumpLevel {
    None,
    Patch,
    Minor,
    Major,
}

impl std::fmt::Display for BumpLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Patch => write!(f, "patch"),
            Self::Minor => write!(f, "minor"),
            Self::Major => write!(f, "major"),
        }
    }
}

impl BumpLevel {
    pub fn apply(self, current: &Version) -> Version {
        let mut next = current.clone();
        match self {
            Self::None => {}
            Self::Patch => next.patch += 1,
            Self::Minor => {
                next.minor += 1;
                next.patch = 0;
            }
            Self::Major => {
                next.major += 1;
                next.minor = 0;
                next.patch = 0;
            }
        }
        next
    }
}

pub fn plan_release(
    current_version: Version,
    base_tag: Option<String>,
    commits: Vec<CommitInput>,
) -> Option<ReleasePlan> {
    if commits.is_empty() {
        return None;
    }

    let mut bump = BumpLevel::None;
    let mut planned = Vec::with_capacity(commits.len());

    for input in commits {
        let (item, local_bump) = PlannedCommit::from_input(input);
        bump = bump.max(local_bump);
        planned.push(item);
    }

    if bump == BumpLevel::None {
        bump = BumpLevel::Patch;
    }

    let next_version = bump.apply(&current_version);

    Some(ReleasePlan {
        base_tag,
        current_version,
        next_version,
        bump_level: bump,
        commits: planned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bumps_minor_for_feature() {
        let plan = plan_release(
            Version::parse("1.2.3").expect("valid semver"),
            None,
            vec![CommitInput {
                hash: "abc".to_string(),
                subject: "feat: add thing".to_string(),
                body: String::new(),
            }],
        )
        .expect("plan should exist");

        assert_eq!(plan.bump_level, BumpLevel::Minor);
        assert_eq!(plan.next_version.to_string(), "1.3.0");
    }
}
