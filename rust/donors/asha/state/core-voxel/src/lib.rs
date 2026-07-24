//! Neutral voxel value / material classification model.
//!
//! # Lane
//!
//! `rust-state` — `std`-only, zero external dependencies. Owns *what a voxel is*
//! as an abstract value, independent of any grid scale, chunk size, storage
//! strategy, or renderer (voxel-capability-02). Consumed by chunk storage,
//! meshing/culling, collision, replay/snapshot hashing, and edit commands.
//!
//! # Design soul: infrastructure-first, not a block game
//!
//! This is **not** a Minecraft-style block taxonomy. There are no product-domain
//! materials (dirt/stone/tree); materials are opaque [`VoxelMaterialId`]s validated
//! against a [`MaterialCatalog`]. The model assumes **large, mostly-static
//! volumes** rather than frequent per-cell edits, so the representation is small
//! and `Copy` and carries no per-cell metadata or churn machinery.
//!
//! # Settled decisions (voxel-capability-02 §"Decisions to make")
//!
//! 1. **`Empty` is a distinct variant**, not material id 0 — a material id and
//!    "no voxel here" are different kinds of thing, and material 0 stays usable.
//! 2. **Transparency is deferred**: every `Solid` is opaque today ([`VoxelValue::is_opaque`]),
//!    with room for a `VoxelOpacity` axis later without changing storage.
//! 3. **Non-cubic shapes deferred**: values classify occupancy, not geometry.
//! 4. **Voxels hold no metadata**: metadata-bearing things are separate entities,
//!    keeping the per-cell value tiny for big volumes.
//! 5. Materials are **Rust-validated** via [`MaterialCatalog`]; a TS catalog may
//!    later author the set, but acceptance stays Rust-side.
//! 6. Unknown materials are a **validation rejection** ([`MaterialError`]), not a
//!    silent fallback — callers decide recovery.
//!
//! Deferred expansion hooks (intentionally not implemented): `VoxelFlags`,
//! `VoxelOpacity`, `VoxelCollisionKind`, `VoxelRenderKind`, `VoxelGridKind`.

#![forbid(unsafe_code)]

/// An opaque reference to a material. Carries no product meaning here; a
/// [`MaterialCatalog`] (Rust-owned, possibly TS-authored later) decides which ids
/// are valid. `u16` keeps the per-voxel value small for large volumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VoxelMaterialId(u16);

impl VoxelMaterialId {
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u16 {
        self.0
    }
}

/// The value of a single voxel cell: either empty space or a solid of some
/// material. Small, `Copy`, with stable equality/ordering/hash and a stable
/// encoding for replay/snapshot artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum VoxelValue {
    /// No voxel here (reserved "air"). The default.
    #[default]
    Empty,
    /// A filled cell of the given material.
    Solid { material: VoxelMaterialId },
}

impl VoxelValue {
    /// The empty value (reserved air).
    pub const EMPTY: VoxelValue = VoxelValue::Empty;

    /// A solid voxel of `material`.
    pub const fn solid(material: VoxelMaterialId) -> Self {
        VoxelValue::Solid { material }
    }

    /// A solid voxel from a raw material id.
    pub const fn solid_raw(material: u16) -> Self {
        VoxelValue::Solid {
            material: VoxelMaterialId::new(material),
        }
    }

    pub const fn is_empty(self) -> bool {
        matches!(self, VoxelValue::Empty)
    }

    pub const fn is_solid(self) -> bool {
        matches!(self, VoxelValue::Solid { .. })
    }

    /// The material, if solid.
    pub const fn material(self) -> Option<VoxelMaterialId> {
        match self {
            VoxelValue::Empty => None,
            VoxelValue::Solid { material } => Some(material),
        }
    }

    /// Whether this cell blocks sight for face-culling purposes.
    ///
    /// Today: solids are opaque, empty is not. Transparency is deferred (decision
    /// 2); when a `VoxelOpacity` axis lands this becomes material-driven without
    /// touching storage.
    pub const fn is_opaque(self) -> bool {
        self.is_solid()
    }

    /// Whether this cell participates in collision by default.
    ///
    /// Today: solids collide, empty does not. Per-material collision kinds are
    /// deferred (decision in voxel-capability-11).
    pub const fn is_collidable(self) -> bool {
        self.is_solid()
    }

    // ── Stable encoding (for replay/snapshot/chunk hashing) ────────────────────
    //
    // Layout: bit 16 is the "solid" tag; the low 16 bits carry the material id.
    //   Empty        -> 0x0000_0000
    //   Solid(m)     -> 0x0001_0000 | m
    // `Empty == 0` makes a zeroed buffer read as all-empty (the common big-volume
    // case). The scheme is fixed; changing it is a replay-format change.

    const SOLID_TAG: u32 = 0x0001_0000;

    /// Encode to a stable `u32` for deterministic artifacts.
    pub const fn to_encoded(self) -> u32 {
        match self {
            VoxelValue::Empty => 0,
            VoxelValue::Solid { material } => Self::SOLID_TAG | material.raw() as u32,
        }
    }

    /// Decode from [`to_encoded`](Self::to_encoded). Returns `None` for any
    /// bit pattern this version did not emit (forward-compat tripwire).
    pub const fn from_encoded(bits: u32) -> Option<Self> {
        if bits == 0 {
            Some(VoxelValue::Empty)
        } else if bits & Self::SOLID_TAG != 0 && bits & !(Self::SOLID_TAG | 0xFFFF) == 0 {
            Some(VoxelValue::Solid {
                material: VoxelMaterialId::new((bits & 0xFFFF) as u16),
            })
        } else {
            None
        }
    }
}

