use std::{
    any::TypeId,
    collections::{BTreeMap, HashSet},
};

use thiserror::Error;

use crate::{
    builder::DiBuilder,
    types::{DependencyInfo, TypeInfo},
};

/// Graph of the entire application
/// Used to check circular dependencies and enables visualization of APP
pub struct DependencyGraph {
    map: BTreeMap<TypeId, DependencyGraphEntry>,
}
impl DependencyGraph {
    pub fn new(builder: &DiBuilder) -> Result<Self, DependencyGraphError> {
        let mut graph = Self {
            map: Default::default(),
        };

        for instance in builder.registered_instances.values() {
            graph.add(instance.info, vec![])?;
        }

        for factory in &builder.registered_factories {
            graph.add(factory.supplies(), factory.dependencies())?;
        }

        Ok(graph)
    }

    pub fn add(
        &mut self,
        info: TypeInfo,
        dependencies: Vec<DependencyInfo>,
    ) -> Result<(), DependencyGraphError> {
        if let Some(existing) = self
            .map
            .insert(info.type_id, DependencyGraphEntry { info, dependencies })
        {
            return Err(DependencyGraphError::Duplicate(existing.info));
        }

        Ok(())
    }

    /// Validate the graph
    ///
    /// Returns a list of all issues
    pub fn check(&self) -> Result<(), DependencyGraphErrors> {
        let mut checked = HashSet::new();
        let mut errors = Vec::new();
        for entry in self.map.values() {
            let mut dependency_chain = Vec::new();
            check_recurse(
                self,
                &mut checked,
                &mut errors,
                &mut dependency_chain,
                entry,
            );
        }

        if !errors.is_empty() {
            return Err(DependencyGraphErrors { errors });
        }

        return Ok(());

        fn check_recurse(
            graph: &DependencyGraph,
            checked: &mut HashSet<TypeId>,
            errors: &mut Vec<DependencyGraphError>,
            dependency_chain: &mut Vec<TypeInfo>,
            entry: &DependencyGraphEntry,
        ) {
            // Circular Dependency Check
            if dependency_chain.contains(&entry.info) {
                let from = *dependency_chain.first().expect("must have entries");
                let to = entry.info;

                dependency_chain.push(to); // Add current so chain is complete

                errors.push(DependencyGraphError::CircularDependency {
                    from,
                    to,
                    chain: dependency_chain.clone(),
                });
            }

            // Skip other checks if already checked
            if !checked.insert(entry.info.type_id) {
                return;
            };

            dependency_chain.push(entry.info);

            for dependency in &entry.dependencies {
                let Some(next_entry) = graph.map.get(&dependency.type_info.type_id) else {
                    if !dependency.optional {
                        errors.push(DependencyGraphError::MissingDependency {
                            dependency: dependency.type_info,
                            required_by: entry.info,
                        });
                    }

                    continue;
                };

                if dependency.lazy {
                    // Don't recurse, this will be checked by itself
                    continue;
                }

                check_recurse(graph, checked, errors, dependency_chain, next_entry);
            }

            dependency_chain.pop();
        }
    }
}

struct DependencyGraphEntry {
    info: TypeInfo,
    dependencies: Vec<DependencyInfo>,
}

#[derive(Error, Debug, Clone)]
pub enum DependencyGraphError {
    #[error("A Type has been registered twice: '{0}'")]
    Duplicate(TypeInfo),
    #[error("'{required_by}' needs '{dependency}' but it is missing")]
    MissingDependency {
        dependency: TypeInfo,
        required_by: TypeInfo,
    },
    #[error("A Circular Dependency exists between '{from}' and '{to}' through {chain:?} - Consider using `Lazy`")]
    CircularDependency {
        from: TypeInfo,
        to: TypeInfo,
        chain: Vec<TypeInfo>,
    },
}
impl std::fmt::Display for DependencyGraphErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut display = Vec::new();
        display.push("The dependency graph had one or more errors:".to_string());
        for error in &self.errors {
            display.push(format!("- {}", error));
        }
        f.write_str(&display.join("\n"))
    }
}

#[derive(Error, Debug, Clone)]
pub struct DependencyGraphErrors {
    pub errors: Vec<DependencyGraphError>,
}
