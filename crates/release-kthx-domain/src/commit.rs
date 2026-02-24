#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInput {
    pub hash: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitKind {
    Feature,
    Fix,
    Refactor,
    Documentation,
    Chore,
    Other,
}

impl std::fmt::Display for CommitKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Feature => write!(f, "feat"),
            Self::Fix => write!(f, "fix"),
            Self::Refactor => write!(f, "refactor"),
            Self::Documentation => write!(f, "docs"),
            Self::Chore => write!(f, "chore"),
            Self::Other => write!(f, "other"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedCommit {
    pub hash: String,
    pub subject: String,
    pub kind: CommitKind,
    pub breaking: bool,
}

impl PlannedCommit {
    pub fn from_input(input: CommitInput) -> (Self, super::BumpLevel) {
        let lower_subject = input.subject.to_lowercase();
        let head = lower_subject
            .split(':')
            .next()
            .map(str::trim)
            .unwrap_or_default();

        let kind = kind_from_head(head);
        let breaking = head.contains('!') || input.body.contains("BREAKING CHANGE");
        let bump = if breaking {
            super::BumpLevel::Major
        } else {
            match kind {
                CommitKind::Feature => super::BumpLevel::Minor,
                CommitKind::Fix | CommitKind::Refactor => super::BumpLevel::Patch,
                CommitKind::Documentation | CommitKind::Chore | CommitKind::Other => {
                    super::BumpLevel::None
                }
            }
        };

        (
            Self {
                hash: input.hash,
                subject: input.subject,
                kind,
                breaking,
            },
            bump,
        )
    }
}

fn kind_from_head(head: &str) -> CommitKind {
    let ty = head
        .split('(')
        .next()
        .unwrap_or_default()
        .trim_end_matches('!')
        .trim();
    match ty {
        "feat" => CommitKind::Feature,
        "fix" => CommitKind::Fix,
        "refactor" | "perf" => CommitKind::Refactor,
        "docs" => CommitKind::Documentation,
        "chore" | "ci" | "build" | "test" => CommitKind::Chore,
        _ => CommitKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_is_minor() {
        let (planned, bump) = PlannedCommit::from_input(CommitInput {
            hash: "a".to_string(),
            subject: "feat(api): add endpoint".to_string(),
            body: String::new(),
        });
        assert_eq!(planned.kind, CommitKind::Feature);
        assert_eq!(bump, super::super::BumpLevel::Minor);
    }

    #[test]
    fn bang_is_major() {
        let (planned, bump) = PlannedCommit::from_input(CommitInput {
            hash: "a".to_string(),
            subject: "fix!: breaking fix".to_string(),
            body: String::new(),
        });
        assert!(planned.breaking);
        assert_eq!(bump, super::super::BumpLevel::Major);
    }
}
