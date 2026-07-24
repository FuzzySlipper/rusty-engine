use std::collections::{BTreeMap, BTreeSet};

use core_assets::AssetId;

use crate::AssetCatalog;

/// Deterministic dependency graph derived from an asset catalog.
pub struct DependencyGraph<'a> {
    edges: BTreeMap<&'a str, Vec<&'a str>>,
    ids: BTreeMap<&'a str, &'a AssetId>,
}

impl<'a> DependencyGraph<'a> {
    pub fn build(catalog: &'a AssetCatalog) -> Self {
        let ids: BTreeMap<&str, &AssetId> = catalog
            .entries
            .iter()
            .map(|entry| (entry.id.as_str(), &entry.id))
            .collect();
        let mut edges = BTreeMap::new();
        for entry in &catalog.entries {
            let mut dependencies: Vec<&str> = entry
                .dependencies
                .iter()
                .map(|dependency| dependency.id().as_str())
                .filter(|dependency| ids.contains_key(dependency))
                .collect();
            dependencies.sort_unstable();
            dependencies.dedup();
            edges.insert(entry.id.as_str(), dependencies);
        }
        Self { edges, ids }
    }

    /// Returns the first deterministic closed cycle path (`a, b, a`).
    pub fn detect_cycle(&self) -> Option<Vec<AssetId>> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Color {
            White,
            Grey,
            Black,
        }

        let mut colors: BTreeMap<&str, Color> =
            self.ids.keys().map(|id| (*id, Color::White)).collect();
        for &root in self.ids.keys() {
            if colors[root] != Color::White {
                continue;
            }
            let mut stack = vec![(root, 0usize)];
            let mut path = vec![root];
            colors.insert(root, Color::Grey);

            while let Some(&(node, child_index)) = stack.last() {
                let children = self.edges.get(node).map(Vec::as_slice).unwrap_or(&[]);
                if child_index < children.len() {
                    stack.last_mut().expect("stack is non-empty").1 += 1;
                    let child = children[child_index];
                    match colors[child] {
                        Color::White => {
                            colors.insert(child, Color::Grey);
                            stack.push((child, 0));
                            path.push(child);
                        }
                        Color::Grey => {
                            let start = path
                                .iter()
                                .position(|candidate| *candidate == child)
                                .expect("grey node is on the active path");
                            let mut cycle: Vec<AssetId> = path[start..]
                                .iter()
                                .map(|id| (*self.ids[id]).clone())
                                .collect();
                            cycle.push((*self.ids[child]).clone());
                            return Some(cycle);
                        }
                        Color::Black => {}
                    }
                } else {
                    colors.insert(node, Color::Black);
                    stack.pop();
                    path.pop();
                }
            }
        }
        None
    }

    /// Transitively dependent asset IDs, sorted and excluding `target`.
    pub fn dependents_of(&self, target: &AssetId) -> Vec<AssetId> {
        let mut reverse: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (&source, dependencies) in &self.edges {
            for &dependency in dependencies {
                reverse.entry(dependency).or_default().push(source);
            }
        }
        let mut seen = BTreeSet::new();
        let mut pending = vec![target.as_str()];
        while let Some(node) = pending.pop() {
            for &dependent in reverse.get(node).map(Vec::as_slice).unwrap_or(&[]) {
                if seen.insert(dependent) {
                    pending.push(dependent);
                }
            }
        }
        seen.into_iter()
            .filter_map(|id| self.ids.get(id).map(|asset| (*asset).clone()))
            .collect()
    }
}
