use semver::{Comparator, Version, VersionReq};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InternalDependencyPolicy {
    #[default]
    Auto,
    Strip,
    Update,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Publication {
    Private,
    Publishable,
}

impl Publication {
    pub fn from_private(private: bool) -> Self {
        if private {
            Self::Private
        } else {
            Self::Publishable
        }
    }

    pub fn is_private(self) -> bool {
        matches!(self, Self::Private)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyOwner {
    Member { publication: Publication },
    UnknownMember,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InternalDependencyContext {
    pub owner: DependencyOwner,
    pub dependency_publication: Publication,
    pub all_members_private: bool,
}

impl InternalDependencyContext {
    pub fn should_strip_when_auto(self) -> bool {
        match self.owner {
            DependencyOwner::Member { publication } => {
                publication.is_private() && self.dependency_publication.is_private()
            }
            DependencyOwner::Workspace => {
                self.all_members_private && self.dependency_publication.is_private()
            }
            DependencyOwner::UnknownMember => false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RequirementStyle {
    operator: RequirementOperator,
    precision: VersionPrecision,
}

impl RequirementStyle {
    pub fn parse(requirement: &str) -> Result<Self, RequirementStyleParseError> {
        let trimmed = requirement.trim();
        if trimmed.is_empty() {
            return Ok(Self::default());
        }

        let parsed = VersionReq::parse(trimmed)
            .map_err(|_| RequirementStyleParseError::unsupported(trimmed))?;
        if parsed.comparators.len() != 1 {
            return Err(RequirementStyleParseError::unsupported(trimmed));
        }

        let comparator = &parsed.comparators[0];
        Ok(Self {
            operator: RequirementOperator::parse(trimmed),
            precision: VersionPrecision::from_comparator(comparator),
        })
    }

    pub fn render(self, version: &Version) -> String {
        format!(
            "{}{}",
            self.operator.prefix(),
            self.precision.render(version)
        )
    }
}

pub fn desired_requirement_style(
    policy: InternalDependencyPolicy,
    context: InternalDependencyContext,
    current_style: Option<RequirementStyle>,
) -> Option<RequirementStyle> {
    match policy {
        InternalDependencyPolicy::Strip => None,
        InternalDependencyPolicy::Update => Some(current_style.unwrap_or_default()),
        InternalDependencyPolicy::Auto if context.should_strip_when_auto() => None,
        InternalDependencyPolicy::Auto => current_style,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum RequirementOperator {
    #[default]
    Bare,
    GreaterEq,
    LessEq,
    Caret,
    Tilde,
    Exact,
    Greater,
    Less,
}

impl RequirementOperator {
    fn parse(requirement: &str) -> Self {
        let trimmed = requirement.trim();
        if trimmed.starts_with(">=") {
            Self::GreaterEq
        } else if trimmed.starts_with("<=") {
            Self::LessEq
        } else if trimmed.starts_with('^') {
            Self::Caret
        } else if trimmed.starts_with('~') {
            Self::Tilde
        } else if trimmed.starts_with('=') {
            Self::Exact
        } else if trimmed.starts_with('>') {
            Self::Greater
        } else if trimmed.starts_with('<') {
            Self::Less
        } else {
            Self::Bare
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Bare => "",
            Self::GreaterEq => ">=",
            Self::LessEq => "<=",
            Self::Caret => "^",
            Self::Tilde => "~",
            Self::Exact => "=",
            Self::Greater => ">",
            Self::Less => "<",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum VersionPrecision {
    Major,
    Minor,
    #[default]
    Patch,
}

impl VersionPrecision {
    fn from_comparator(comparator: &Comparator) -> Self {
        if comparator.patch.is_some() {
            Self::Patch
        } else if comparator.minor.is_some() {
            Self::Minor
        } else {
            Self::Major
        }
    }

    fn render(self, version: &Version) -> String {
        match self {
            Self::Major => version.major.to_string(),
            Self::Minor => format!("{}.{}", version.major, version.minor),
            Self::Patch => version.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementStyleParseError {
    requirement: String,
}

impl RequirementStyleParseError {
    fn unsupported(requirement: &str) -> Self {
        Self {
            requirement: requirement.to_string(),
        }
    }
}

impl std::fmt::Display for RequirementStyleParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unsupported internal dependency version requirement `{}`",
            self.requirement
        )
    }
}

impl std::error::Error for RequirementStyleParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_policy_strips_private_member_dependencies() {
        let context = InternalDependencyContext {
            owner: DependencyOwner::Member {
                publication: Publication::Private,
            },
            dependency_publication: Publication::Private,
            all_members_private: true,
        };

        assert_eq!(
            desired_requirement_style(
                InternalDependencyPolicy::Auto,
                context,
                Some(RequirementStyle::parse("^0.4.0").expect("parse style")),
            ),
            None,
        );
    }

    #[test]
    fn auto_policy_preserves_existing_style_for_publishable_edges() {
        let context = InternalDependencyContext {
            owner: DependencyOwner::Member {
                publication: Publication::Publishable,
            },
            dependency_publication: Publication::Private,
            all_members_private: false,
        };

        let style = RequirementStyle::parse("^0.4").expect("parse style");
        assert_eq!(
            desired_requirement_style(InternalDependencyPolicy::Auto, context, Some(style)),
            Some(style),
        );
    }

    #[test]
    fn update_policy_inserts_default_style_when_missing() {
        let context = InternalDependencyContext {
            owner: DependencyOwner::UnknownMember,
            dependency_publication: Publication::Publishable,
            all_members_private: false,
        };

        let next = desired_requirement_style(InternalDependencyPolicy::Update, context, None)
            .expect("style should be present");
        assert_eq!(
            next.render(&Version::parse("0.5.1").expect("valid semver")),
            "0.5.1"
        );
    }

    #[test]
    fn requirement_style_preserves_operator_and_precision() {
        let version = Version::parse("0.5.1").expect("valid semver");
        assert_eq!(
            RequirementStyle::parse("0.4.0")
                .expect("parse style")
                .render(&version),
            "0.5.1"
        );
        assert_eq!(
            RequirementStyle::parse("^0.4")
                .expect("parse style")
                .render(&version),
            "^0.5"
        );
        assert_eq!(
            RequirementStyle::parse("=0.4.0")
                .expect("parse style")
                .render(&version),
            "=0.5.1"
        );
    }

    #[test]
    fn rejects_complex_requirements() {
        let error = RequirementStyle::parse(">=0.4.0, <0.5.0").expect_err("should fail");
        assert_eq!(
            error.to_string(),
            "unsupported internal dependency version requirement `>=0.4.0, <0.5.0`"
        );
    }
}
