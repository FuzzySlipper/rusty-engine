//! A catalog list whose entries are found by id: adding or removing one
//! entry costs that entry, not a pass over the list.

use std::{
    collections::{BTreeSet, HashMap},
    ops::Deref,
    sync::Arc,
};

use render_model::{
    AnimatedMeshAsset, RenderMaterialDescriptor, ShaderDescriptor, SpriteAtlasDescriptor,
    StaticMeshAsset, TextureDescriptor,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A catalog entry's id.
pub trait CatalogEntry {
    fn catalog_id(&self) -> &str;
}

impl CatalogEntry for RenderMaterialDescriptor {
    fn catalog_id(&self) -> &str {
        &self.id
    }
}

impl CatalogEntry for TextureDescriptor {
    fn catalog_id(&self) -> &str {
        &self.id
    }
}

impl CatalogEntry for ShaderDescriptor {
    fn catalog_id(&self) -> &str {
        &self.id
    }
}

impl CatalogEntry for SpriteAtlasDescriptor {
    fn catalog_id(&self) -> &str {
        &self.id
    }
}

impl CatalogEntry for Arc<StaticMeshAsset> {
    fn catalog_id(&self) -> &str {
        &self.asset
    }
}

impl CatalogEntry for Arc<AnimatedMeshAsset> {
    fn catalog_id(&self) -> &str {
        &self.asset
    }
}

/// Catalog entries in a list, each id's position indexed. Removal moves the
/// last entry into the gap, so the order is not insertion order; the
/// projection reads entries by id. The list remembers which ids changed
/// since the projection last took them, so it reconciles only those.
#[derive(Debug, Clone)]
pub struct ResourceList<T> {
    entries: Vec<T>,
    positions: HashMap<String, usize>,
    changed: BTreeSet<String>,
    /// Built or replaced as a whole: every id, and every id the projection
    /// held before, may have changed.
    whole: bool,
}

/// The ids of a list that changed since the projection last took them.
pub(crate) enum ResourceChanges<'a> {
    Ids(&'a BTreeSet<String>),
    Whole,
}

impl<T> Default for ResourceList<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            positions: HashMap::new(),
            changed: BTreeSet::new(),
            whole: true,
        }
    }
}

impl<T: CatalogEntry> ResourceList<T> {
    /// Adds an entry, or replaces the entry with the same id.
    pub fn push(&mut self, entry: T) {
        let id = entry.catalog_id().to_owned();
        match self.positions.get(&id) {
            Some(position) => self.entries[*position] = entry,
            None => {
                self.positions.insert(id.clone(), self.entries.len());
                self.entries.push(entry);
            }
        }
        self.changed.insert(id);
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.positions
            .get(id)
            .map(|position| &self.entries[*position])
    }

    /// The entry with this id, to change in place. Its id must not change.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut T> {
        let position = *self.positions.get(id)?;
        self.changed.insert(id.to_owned());
        Some(&mut self.entries[position])
    }

    pub fn contains(&self, id: &str) -> bool {
        self.positions.contains_key(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<T> {
        let position = self.positions.remove(id)?;
        let entry = self.entries.swap_remove(position);
        if let Some(moved) = self.entries.get(position) {
            self.positions
                .insert(moved.catalog_id().to_owned(), position);
        }
        self.changed.insert(id.to_owned());
        Some(entry)
    }

    pub(crate) fn changes(&self) -> ResourceChanges<'_> {
        if self.whole {
            ResourceChanges::Whole
        } else {
            ResourceChanges::Ids(&self.changed)
        }
    }

    /// Every entry counts as changed, for a projection that holds none.
    pub(crate) fn mark_whole(&mut self) {
        self.whole = true;
    }

    /// The projection holds every change so far.
    pub(crate) fn clear_changes(&mut self) {
        self.changed.clear();
        self.whole = false;
    }

    fn reindex(&mut self) {
        self.positions.clear();
        let mut kept = Vec::with_capacity(self.entries.len());
        for entry in std::mem::take(&mut self.entries) {
            // A later entry with the same id replaces an earlier one.
            match self.positions.get(entry.catalog_id()) {
                Some(position) => kept[*position] = entry,
                None => {
                    self.positions
                        .insert(entry.catalog_id().to_owned(), kept.len());
                    kept.push(entry);
                }
            }
        }
        self.entries = kept;
    }
}

impl<T> Deref for ResourceList<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.entries
    }
}

impl<T: PartialEq> PartialEq for ResourceList<T> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl<T: CatalogEntry> Extend<T> for ResourceList<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, entries: I) {
        for entry in entries {
            self.push(entry);
        }
    }
}

impl<T: CatalogEntry> FromIterator<T> for ResourceList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(entries: I) -> Self {
        let mut list = Self::default();
        list.extend(entries);
        list
    }
}

impl<T: CatalogEntry> From<Vec<T>> for ResourceList<T> {
    fn from(entries: Vec<T>) -> Self {
        let mut list = Self {
            entries,
            ..Self::default()
        };
        list.reindex();
        list
    }
}

impl<'a, T> IntoIterator for &'a ResourceList<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<T: Serialize> Serialize for ResourceList<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.entries.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de> + CatalogEntry> Deserialize<'de> for ResourceList<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<T>::deserialize(deserializer).map(Self::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shader(id: &str) -> ShaderDescriptor {
        ShaderDescriptor {
            id: id.to_owned(),
            path: "shaders/a.wgsl".to_owned(),
            source: String::new(),
            keywords: Vec::new(),
        }
    }

    #[test]
    fn entries_are_found_and_removed_by_id() {
        let mut list: ResourceList<ShaderDescriptor> =
            ["a", "b", "c", "d"].into_iter().map(shader).collect();
        assert_eq!(list.remove("b").map(|entry| entry.id), Some("b".to_owned()));
        assert_eq!(list.remove("b"), None);
        // The last entry moved into the gap and is still found.
        assert_eq!(list.get("d").map(|entry| entry.id.as_str()), Some("d"));
        assert_eq!(list.len(), 3);
        list.remove("d");
        list.push(shader("e"));
        let mut ids: Vec<_> = list.iter().map(|entry| entry.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(ids, ["a", "c", "e"]);
        assert!(["a", "c", "e"]
            .iter()
            .all(|id| list.get(id).is_some_and(|entry| entry.id == *id)));
    }

    #[test]
    fn a_pushed_id_replaces_its_entry_and_changes_are_remembered() {
        let mut list: ResourceList<ShaderDescriptor> =
            vec![shader("a"), shader("b"), shader("a")].into();
        assert_eq!(list.len(), 2, "a later entry replaces an earlier one");
        assert!(matches!(list.changes(), ResourceChanges::Whole));
        list.clear_changes();
        assert!(matches!(list.changes(), ResourceChanges::Ids(ids) if ids.is_empty()));
        let mut replaced = shader("b");
        replaced.path = "shaders/b.wgsl".to_owned();
        list.push(replaced);
        assert_eq!(list.len(), 2);
        assert_eq!(list.get("b").unwrap().path, "shaders/b.wgsl");
        list.remove("a");
        list.get_mut("b");
        list.get_mut("missing");
        let ResourceChanges::Ids(ids) = list.changes() else {
            panic!("only some ids changed");
        };
        assert_eq!(
            ids.iter().map(String::as_str).collect::<Vec<_>>(),
            ["a", "b"]
        );
    }
}
