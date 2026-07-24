//! Typed asset-reference vocabulary for the ASHA scene/world foundation.
//!
//! # Lane
//!
//! `rust-foundation` — no knowledge of state, protocol, render, or TS packages.
//!
//! # Allowed dependencies
//!
//! None. Like [`core-ids`](../core_ids/index.html), this crate is `std`-only
//! with zero external dependencies so every layer above it can validate asset
//! references without pulling transitive baggage.
//!
//! # Scope (subtask #2314)
//!
//! This is the **vocabulary / validation seam** the scene/world document model
//! (`scene-capability-01`) and the asset registry (`scene-capability-03`) agreed
//! to co-design *before* their implementations diverge. It provides:
//!
//! * [`AssetKind`] — the closed set of asset kinds the scene docs need.
//! * [`AssetId`] — a stable, kind-prefixed scoped-kebab-case identifier with
//!   parse/validation that structurally rejects spaces, freeform strings,
//!   unknown kinds, and bad segments.
//! * [`AssetRef`] — a *typed* reference whose phantom kind lets the compiler
//!   reject wrong-kind references, plus [`AssetReference`] for heterogeneous
//!   dependency lists.
//! * [`AssetVersionReq`] and [`AssetHash`] — minimal version-constraint and
//!   content-hash vocabulary so a reference can be pinned/locked later.
//!
//! # Not in scope here (remains for the full asset registry, task #2311)
//!
//! Catalog manifests, asset **resolution** against a catalog, the asset
//! dependency DAG + cycle detection, asset-lock generation, per-kind/per-context
//! fallback policy, the material authority/style projection split, and
//! hot-reload/update-one-asset APIs all belong to the full asset-registry work.
//! This crate deliberately owns *identity and reference shape only*: it never
//! reads a catalog, allocates IDs, or decides whether a referenced asset exists.
//!
//! Richer version semantics (semver ranges) and a committed hash algorithm are
//! also deferred; [`AssetVersionReq`] and [`AssetHash`] are intentionally small.
//!
//! # TS border
//!
//! Asset references only cross the Rust/TS border once they are embedded in the
//! scene-document and catalog shapes that TS authors. Those generated contracts
//! land with the flat scene document (subtask #2315) and the catalog registry
//! (task #2311); this crate adds no `protocol-*` / codegen surface on its own.

#![forbid(unsafe_code)]

use std::fmt;
use std::marker::PhantomData;

// ── Asset kinds ───────────────────────────────────────────────────────────────

/// The closed set of asset kinds the scene/world documents reference.
///
/// Each kind has a canonical kebab-case **prefix** that begins every [`AssetId`]
/// of that kind (e.g. `material/concrete-wet`). The prefix is part of durable
/// identity, which is what lets Rust structurally reject a wrong-kind reference
/// without consulting a catalog.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum AssetKind {
    /// Surface material (authority flags + visual style; split downstream).
    Material,
    /// Authored / static (non-voxel) mesh asset.
    StaticMesh,
    /// A single non-UI sprite / billboard.
    Sprite,
    /// A sprite sheet / atlas of frames.
    SpriteSheet,
    /// A standalone texture image.
    Texture,
    /// A decoded-at-realization audio clip (for example WAV or OGG content).
    AudioClip,
    /// A font resource loaded by a presentation host (for example WOFF2).
    Font,
    /// An authored voxel volume (grid of cells).
    VoxelVolume,
    /// A reusable voxel object instanced into scenes.
    VoxelObject,
    /// A script / policy reference.
    Script,
    /// A composable scene document reference.
    Scene,
}

