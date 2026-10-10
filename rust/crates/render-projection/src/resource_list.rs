//! A catalog list whose entries are found by id: adding or removing one
//! entry costs that entry, not a pass over the list.

use std::{collections::HashMap, ops::Deref, sync::Arc};

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
/// projection reads entries by id. A list with duplicate ids (which
/// projection refuses) indexes the last of them.
#[derive(Debug, Clone)]
pub struct ResourceList<T> {
    entries: Vec<T>,
    positions: HashMap<String, usize>,
}

impl<T> Default for ResourceList<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            positions: HashMap::new(),
        }
    }
}

impl<T: CatalogEntry> ResourceList<T> {
    pub fn push(&mut self, entry: T) {
        self.positions
            .insert(entry.catalog_id().to_owned(), self.entries.len());
        self.entries.push(entry);
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.positions
            .get(id)
            .map(|position| &self.entries[*position])
    }

    /// The entry with this id, to change in place. Its id must not change.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut T> {
        self.positions
            .get(id)
            .map(|position| &mut self.entries[*position])
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
        Some(entry)
    }

    /// Keeps the entries `keep` accepts: a pass over the list.
    pub fn retain(&mut self, keep: impl FnMut(&T) -> bool) {
        self.entries.retain(keep);
        self.reindex();
    }

    fn reindex(&mut self) {
        self.positions = self
            .entries
            .iter()
            .enumerate()
            .map(|(position, entry)| (entry.catalog_id().to_owned(), position))
            .collect();
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
            positions: HashMap::new(),
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
        list.retain(|entry| entry.id != "a");
        assert!(!list.contains("a") && list.contains("c") && list.contains("e"));
    }
}
