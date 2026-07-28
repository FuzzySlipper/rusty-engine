use std::collections::{BTreeMap, BTreeSet};

use crate::{AdmittedRulePackage, RulePackageIdentity, RulePackageSetError};

pub const MAX_RULE_PACKAGES_PER_SET: usize = 64;
pub const MAX_CANONICAL_RULE_PACKAGE_SET_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DEPENDENCIES_PER_RULE_PACKAGE_SET: usize = 512;
pub const MAX_SOURCES_PER_RULE_PACKAGE_SET: usize = 1_024;
pub const MAX_PROVENANCE_PER_RULE_PACKAGE_SET: usize = 16_384;
pub const MAX_JSON_NODES_PER_RULE_PACKAGE_SET: usize = 400_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRulePackages {
    packages: Vec<AdmittedRulePackage>,
    canonical_bytes: usize,
    dependencies: usize,
    sources: usize,
    provenance: usize,
    json_nodes: usize,
}

impl ResolvedRulePackages {
    pub fn packages(&self) -> &[AdmittedRulePackage] {
        &self.packages
    }

    pub fn package(&self, identity: &RulePackageIdentity) -> Option<&AdmittedRulePackage> {
        self.packages
            .iter()
            .find(|package| package.identity() == identity)
    }

    pub fn into_packages(self) -> Vec<AdmittedRulePackage> {
        self.packages
    }

    pub const fn canonical_bytes(&self) -> usize {
        self.canonical_bytes
    }

    pub const fn dependency_count(&self) -> usize {
        self.dependencies
    }

    pub const fn source_count(&self) -> usize {
        self.sources
    }

    pub const fn provenance_count(&self) -> usize {
        self.provenance
    }

    pub const fn json_nodes(&self) -> usize {
        self.json_nodes
    }
}