impl AssetKind {
    /// Every kind, in a stable order, for iteration and exhaustive fixtures.
    pub const ALL: &'static [AssetKind] = &[
        AssetKind::Material,
        AssetKind::StaticMesh,
        AssetKind::Sprite,
        AssetKind::SpriteSheet,
        AssetKind::Texture,
        AssetKind::AudioClip,
        AssetKind::Font,
        AssetKind::VoxelVolume,
        AssetKind::VoxelObject,
        AssetKind::Script,
        AssetKind::Scene,
    ];

    /// The canonical kebab-case prefix that begins every ID of this kind.
    #[inline]
    pub const fn prefix(self) -> &'static str {
        match self {
            AssetKind::Material => "material",
            AssetKind::StaticMesh => "mesh",
            AssetKind::Sprite => "sprite",
            AssetKind::SpriteSheet => "sprite-sheet",
            AssetKind::Texture => "texture",
            AssetKind::AudioClip => "audio",
            AssetKind::Font => "font",
            AssetKind::VoxelVolume => "voxel-volume",
            AssetKind::VoxelObject => "voxel-object",
            AssetKind::Script => "script",
            AssetKind::Scene => "scene",
        }
    }

    /// Resolve a kind from its canonical prefix, or `None` if unknown.
    pub fn from_prefix(prefix: &str) -> Option<AssetKind> {
        AssetKind::ALL
            .iter()
            .copied()
            .find(|k| k.prefix() == prefix)
    }
}

impl fmt::Display for AssetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.prefix())
    }
}

// ── Asset identifiers ─────────────────────────────────────────────────────────

/// A stable, kind-prefixed, scoped-kebab-case asset identifier.
///
/// The canonical form is `<kind-prefix>/<name>` where `<name>` is one or more
/// kebab-case segments separated by `/` (the extra `/`s allow scoping, e.g.
/// `material/factory/concrete-wet`). Each segment is `[a-z0-9]+` groups joined by
/// single hyphens. IDs are stable across project moves; human-readable display
/// names and source paths are metadata held elsewhere, never identity.
///
/// Construct via [`AssetId::parse`]; the constructor is the only way to obtain a
/// value, so an `AssetId` in hand is always well-formed and carries a known
/// [`AssetKind`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId {
    /// Canonical, validated string (e.g. `material/concrete-wet`).
    text: String,
    /// Kind parsed from the leading prefix.
    kind: AssetKind,
    /// Byte offset of the name (just past the first `/`).
    name_start: usize,
}

impl AssetId {
    /// Parse and validate a canonical asset ID string.
    ///
    /// Rejects empty input, any whitespace, a missing `kind/` separator, an
    /// unknown kind prefix, an empty name, and any non-kebab segment (uppercase,
    /// underscores, leading/trailing/double hyphens, or other punctuation).
    pub fn parse(text: &str) -> Result<AssetId, AssetIdError> {
        if text.is_empty() {
            return Err(AssetIdError::Empty);
        }
        if text.chars().any(char::is_whitespace) {
            return Err(AssetIdError::ContainsWhitespace);
        }
        let sep = text.find('/').ok_or(AssetIdError::MissingKindSeparator)?;
        let prefix = &text[..sep];
        let kind = AssetKind::from_prefix(prefix).ok_or_else(|| AssetIdError::UnknownKind {
            prefix: prefix.to_string(),
        })?;

        let name_start = sep + 1;
        let name = &text[name_start..];
        if name.is_empty() {
            return Err(AssetIdError::EmptyName);
        }
        for segment in name.split('/') {
            if !is_kebab_segment(segment) {
                return Err(AssetIdError::InvalidSegment {
                    segment: segment.to_string(),
                });
            }
        }

        Ok(AssetId {
            text: text.to_string(),
            kind,
            name_start,
        })
    }

    /// The asset kind, derived from the leading prefix.
    #[inline]
    pub fn kind(&self) -> AssetKind {
        self.kind
    }

    /// The full canonical ID string (e.g. `material/concrete-wet`).
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The name portion after the kind prefix (e.g. `concrete-wet`).
    #[inline]
    pub fn name(&self) -> &str {
        &self.text[self.name_start..]
    }
}

/// `true` if `s` is a single kebab-case segment: one or more `[a-z0-9]+` groups
/// joined by single hyphens, with no leading/trailing/double hyphens.
fn is_kebab_segment(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut prev_hyphen = true; // disallow leading hyphen
    for c in s.chars() {
        match c {
            'a'..='z' | '0'..='9' => prev_hyphen = false,
            '-' => {
                if prev_hyphen {
                    return false; // leading or double hyphen
                }
                prev_hyphen = true;
            }
            _ => return false,
        }
    }
    !prev_hyphen // disallow trailing hyphen
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl fmt::Debug for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AssetId({})", self.text)
    }
}