/// A failure validating a voxel value against a [`MaterialCatalog`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialError {
    /// The value referenced a material id the catalog does not contain.
    UnknownMaterial(VoxelMaterialId),
}

impl core::fmt::Display for MaterialError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MaterialError::UnknownMaterial(id) => {
                write!(f, "unknown voxel material id {}", id.raw())
            }
        }
    }
}

impl std::error::Error for MaterialError {}

/// The set of material ids accepted by the authority. Rust-owned; a TS catalog may
/// later author the membership, but validation stays here. Deliberately just a
/// membership check — it holds no product material definitions.
#[derive(Debug, Clone, Default)]
pub struct MaterialCatalog {
    /// Sorted, deduplicated for deterministic iteration and binary search.
    ids: Vec<VoxelMaterialId>,
}

impl MaterialCatalog {
    /// Build a catalog from an id iterator (deduplicated, sorted).
    pub fn new(ids: impl IntoIterator<Item = VoxelMaterialId>) -> Self {
        let mut ids: Vec<_> = ids.into_iter().collect();
        ids.sort_unstable();
        ids.dedup();
        Self { ids }
    }

    /// Whether `id` is a registered material.
    pub fn contains(&self, id: VoxelMaterialId) -> bool {
        self.ids.binary_search(&id).is_ok()
    }

    /// Number of registered materials.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Registered ids in ascending order (deterministic).
    pub fn ids(&self) -> impl Iterator<Item = VoxelMaterialId> + '_ {
        self.ids.iter().copied()
    }

    /// Validate a voxel value. `Empty` is always valid; a `Solid` must reference a
    /// registered material, else [`MaterialError::UnknownMaterial`].
    pub fn validate(&self, value: VoxelValue) -> Result<(), MaterialError> {
        match value.material() {
            None => Ok(()),
            Some(id) if self.contains(id) => Ok(()),
            Some(id) => Err(MaterialError::UnknownMaterial(id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_vs_solid_classification() {
        assert!(VoxelValue::EMPTY.is_empty());
        assert!(!VoxelValue::EMPTY.is_solid());
        assert_eq!(VoxelValue::EMPTY.material(), None);
        assert!(!VoxelValue::EMPTY.is_opaque());
        assert!(!VoxelValue::EMPTY.is_collidable());

        let s = VoxelValue::solid_raw(7);
        assert!(s.is_solid());
        assert!(!s.is_empty());
        assert_eq!(s.material(), Some(VoxelMaterialId::new(7)));
        assert!(s.is_opaque());
        assert!(s.is_collidable());
        assert_eq!(VoxelValue::default(), VoxelValue::Empty);
    }

    #[test]
    fn empty_is_distinct_from_material_zero() {
        assert_ne!(VoxelValue::EMPTY, VoxelValue::solid_raw(0));
        assert_eq!(
            VoxelValue::solid_raw(0).material(),
            Some(VoxelMaterialId::new(0))
        );
    }

    #[test]
    fn material_id_equality_order_and_hash() {
        use std::collections::HashSet;
        assert_eq!(VoxelMaterialId::new(3), VoxelMaterialId::new(3));
        assert!(VoxelMaterialId::new(1) < VoxelMaterialId::new(2));
        let set: HashSet<_> = [VoxelMaterialId::new(1), VoxelMaterialId::new(1)]
            .into_iter()
            .collect();
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn encoding_roundtrips_and_is_stable() {
        // Stable golden encodings — changing these is a replay-format change.
        assert_eq!(VoxelValue::EMPTY.to_encoded(), 0x0000_0000);
        assert_eq!(VoxelValue::solid_raw(0).to_encoded(), 0x0001_0000);
        assert_eq!(VoxelValue::solid_raw(7).to_encoded(), 0x0001_0007);
        assert_eq!(VoxelValue::solid_raw(0xFFFF).to_encoded(), 0x0001_FFFF);

        for v in [
            VoxelValue::EMPTY,
            VoxelValue::solid_raw(0),
            VoxelValue::solid_raw(42),
            VoxelValue::solid_raw(u16::MAX),
        ] {
            assert_eq!(VoxelValue::from_encoded(v.to_encoded()), Some(v));
        }
    }

    #[test]
    fn decoding_rejects_unknown_bit_patterns() {
        // Garbage high bits are rejected rather than silently misread.
        assert_eq!(VoxelValue::from_encoded(0x0000_0001), None);
        assert_eq!(VoxelValue::from_encoded(0x0002_0000), None);
        assert_eq!(VoxelValue::from_encoded(0xFFFF_FFFF), None);
    }

    #[test]
    fn catalog_validates_membership_and_rejects_unknown() {
        let catalog = MaterialCatalog::new([
            VoxelMaterialId::new(5),
            VoxelMaterialId::new(1),
            VoxelMaterialId::new(5),
        ]);
        assert_eq!(catalog.len(), 2); // deduped
        assert_eq!(
            catalog.ids().map(|m| m.raw()).collect::<Vec<_>>(),
            vec![1, 5]
        ); // sorted

        assert_eq!(catalog.validate(VoxelValue::EMPTY), Ok(()));
        assert_eq!(catalog.validate(VoxelValue::solid_raw(5)), Ok(()));
        assert_eq!(
            catalog.validate(VoxelValue::solid_raw(9)),
            Err(MaterialError::UnknownMaterial(VoxelMaterialId::new(9))),
        );
    }

    #[test]
    fn classification_is_independent_of_any_grid() {
        // The value model references no grid/chunk type at all; this compiles and
        // holds regardless of voxel size or chunk strategy.
        let v = VoxelValue::solid_raw(3);
        assert_eq!(v.is_opaque(), v.is_solid());
    }
}
