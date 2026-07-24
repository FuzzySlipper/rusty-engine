//! Typed stable identities shared by Rusty Engine's object-centric mechanisms.
//!
//! Every identity is a distinct `u64` newtype. Public codecs choose explicitly
//! which identities cross their boundary; there is no global protocol scanner or
//! generated ID registry.

#![forbid(unsafe_code)]

macro_rules! id_type {
    ($(#[$attribute:meta])* $name:ident) => {
        $(#[$attribute])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            #[inline]
            pub const fn new(raw: u64) -> Self {
                Self(raw)
            }

            #[inline]
            pub const fn raw(self) -> u64 {
                self.0
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}({})", stringify!($name), self.0)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

id_type!(/// One live or tombstoned gameplay entity.
    EntityId);
id_type!(/// An authority subject or policy identity.
    SubjectId);
id_type!(/// A named process or controller identity.
    ProcessId);
id_type!(/// A state-machine mode identity.
    ModeId);
id_type!(/// A typed signal identity.
    SignalId);
id_type!(/// An authority-owned entity classification label.
    TagId);
id_type!(/// A durable authored project identity.
    ProjectId);
id_type!(/// A durable authored scene identity.
    SceneId);
id_type!(/// A durable authored node within a scene.
    SceneNodeId);
id_type!(/// A durable prefab definition identity.
    PrefabId);
id_type!(/// A stable authored part within a prefab.
    PrefabPartId);
id_type!(/// One stored or admitted prefab instance.
    PrefabInstanceId);

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn construction_and_raw_roundtrip() {
        assert_eq!(EntityId::new(42).raw(), 42);
        assert_eq!(SubjectId::new(1).raw(), 1);
        assert_eq!(ProcessId::new(2).raw(), 2);
        assert_eq!(ModeId::new(3).raw(), 3);
        assert_eq!(SignalId::new(4).raw(), 4);
        assert_eq!(TagId::new(5).raw(), 5);
        assert_eq!(ProjectId::new(6).raw(), 6);
        assert_eq!(SceneId::new(7).raw(), 7);
        assert_eq!(SceneNodeId::new(8).raw(), 8);
        assert_eq!(PrefabId::new(9).raw(), 9);
        assert_eq!(PrefabPartId::new(10).raw(), 10);
        assert_eq!(PrefabInstanceId::new(u64::MAX).raw(), u64::MAX);
    }

    #[test]
    fn equality_ordering_and_hash_are_value_based() {
        let one = EntityId::new(1);
        let another_one = EntityId::new(1);
        let two = EntityId::new(2);
        assert_eq!(one, another_one);
        assert!(one < two);
        assert_eq!(HashSet::from([one, another_one, two]).len(), 2);
    }

    #[test]
    fn debug_and_display_expose_stable_brands() {
        assert_eq!(format!("{:?}", EntityId::new(7)), "EntityId(7)");
        assert_eq!(format!("{}", SceneNodeId::new(7)), "SceneNodeId(7)");
        assert_eq!(format!("{:?}", TagId::new(7)), "TagId(7)");
    }
}