/// Why an [`AssetId`] string failed validation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AssetIdError {
    /// The input was the empty string.
    Empty,
    /// The input contained whitespace (spaces, tabs, newlines).
    ContainsWhitespace,
    /// No `/` separating the kind prefix from the name.
    MissingKindSeparator,
    /// The leading prefix is not a known [`AssetKind`].
    UnknownKind {
        /// The unrecognized prefix.
        prefix: String,
    },
    /// The name after the prefix was empty.
    EmptyName,
    /// A `/`-delimited name segment was not valid kebab-case.
    InvalidSegment {
        /// The offending segment.
        segment: String,
    },
}

impl fmt::Display for AssetIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetIdError::Empty => f.write_str("asset id is empty"),
            AssetIdError::ContainsWhitespace => f.write_str("asset id contains whitespace"),
            AssetIdError::MissingKindSeparator => {
                f.write_str("asset id is missing a `kind/name` separator")
            }
            AssetIdError::UnknownKind { prefix } => {
                write!(f, "asset id has unknown kind prefix `{prefix}`")
            }
            AssetIdError::EmptyName => f.write_str("asset id has an empty name"),
            AssetIdError::InvalidSegment { segment } => {
                write!(f, "asset id segment `{segment}` is not scoped kebab-case")
            }
        }
    }
}

impl std::error::Error for AssetIdError {}

// ── Version constraints and content hashes ────────────────────────────────────

/// A minimal version constraint carried by an asset reference.
///
/// Intentionally small for the vocabulary seam — richer semver ranges are
/// deferred to the asset-registry work (task #2311). The default is [`Any`].
///
/// [`Any`]: AssetVersionReq::Any
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AssetVersionReq {
    /// Accept any catalog version (the unpinned default).
    #[default]
    Any,
    /// Require exactly this version.
    Exact(u32),
    /// Require this version or newer.
    AtLeast(u32),
}

/// A content hash pinning an asset reference, kept algorithm-agnostic.
///
/// Stored as a validated lowercase-hex string of even, non-zero length. The seam
/// does not commit to a digest algorithm; the registry decides that later.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetHash(String);

impl AssetHash {
    /// Parse a lowercase-hex hash string. Rejects empty, odd-length, or
    /// non-`[0-9a-f]` input.
    pub fn parse(hex: &str) -> Result<AssetHash, AssetHashError> {
        if hex.is_empty() {
            return Err(AssetHashError::Empty);
        }
        if !hex.len().is_multiple_of(2) {
            return Err(AssetHashError::OddLength);
        }
        if !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(AssetHashError::NotLowercaseHex);
        }
        Ok(AssetHash(hex.to_string()))
    }

    /// The hex string.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for AssetHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AssetHash({})", self.0)
    }
}

/// Why an [`AssetHash`] string failed validation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AssetHashError {
    /// The input was empty.
    Empty,
    /// The hex string had an odd number of digits.
    OddLength,
    /// The string contained a non-lowercase-hex character.
    NotLowercaseHex,
}

impl fmt::Display for AssetHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetHashError::Empty => f.write_str("asset hash is empty"),
            AssetHashError::OddLength => f.write_str("asset hash has an odd number of hex digits"),
            AssetHashError::NotLowercaseHex => {
                f.write_str("asset hash is not lowercase hex (`0-9a-f`)")
            }
        }
    }
}

impl std::error::Error for AssetHashError {}

// ── References ────────────────────────────────────────────────────────────────

/// A kind-erased asset reference for heterogeneous dependency lists.
///
/// Use this where a collection mixes kinds (e.g. a scene document's
/// `dependencies`). The kind is always recoverable via [`AssetReference::kind`].
/// For a single statically-known kind, prefer the compile-time-checked
/// [`AssetRef`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AssetReference {
    id: AssetId,
    version: AssetVersionReq,
    hash: Option<AssetHash>,
}

impl AssetReference {
    /// Build a reference from an already-validated [`AssetId`].
    pub fn new(id: AssetId, version: AssetVersionReq, hash: Option<AssetHash>) -> AssetReference {
        AssetReference { id, version, hash }
    }

    /// The referenced asset's ID.
    #[inline]
    pub fn id(&self) -> &AssetId {
        &self.id
    }

    /// The referenced asset's kind (from its ID prefix).
    #[inline]
    pub fn kind(&self) -> AssetKind {
        self.id.kind()
    }

