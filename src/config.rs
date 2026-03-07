use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ReleaseKthxConfig {
    #[serde(default)]
    pub release: ReleaseConfig,
    #[serde(default)]
    pub github: GithubConfig,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InternalDependencyPolicy {
    Auto,
    Strip,
    Update,
}

impl Default for InternalDependencyPolicy {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReleaseConfig {
    pub tag_template: String,
    #[serde(default)]
    pub internal_dependency_policy: InternalDependencyPolicy,
}

impl Default for ReleaseConfig {
    fn default() -> Self {
        Self {
            tag_template: "{{ crate }}-v{{ version }}".to_string(),
            internal_dependency_policy: InternalDependencyPolicy::Auto,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GithubConfig {
    pub create_release: bool,
    pub token_env: String,
    pub repository_env: String,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            create_release: true,
            token_env: "GITHUB_TOKEN".to_string(),
            repository_env: "GITHUB_REPOSITORY".to_string(),
        }
    }
}

impl ReleaseKthxConfig {
    pub fn from_path(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed reading config {}", path.display()))?;
        let cfg = toml::from_str::<Self>(&contents)
            .with_context(|| format!("failed parsing config {}", path.display()))?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<()> {
        if self.release.tag_template.trim().is_empty() {
            bail!("release.tag_template cannot be empty");
        }
        if !self.release.tag_template.contains("{{ version }}") {
            bail!("release.tag_template must contain '{{ version }}'");
        }
        if self.github.token_env.trim().is_empty() {
            bail!("github.token_env cannot be empty");
        }
        if self.github.repository_env.trim().is_empty() {
            bail!("github.repository_env cannot be empty");
        }
        Ok(())
    }
}

pub fn init_config(destination: &Path, force: bool) -> Result<()> {
    if destination.exists() && !force {
        bail!(
            "{} already exists. rerun with --force to overwrite",
            destination.display()
        );
    }

    let cfg = ReleaseKthxConfig::default();
    let content = toml::to_string_pretty(&cfg).context("failed serializing default config")?;
    fs::write(destination, content)
        .with_context(|| format!("failed writing {}", destination.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        let cfg = ReleaseKthxConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn template_requires_version_placeholder() {
        let mut cfg = ReleaseKthxConfig::default();
        cfg.release.tag_template = "release".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn default_policy_is_auto() {
        let cfg = ReleaseKthxConfig::default();
        assert_eq!(
            cfg.release.internal_dependency_policy,
            InternalDependencyPolicy::Auto
        );
    }
}