pub fn resolve_rule_packages(
    mut packages: Vec<AdmittedRulePackage>,
) -> Result<ResolvedRulePackages, RulePackageSetError> {
    enforce_aggregate_quota("packages", packages.len(), MAX_RULE_PACKAGES_PER_SET)?;

    packages.sort_by(|left, right| left.identity().cmp(right.identity()));
    validate_unique_identities(&packages)?;

    let canonical_bytes = checked_aggregate(
        &packages,
        "canonical bytes",
        MAX_CANONICAL_RULE_PACKAGE_SET_BYTES,
        |package| package.canonical_bytes().len(),
    )?;
    let dependencies = checked_aggregate(
        &packages,
        "dependencies",
        MAX_DEPENDENCIES_PER_RULE_PACKAGE_SET,
        |package| package.dependencies().len(),
    )?;
    let sources = checked_aggregate(
        &packages,
        "sources",
        MAX_SOURCES_PER_RULE_PACKAGE_SET,
        |package| package.sources().len(),
    )?;
    let provenance = checked_aggregate(
        &packages,
        "provenance",
        MAX_PROVENANCE_PER_RULE_PACKAGE_SET,
        |package| package.provenance().len(),
    )?;
    let json_nodes = checked_aggregate(
        &packages,
        "JSON nodes",
        MAX_JSON_NODES_PER_RULE_PACKAGE_SET,
        AdmittedRulePackage::json_nodes,
    )?;

    let identity_indexes = packages
        .iter()
        .enumerate()
        .map(|(index, package)| (package.identity().clone(), index))
        .collect::<BTreeMap<_, _>>();
    let logical_indexes = packages
        .iter()
        .enumerate()
        .map(|(index, package)| {
            (
                (
                    package.identity().domain().clone(),
                    package.identity().package().clone(),
                ),
                index,
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut indegrees = vec![0_usize; packages.len()];
    let mut dependents = vec![Vec::new(); packages.len()];
    for (package_index, package) in packages.iter().enumerate() {
        for dependency in package.dependencies() {
            let logical_key = (dependency.domain().clone(), dependency.package().clone());
            let Some(&dependency_index) = logical_indexes.get(&logical_key) else {
                return Err(RulePackageSetError::MissingDependency {
                    package: package.identity().clone(),
                    dependency: Box::new(dependency.clone()),
                });
            };
            let available = &packages[dependency_index];
            if available.identity().version() != dependency.version() {
                return Err(RulePackageSetError::DependencyVersionMismatch {
                    package: package.identity().clone(),
                    dependency: Box::new(dependency.clone()),
                    available: available.identity().version(),
                });
            }
            if let Some(expected) = dependency.fingerprint() {
                if available.fingerprint() != expected {
                    return Err(RulePackageSetError::DependencyFingerprintMismatch {
                        package: package.identity().clone(),
                        dependency: Box::new(dependency.clone()),
                        actual: available.fingerprint().clone(),
                    });
                }
            }
            indegrees[package_index] = indegrees[package_index].checked_add(1).ok_or(
                RulePackageSetError::ArithmeticOverflow {
                    field: "dependency indegree",
                },
            )?;
            dependents[dependency_index].push(package_index);
        }
    }

    let mut ready = packages
        .iter()
        .enumerate()
        .filter(|(index, _)| indegrees[*index] == 0)
        .map(|(index, package)| (package.identity().clone(), index))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(packages.len());
    while let Some((identity, index)) = ready.pop_first() {
        debug_assert_eq!(&identity, packages[index].identity());
        order.push(index);
        for &dependent in &dependents[index] {
            indegrees[dependent] -= 1;
            if indegrees[dependent] == 0 {
                ready.insert((packages[dependent].identity().clone(), dependent));
            }
        }
    }

    if order.len() != packages.len() {
        return Err(RulePackageSetError::DependencyCycle {
            packages: find_dependency_cycle(&packages, &identity_indexes),
        });
    }

    let mut packages = packages.into_iter().map(Some).collect::<Vec<_>>();
    let packages = order
        .into_iter()
        .map(|index| {
            packages[index]
                .take()
                .expect("topological indexes are unique")
        })
        .collect();
    Ok(ResolvedRulePackages {
        packages,
        canonical_bytes,
        dependencies,
        sources,
        provenance,
        json_nodes,
    })
}

fn validate_unique_identities(packages: &[AdmittedRulePackage]) -> Result<(), RulePackageSetError> {
    for pair in packages.windows(2) {
        let first = pair[0].identity();
        let second = pair[1].identity();
        if first == second {
            return Err(RulePackageSetError::DuplicatePackage {
                package: second.clone(),
            });
        }
        if first.domain() == second.domain() && first.package() == second.package() {
            return Err(RulePackageSetError::ConflictingVersions {
                domain: second.domain().clone(),
                package: second.package().clone(),
                first: first.version(),
                second: second.version(),
            });
        }
    }
    Ok(())
}

fn checked_aggregate(
    packages: &[AdmittedRulePackage],
    field: &'static str,
    maximum: usize,
    cost: impl Fn(&AdmittedRulePackage) -> usize,
) -> Result<usize, RulePackageSetError> {
    let mut total = 0_usize;
    for package in packages {
        total = total
            .checked_add(cost(package))
            .ok_or(RulePackageSetError::ArithmeticOverflow { field })?;
        enforce_aggregate_quota(field, total, maximum)?;
    }
    Ok(total)
}

fn enforce_aggregate_quota(
    field: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), RulePackageSetError> {
    if actual > maximum {
        return Err(RulePackageSetError::AggregateQuotaExceeded {
            field,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn find_dependency_cycle(
    packages: &[AdmittedRulePackage],
    identity_indexes: &BTreeMap<RulePackageIdentity, usize>,
) -> Vec<RulePackageIdentity> {
    let mut states = vec![VisitState::Unvisited; packages.len()];
    let mut stack = Vec::new();
    for index in 0..packages.len() {
        if states[index] == VisitState::Unvisited {
            if let Some(cycle) =
                visit_dependencies(index, packages, identity_indexes, &mut states, &mut stack)
            {
                return cycle;
            }
        }
    }
    Vec::new()
}

fn visit_dependencies(
    index: usize,
    packages: &[AdmittedRulePackage],
    identity_indexes: &BTreeMap<RulePackageIdentity, usize>,
    states: &mut [VisitState],
    stack: &mut Vec<usize>,
) -> Option<Vec<RulePackageIdentity>> {
    states[index] = VisitState::Visiting;
    stack.push(index);
    for dependency in packages[index].dependencies() {
        let dependency_index = *identity_indexes
            .get(dependency.identity())
            .expect("dependencies were validated before cycle detection");
        match states[dependency_index] {
            VisitState::Unvisited => {
                if let Some(cycle) =
                    visit_dependencies(dependency_index, packages, identity_indexes, states, stack)
                {
                    return Some(cycle);
                }
            }
            VisitState::Visiting => {
                let cycle_start = stack
                    .iter()
                    .position(|candidate| *candidate == dependency_index)
                    .expect("visiting dependency is on the traversal stack");
                let mut cycle = stack[cycle_start..]
                    .iter()
                    .map(|candidate| packages[*candidate].identity().clone())
                    .collect::<Vec<_>>();
                cycle.push(packages[dependency_index].identity().clone());
                return Some(cycle);
            }
            VisitState::Visited => {}
        }
    }
    stack.pop();
    states[index] = VisitState::Visited;
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Unvisited,
    Visiting,
    Visited,
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::{
        admit_rule_package, RuleDomainId, RulePackageCandidate, RulePackageId, RuleVersion,
    };

    #[test]
    fn aggregate_arithmetic_overflow_is_typed_before_graph_work() {
        let package = admit_rule_package(RulePackageCandidate::new(
            RuleDomainId::parse("test").unwrap(),
            RulePackageId::parse("overflow").unwrap(),
            RuleVersion::new(1).unwrap(),
            vec![],
            vec![],
            vec![],
            Value::Null,
        ))
        .unwrap();
        assert!(matches!(
            checked_aggregate(
                &[package.clone(), package],
                "synthetic cost",
                usize::MAX,
                |_| usize::MAX,
            ),
            Err(RulePackageSetError::ArithmeticOverflow {
                field: "synthetic cost"
            })
        ));
    }
}