    /// The version constraint.
    #[inline]
    pub fn version(&self) -> AssetVersionReq {
        self.version
    }

    /// The optional pinned content hash.
    #[inline]
    pub fn hash(&self) -> Option<&AssetHash> {
        self.hash.as_ref()
    }
}

/// Compile-time marker tying a zero-sized type to one [`AssetKind`].
///
/// Implemented by the zero-sized types in [`markers`]; it lets [`AssetRef<K>`]
/// reject a wrong-kind [`AssetId`] at construction and makes
/// `AssetRef<markers::Material>` a distinct type from `AssetRef<markers::Scene>`.
pub trait AssetKindMarker {
    /// The asset kind this marker represents.
    const KIND: AssetKind;
}

/// A reference to an asset of a statically-known kind `K`.
///
/// `AssetRef<markers::Material>` cannot be built from a `mesh/...` ID, and cannot
/// be passed where `AssetRef<markers::StaticMesh>` is expected — wrong-kind
/// references are a compile or construction error rather than a runtime surprise.
#[derive(Clone, PartialEq, Eq)]
pub struct AssetRef<K: AssetKindMarker> {
    id: AssetId,
    version: AssetVersionReq,
    hash: Option<AssetHash>,
    _kind: PhantomData<fn() -> K>,
}

impl<K: AssetKindMarker> AssetRef<K> {
    /// Build a typed reference, checking the ID's kind matches `K`.
    pub fn new(
        id: AssetId,
        version: AssetVersionReq,
        hash: Option<AssetHash>,
    ) -> Result<AssetRef<K>, AssetRefError> {
        if id.kind() != K::KIND {
            return Err(AssetRefError::KindMismatch {
                expected: K::KIND,
                actual: id.kind(),
            });
        }
        Ok(AssetRef {
            id,
            version,
            hash,
            _kind: PhantomData,
        })
    }

    /// Parse an ID string and build a typed reference in one step.
    pub fn parse(
        id: &str,
        version: AssetVersionReq,
        hash: Option<AssetHash>,
    ) -> Result<AssetRef<K>, AssetRefError> {
        let id = AssetId::parse(id).map_err(AssetRefError::Id)?;
        AssetRef::new(id, version, hash)
    }

    /// The referenced asset's ID.
    #[inline]
    pub fn id(&self) -> &AssetId {
        &self.id
    }

    /// The static kind of this reference.
    #[inline]
    pub fn kind(&self) -> AssetKind {
        K::KIND
    }

    /// The version constraint.
    #[inline]
    pub fn version(&self) -> AssetVersionReq {
        self.version
    }

    /// The optional pinned content hash.
    #[inline]
    pub fn hash(&self) -> Option<&AssetHash> {
        self.hash.as_ref()
    }

    /// Erase the static kind into a [`AssetReference`] for heterogeneous lists.
    pub fn erase(self) -> AssetReference {
        AssetReference {
            id: self.id,
            version: self.version,
            hash: self.hash,
        }
    }
}

impl<K: AssetKindMarker> fmt::Debug for AssetRef<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AssetRef")
            .field("id", &self.id)
            .field("version", &self.version)
            .field("hash", &self.hash)
            .finish()
    }
}

/// Why a typed [`AssetRef`] could not be built.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AssetRefError {
    /// The ID string itself was invalid.
    Id(AssetIdError),
    /// The ID's kind did not match the reference's static kind `K`.
    KindMismatch {
        /// The kind required by the typed reference.
        expected: AssetKind,
        /// The kind the ID actually carries.
        actual: AssetKind,
    },
}

impl fmt::Display for AssetRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetRefError::Id(e) => write!(f, "{e}"),
            AssetRefError::KindMismatch { expected, actual } => write!(
                f,
                "asset ref expected kind `{expected}` but id is `{actual}`"
            ),
        }
    }
}

impl std::error::Error for AssetRefError {}

/// Zero-sized kind markers for [`AssetRef`].
///
/// Kept in a submodule so the marker `markers::Material` never collides with the
/// [`AssetKind::Material`] enum variant.
pub mod markers {
    use super::{AssetKind, AssetKindMarker};

