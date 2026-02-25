use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceCrate {
    pub name: String,
    pub local_dependencies: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGraph {
    dependencies: BTreeMap<String, BTreeSet<String>>,
    reverse_dependencies: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTopology {
    directly_affected: BTreeSet<String>,
    release_crates: BTreeSet<String>,
}

impl WorkspaceGraph {
    pub fn from_crates<I>(crates: I) -> Self
    where
        I: IntoIterator<Item = WorkspaceCrate>,
    {
        let mut dependencies = BTreeMap::new();
        for crate_info in crates {
            dependencies.insert(crate_info.name, crate_info.local_dependencies);
        }

        let known_crates = dependencies.keys().cloned().collect::<BTreeSet<_>>();
        for (crate_name, local_dependencies) in dependencies.iter_mut() {
            local_dependencies
                .retain(|dependency| dependency != crate_name && known_crates.contains(dependency));
        }

        let mut reverse_dependencies = known_crates
            .iter()
            .cloned()
            .map(|name| (name, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();

        for (crate_name, local_dependencies) in &dependencies {
            for dependency in local_dependencies {
                reverse_dependencies
                    .entry(dependency.clone())
                    .or_default()
                    .insert(crate_name.clone());
            }
        }

        Self {
            dependencies,
            reverse_dependencies,
        }
    }

    pub fn release_topology<I, S>(&self, directly_affected: I) -> ReleaseTopology
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let directly_affected = directly_affected
            .into_iter()
            .map(|name| name.as_ref().to_string())
            .filter(|name| self.dependencies.contains_key(name))
            .collect::<BTreeSet<_>>();

        let mut release_crates = directly_affected.clone();
        let mut queue = directly_affected.iter().cloned().collect::<VecDeque<_>>();

        while let Some(crate_name) = queue.pop_front() {
            if let Some(dependents) = self.reverse_dependencies.get(&crate_name) {
                for dependent in dependents {
                    if release_crates.insert(dependent.clone()) {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }

        ReleaseTopology {
            directly_affected,
            release_crates,
        }
    }
}

impl ReleaseTopology {
    pub fn directly_affected(&self) -> &BTreeSet<String> {
        &self.directly_affected
    }

    pub fn release_crates(&self) -> &BTreeSet<String> {
        &self.release_crates
    }

    pub fn includes(&self, crate_name: &str) -> bool {
        self.release_crates.contains(crate_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_crate(name: &str, deps: &[&str]) -> WorkspaceCrate {
        WorkspaceCrate {
            name: name.to_string(),
            local_dependencies: deps.iter().map(|dep| dep.to_string()).collect(),
        }
    }

    #[test]
    fn release_topology_expands_transitive_dependents() {
        let graph = WorkspaceGraph::from_crates([
            workspace_crate("domain", &[]),
            workspace_crate("api", &["domain"]),
            workspace_crate("app", &["api"]),
        ]);

        let topology = graph.release_topology(["domain"]);

        assert_eq!(
            topology.directly_affected(),
            &BTreeSet::from(["domain".to_string()])
        );
        assert_eq!(
            topology.release_crates(),
            &BTreeSet::from(["app".to_string(), "api".to_string(), "domain".to_string(),])
        );
    }

    #[test]
    fn graph_filters_self_and_unknown_dependencies() {
        let graph = WorkspaceGraph::from_crates([
            workspace_crate("domain", &["domain", "serde"]),
            workspace_crate("app", &["domain"]),
        ]);

        let topology = graph.release_topology(["domain"]);
        assert!(topology.includes("domain"));
        assert!(topology.includes("app"));
        assert!(!topology.includes("serde"));
    }

    #[test]
    fn unknown_directly_affected_crates_are_ignored() {
        let graph = WorkspaceGraph::from_crates([
            workspace_crate("domain", &[]),
            workspace_crate("app", &["domain"]),
        ]);

        let topology = graph.release_topology(["unknown"]);
        assert!(topology.directly_affected().is_empty());
        assert!(topology.release_crates().is_empty());
    }
}
