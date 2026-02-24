use crate::release::{CommitKind, ReleasePlan};

pub fn render_markdown(plan: &ReleasePlan) -> String {
    let mut out = String::new();
    out.push_str(&format!("## {}\n\n", plan.next_version));

    if let Some(base_tag) = &plan.base_tag {
        out.push_str(&format!("Changes since `{}`\n\n", base_tag));
    }

    append_section(&mut out, "Features", plan, CommitKind::Feature);
    append_section(&mut out, "Fixes", plan, CommitKind::Fix);
    append_section(&mut out, "Refactors", plan, CommitKind::Refactor);
    append_section(&mut out, "Documentation", plan, CommitKind::Documentation);
    append_section(&mut out, "Chores", plan, CommitKind::Chore);
    append_section(&mut out, "Other", plan, CommitKind::Other);

    out
}

fn append_section(out: &mut String, title: &str, plan: &ReleasePlan, kind: CommitKind) {
    let mut wrote_any = false;
    for commit in &plan.commits {
        if commit.kind == kind {
            if !wrote_any {
                out.push_str(&format!("### {}\n", title));
                wrote_any = true;
            }
            if commit.breaking {
                out.push_str(&format!(
                    "- {} ({}) **BREAKING**\n",
                    commit.subject,
                    short_hash(&commit.hash)
                ));
            } else {
                out.push_str(&format!(
                    "- {} ({})\n",
                    commit.subject,
                    short_hash(&commit.hash)
                ));
            }
        }
    }
    if wrote_any {
        out.push('\n');
    }
}

fn short_hash(hash: &str) -> &str {
    let max = std::cmp::min(7, hash.len());
    &hash[..max]
}

#[cfg(test)]
mod tests {
    use super::*;
    use release_kthx_domain::{BumpLevel, PlannedCommit};
    use semver::Version;

    #[test]
    fn renders_feature_section() {
        let plan = ReleasePlan {
            base_tag: Some("v0.1.0".to_string()),
            current_version: Version::parse("0.1.0").expect("valid semver"),
            next_version: Version::parse("0.2.0").expect("valid semver"),
            bump_level: BumpLevel::Minor,
            commits: vec![PlannedCommit {
                hash: "abcdef123456".to_string(),
                subject: "feat: add private release mode".to_string(),
                kind: CommitKind::Feature,
                breaking: false,
            }],
        };

        let text = render_markdown(&plan);
        assert!(text.contains("### Features"));
        assert!(text.contains("add private release mode"));
    }
}