    /// Declares a zero-sized marker type bound to one [`AssetKind`].
    macro_rules! marker {
        ($(#[$attr:meta])* $name:ident => $kind:ident) => {
            $(#[$attr])*
            #[derive(Clone, Copy, PartialEq, Eq, Debug)]
            pub struct $name;

            impl AssetKindMarker for $name {
                const KIND: AssetKind = AssetKind::$kind;
            }
        };
    }

    marker!(/// Marker for material references. = no-op
        Material => Material);
    marker!(/// Marker for static-mesh references.
        StaticMesh => StaticMesh);
    marker!(/// Marker for sprite references.
        Sprite => Sprite);
    marker!(/// Marker for sprite-sheet references.
        SpriteSheet => SpriteSheet);
    marker!(/// Marker for texture references.
        Texture => Texture);
    marker!(/// Marker for voxel-volume references.
        VoxelVolume => VoxelVolume);
    marker!(/// Marker for voxel-object references.
        VoxelObject => VoxelObject);
    marker!(/// Marker for script/policy references.
        Script => Script);
    marker!(/// Marker for scene references.
        Scene => Scene);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_unique_prefix_that_round_trips() {
        let mut seen = std::collections::HashSet::new();
        for &kind in AssetKind::ALL {
            let prefix = kind.prefix();
            assert!(seen.insert(prefix), "duplicate prefix `{prefix}`");
            assert_eq!(AssetKind::from_prefix(prefix), Some(kind));
        }
        assert_eq!(AssetKind::from_prefix("nope"), None);
    }

    #[test]
    fn parses_valid_ids_for_each_kind() {
        let cases = [
            ("material/concrete-wet", AssetKind::Material, "concrete-wet"),
            (
                "mesh/factory-belt-straight",
                AssetKind::StaticMesh,
                "factory-belt-straight",
            ),
            (
                "sprite/person-hard-hat",
                AssetKind::Sprite,
                "person-hard-hat",
            ),
            (
                "sprite-sheet/worker-frames",
                AssetKind::SpriteSheet,
                "worker-frames",
            ),
            ("texture/worker-atlas", AssetKind::Texture, "worker-atlas"),
            (
                "voxel-volume/wall-section-a",
                AssetKind::VoxelVolume,
                "wall-section-a",
            ),
            ("voxel-object/crate-1", AssetKind::VoxelObject, "crate-1"),
            ("script/idle-policy", AssetKind::Script, "idle-policy"),
            (
                "scene/district-template-a",
                AssetKind::Scene,
                "district-template-a",
            ),
        ];
        for (text, kind, name) in cases {
            let id = AssetId::parse(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(id.kind(), kind);
            assert_eq!(id.as_str(), text);
            assert_eq!(id.name(), name);
        }
    }

    #[test]
    fn accepts_scoped_multi_segment_names() {
        let id = AssetId::parse("material/factory/concrete-wet").unwrap();
        assert_eq!(id.kind(), AssetKind::Material);
        assert_eq!(id.name(), "factory/concrete-wet");
    }

    #[test]
    fn rejects_spaces_and_freeform() {
        // Spaces anywhere.
        assert_eq!(
            AssetId::parse("material/concrete wet"),
            Err(AssetIdError::ContainsWhitespace)
        );
        assert_eq!(
            AssetId::parse(" material/concrete-wet"),
            Err(AssetIdError::ContainsWhitespace)
        );
        // Freeform string with no kind separator.
        assert_eq!(
            AssetId::parse("concrete-wet"),
            Err(AssetIdError::MissingKindSeparator)
        );
        // Empty.
        assert_eq!(AssetId::parse(""), Err(AssetIdError::Empty));
    }

    #[test]
    fn rejects_unknown_kind() {
        assert_eq!(
            AssetId::parse("widget/foo"),
            Err(AssetIdError::UnknownKind {
                prefix: "widget".to_string()
            })
        );
    }

    #[test]
    fn rejects_bad_segments() {
        // Empty name.
        assert_eq!(AssetId::parse("material/"), Err(AssetIdError::EmptyName));
        // Uppercase.
        assert!(matches!(
            AssetId::parse("material/Concrete"),
            Err(AssetIdError::InvalidSegment { .. })
        ));
        // Underscore.
        assert!(matches!(
            AssetId::parse("material/concrete_wet"),
            Err(AssetIdError::InvalidSegment { .. })
        ));
        // Leading / trailing / double hyphen.
        for bad in ["material/-wet", "material/wet-", "material/con--crete"] {
            assert!(
                matches!(
                    AssetId::parse(bad),
                    Err(AssetIdError::InvalidSegment { .. })
                ),
                "expected {bad} to be rejected"
            );
        }
        // Empty interior segment.
        assert!(matches!(
            AssetId::parse("material/factory//wet"),
            Err(AssetIdError::InvalidSegment { .. })
        ));
    }

    #[test]
    fn kebab_segment_predicate_edges() {
        assert!(is_kebab_segment("a"));
        assert!(is_kebab_segment("a1-b2"));
        assert!(!is_kebab_segment(""));
        assert!(!is_kebab_segment("-a"));
        assert!(!is_kebab_segment("a-"));
        assert!(!is_kebab_segment("a--b"));
        assert!(!is_kebab_segment("A"));
    }

    #[test]
    fn typed_ref_accepts_matching_kind() {
        let r = AssetRef::<markers::Material>::parse(
            "material/concrete-wet",
            AssetVersionReq::Exact(3),
            None,
        )
        .unwrap();
        assert_eq!(r.kind(), AssetKind::Material);
        assert_eq!(r.version(), AssetVersionReq::Exact(3));
        assert_eq!(r.id().as_str(), "material/concrete-wet");
    }

    #[test]
    fn typed_ref_rejects_mismatched_kind() {
        // A mesh id cannot become a Material ref.
        let err = AssetRef::<markers::Material>::parse(
            "mesh/factory-belt-straight",
            AssetVersionReq::Any,
            None,
        )
        .unwrap_err();
        assert_eq!(
            err,
            AssetRefError::KindMismatch {
                expected: AssetKind::Material,
                actual: AssetKind::StaticMesh,
            }
        );
    }

    #[test]
    fn typed_ref_surfaces_id_errors() {
        let err = AssetRef::<markers::Scene>::parse("scene/Bad Name", AssetVersionReq::Any, None)
            .unwrap_err();
        assert!(matches!(
            err,
            AssetRefError::Id(AssetIdError::ContainsWhitespace)
        ));
    }

    #[test]
    fn erase_preserves_id_and_kind() {
        let r = AssetRef::<markers::Scene>::parse(
            "scene/district-template-a",
            AssetVersionReq::AtLeast(2),
            None,
        )
        .unwrap();
        let erased = r.erase();
        assert_eq!(erased.kind(), AssetKind::Scene);
        assert_eq!(erased.id().as_str(), "scene/district-template-a");
        assert_eq!(erased.version(), AssetVersionReq::AtLeast(2));
    }

    #[test]
    fn version_req_defaults_to_any() {
        assert_eq!(AssetVersionReq::default(), AssetVersionReq::Any);
    }

    #[test]
    fn asset_hash_validation() {
        assert_eq!(AssetHash::parse("00ff").unwrap().as_str(), "00ff");
        assert_eq!(AssetHash::parse(""), Err(AssetHashError::Empty));
        assert_eq!(AssetHash::parse("abc"), Err(AssetHashError::OddLength));
        assert_eq!(
            AssetHash::parse("00FF"),
            Err(AssetHashError::NotLowercaseHex)
        );
        assert_eq!(AssetHash::parse("zz"), Err(AssetHashError::NotLowercaseHex));
    }

    #[test]
    fn heterogeneous_dependency_list_keeps_kinds() {
        let deps = [
            AssetRef::<markers::Material>::parse(
                "material/concrete-wet",
                AssetVersionReq::Any,
                None,
            )
            .unwrap()
            .erase(),
            AssetRef::<markers::StaticMesh>::parse("mesh/belt", AssetVersionReq::Any, None)
                .unwrap()
                .erase(),
            AssetReference::new(
                AssetId::parse("scene/sub-scene").unwrap(),
                AssetVersionReq::Any,
                AssetHash::parse("deadbeef").ok(),
            ),
        ];
        let kinds: Vec<AssetKind> = deps.iter().map(AssetReference::kind).collect();
        assert_eq!(
            kinds,
            [AssetKind::Material, AssetKind::StaticMesh, AssetKind::Scene]
        );
        assert_eq!(deps[2].hash().unwrap().as_str(), "deadbeef");
    }
}
