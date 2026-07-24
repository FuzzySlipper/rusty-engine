//! Typed entity identity shared by Rusty Engine's object-centric runtime.
//!
//! This crate is a narrowed fork of Asha Engine's `core-ids`. The donor's
//! abstract subject/process/mode/signal IDs and its project/session/prefab IDs
//! had no consumer in Rusty Engine, so only the established entity identity is
//! retained. It is `std`-only and has no external dependencies.

#![forbid(unsafe_code)]

/// Identifies one discrete gameplay entity.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(u64);

impl EntityId {
    /// Construct an entity ID from its stable raw value.
    #[inline]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the stable raw value.
    #[inline]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl std::fmt::Debug for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EntityId({})", self.0)
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EntityId({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn construction_and_raw_roundtrip() {
        assert_eq!(EntityId::new(42).raw(), 42);
        assert_eq!(EntityId::new(u64::MAX).raw(), u64::MAX);
    }

    #[test]
    fn equality_ordering_and_hash_are_value_based() {
        let one = EntityId::new(1);
        let another_one = EntityId::new(1);
        let two = EntityId::new(2);
        assert_eq!(one, another_one);
        assert!(one < two);

        let set = HashSet::from([one, another_one, two]);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn debug_and_display_are_stable() {
        let entity = EntityId::new(7);
        assert_eq!(format!("{entity:?}"), "EntityId(7)");
        assert_eq!(format!("{entity}"), "EntityId(7)");
    }
}
