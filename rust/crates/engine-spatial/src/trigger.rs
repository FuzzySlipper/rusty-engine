use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use entity_state::EntityState;
use serde::{Deserialize, Serialize};

use crate::trigger_geometry::live_aabb;

pub const TRIGGER_VOLUME_SNAPSHOT_SCHEMA_VERSION: u32 = 2;
pub const MAX_TRIGGER_DEFINITIONS: usize = 4_096;
pub const MAX_ACTIVE_TRIGGER_OVERLAPS: usize = 1_000_000;
const MAX_TRIGGER_READ_ITEMS: usize = 100_000;

/// Selects where a registered trigger derives its live AABB during reconciliation.
///
/// `ActiveCollision` is the historical behavior: the trigger entity must be
/// active and expose an enabled collision component in addition to bounds and
/// a composed world transform. `EntityBounds` derives the same AABB from the
/// canonical entity lifecycle, bounds, and composed world transform without
/// consulting the collision component at all, so the trigger entity never has
/// to become a solid motion obstacle to sense subjects. Subject eligibility is
/// unaffected: subjects always require active collision regardless of the
/// trigger's geometry source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TriggerGeometrySource {
    #[default]
    ActiveCollision,
    EntityBounds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KinematicTriggerDefinition {
    pub trigger: u64,
    pub scope: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub geometry: TriggerGeometrySource,
}

impl KinematicTriggerDefinition {
    pub fn new(
        trigger: EntityId,
        scope: impl Into<String>,
        tags: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut tags = tags.into_iter().map(Into::into).collect::<Vec<_>>();
        tags.sort();
        tags.dedup();
        Self {
            trigger: trigger.raw(),
            scope: scope.into(),
            tags,
            geometry: TriggerGeometrySource::ActiveCollision,
        }
    }

    /// Selects a non-default trigger geometry source. Existing definitions
    /// keep `ActiveCollision`; `EntityBounds` registers a trigger that senses
    /// from bounds and composed transform without requiring active collision.
    pub const fn with_geometry_source(mut self, geometry: TriggerGeometrySource) -> Self {
        self.geometry = geometry;
        self
    }

    pub const fn geometry_source(&self) -> TriggerGeometrySource {
        self.geometry
    }

    pub const fn trigger_id(&self) -> EntityId {
        EntityId::new(self.trigger)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TriggerOverlapPair {
    pub trigger: u64,
    pub subject: u64,
}

impl TriggerOverlapPair {
    pub const fn new(trigger: EntityId, subject: EntityId) -> Self {
        Self {
            trigger: trigger.raw(),
            subject: subject.raw(),
        }
    }

    pub const fn trigger_id(self) -> EntityId {
        EntityId::new(self.trigger)
    }

    pub const fn subject_id(self) -> EntityId {
        EntityId::new(self.subject)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TriggerOverlapFactKind {
    Exit,
    Enter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TriggerReconcileCause {
    Scheduled,
    Spawn,
    Movement,
    Teleport,
    ActivationChanged,
    LifecycleChanged,
    Restore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerOverlapFact {
    pub kind: TriggerOverlapFactKind,
    pub pair: TriggerOverlapPair,
    pub scope: String,
    pub tags: Vec<String>,
    pub tick: u64,
    pub cause: TriggerReconcileCause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TriggerVolumeDiagnosticCode {
    DuplicateDefinition,
    MissingDefinition,
    InvalidIdentifier,
    InvalidTag,
    StaleEntity,
    MissingCollision,
    InactiveCollision,
    MissingBounds,
    MissingTransform,
    SnapshotDecode,
    SnapshotVersion,
    SnapshotInvariant,
    QuotaExceeded,
    StaleRevision,
    DuplicateLifecycle,
    RevisionOverflow,
}

impl TriggerVolumeDiagnosticCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::DuplicateDefinition => "duplicate-trigger-definition",
            Self::MissingDefinition => "missing-trigger-definition",
            Self::InvalidIdentifier => "invalid-trigger-identifier",
            Self::InvalidTag => "invalid-trigger-tag",
            Self::StaleEntity => "stale-trigger-entity",
            Self::MissingCollision => "trigger-missing-collision",
            Self::InactiveCollision => "trigger-inactive-collision",
            Self::MissingBounds => "trigger-missing-bounds",
            Self::MissingTransform => "trigger-missing-transform",
            Self::SnapshotDecode => "trigger-snapshot-decode",
            Self::SnapshotVersion => "trigger-snapshot-version",
            Self::SnapshotInvariant => "trigger-snapshot-invariant",
            Self::QuotaExceeded => "trigger-quota-exceeded",
            Self::StaleRevision => "stale-trigger-revision",
            Self::DuplicateLifecycle => "duplicate-trigger-lifecycle",
            Self::RevisionOverflow => "trigger-revision-overflow",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerVolumeDiagnostic {
    pub code: TriggerVolumeDiagnosticCode,
    pub entity: Option<EntityId>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerVolumeError {
    pub diagnostics: Vec<TriggerVolumeDiagnostic>,
}

impl std::fmt::Display for TriggerVolumeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "trigger-volume operation rejected with {} diagnostic(s)",
            self.diagnostics.len()
        )
    }
}

impl std::error::Error for TriggerVolumeError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerOverlapReadout {
    pub trigger: EntityId,
    pub subjects: Vec<EntityId>,
    pub revision: u64,
}

/// A deterministic, bounded page of current subjects for one trigger definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerOverlapPage {
    pub trigger: EntityId,
    pub subjects: Vec<EntityId>,
    pub revision: u64,
    pub total: usize,
    pub next_cursor: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerReconcileReceipt {
    pub tick: u64,
    pub cause: TriggerReconcileCause,
    pub revision: u64,
    pub facts: Vec<TriggerOverlapFact>,
    pub continued: Vec<TriggerOverlapPair>,
    pub active_overlaps: Vec<TriggerOverlapPair>,
    pub diagnostics: Vec<TriggerVolumeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerLifecycleReceipt {
    pub trigger: EntityId,
    pub active: bool,
    pub revision_before: u64,
    pub revision_after: u64,
    pub removed_overlaps: Vec<TriggerOverlapPair>,
    pub facts: Vec<TriggerOverlapFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerRestoreReceipt {
    pub revision_before: u64,
    pub revision_after: u64,
    pub registered_count: usize,
    pub active_count: usize,
    pub active_overlaps: Vec<TriggerOverlapPair>,
    pub diagnostics: Vec<TriggerVolumeDiagnostic>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TriggerVolumeSystem {
    definitions: BTreeMap<EntityId, KinematicTriggerDefinition>,
    inactive_triggers: BTreeSet<EntityId>,
    active_overlaps: BTreeSet<TriggerOverlapPair>,
    revision: u64,
}

impl TriggerVolumeSystem {
    pub fn new(
        definitions: impl IntoIterator<Item = KinematicTriggerDefinition>,
    ) -> Result<Self, TriggerVolumeError> {
        let mut system = Self::default();
        for definition in definitions {
            system.register(definition)?;
        }
        Ok(system)
    }

    pub fn register(
        &mut self,
        mut definition: KinematicTriggerDefinition,
    ) -> Result<(), TriggerVolumeError> {
        let trigger = definition.trigger_id();
        let mut diagnostics = validate_definition(&definition);
        if self.definitions.contains_key(&trigger) {
            diagnostics.push(diagnostic(
                TriggerVolumeDiagnosticCode::DuplicateDefinition,
                Some(trigger),
                "trigger entity already has a registered definition",
            ));
        }
        if self.definitions.len() >= MAX_TRIGGER_DEFINITIONS {
            diagnostics.push(diagnostic(
                TriggerVolumeDiagnosticCode::QuotaExceeded,
                Some(trigger),
                format!("trigger definition limit is {MAX_TRIGGER_DEFINITIONS}"),
            ));
        }
        if !diagnostics.is_empty() {
            return Err(TriggerVolumeError { diagnostics });
        }
        definition.tags.sort();
        definition.tags.dedup();
        self.definitions.insert(trigger, definition);
        Ok(())
    }

    pub fn definitions(&self) -> impl Iterator<Item = &KinematicTriggerDefinition> {
        self.definitions.values()
    }

    pub fn active_overlaps(&self) -> impl Iterator<Item = TriggerOverlapPair> + '_ {
        self.active_overlaps.iter().copied()
    }

    pub fn is_active(&self, trigger: EntityId) -> Result<bool, TriggerVolumeError> {
        if !self.definitions.contains_key(&trigger) {
            return Err(missing_definition(trigger));
        }
        Ok(!self.inactive_triggers.contains(&trigger))
    }

    pub fn active_trigger_count(&self) -> usize {
        self.definitions.len() - self.inactive_triggers.len()
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn set_active(
        &mut self,
        trigger: EntityId,
        expected_revision: u64,
        active: bool,
        tick: u64,
    ) -> Result<TriggerLifecycleReceipt, TriggerVolumeError> {
        self.require_revision(expected_revision)?;
        let currently_active = self.is_active(trigger)?;
        if currently_active == active {
            return Err(TriggerVolumeError {
                diagnostics: vec![diagnostic(
                    TriggerVolumeDiagnosticCode::DuplicateLifecycle,
                    Some(trigger),
                    if active {
                        "trigger is already active"
                    } else {
                        "trigger is already inactive"
                    },
                )],
            });
        }
        let revision_before = self.revision;
        let revision_after = next_revision(revision_before)?;
        let removed_overlaps = if active {
            self.inactive_triggers.remove(&trigger);
            Vec::new()
        } else {
            self.inactive_triggers.insert(trigger);
            let removed = overlaps_for(&self.active_overlaps, trigger);
            for pair in &removed {
                self.active_overlaps.remove(pair);
            }
            removed
        };
        let definition = &self.definitions[&trigger];
        let facts = removed_overlaps
            .iter()
            .copied()
            .map(|pair| TriggerOverlapFact {
                kind: TriggerOverlapFactKind::Exit,
                pair,
                scope: definition.scope.clone(),
                tags: definition.tags.clone(),
                tick,
                cause: TriggerReconcileCause::LifecycleChanged,
            })
            .collect();
        self.revision = revision_after;
        Ok(TriggerLifecycleReceipt {
            trigger,
            active,
            revision_before,
            revision_after,
            removed_overlaps,
            facts,
        })
    }

    pub fn restore(
        &mut self,
        active_triggers: &[EntityId],
        entities: &EntityState,
        expected_revision: u64,
    ) -> Result<TriggerRestoreReceipt, TriggerVolumeError> {
        self.require_revision(expected_revision)?;
        if active_triggers.len() > MAX_TRIGGER_DEFINITIONS {
            return Err(TriggerVolumeError {
                diagnostics: vec![diagnostic(
                    TriggerVolumeDiagnosticCode::QuotaExceeded,
                    None,
                    format!("active trigger limit is {MAX_TRIGGER_DEFINITIONS}"),
                )],
            });
        }
        let active = active_triggers.iter().copied().collect::<BTreeSet<_>>();
        if active.len() != active_triggers.len() {
            return Err(TriggerVolumeError {
                diagnostics: vec![diagnostic(
                    TriggerVolumeDiagnosticCode::DuplicateLifecycle,
                    None,
                    "restore active trigger set contains a duplicate",
                )],
            });
        }
        if let Some(unknown) = active
            .iter()
            .find(|trigger| !self.definitions.contains_key(trigger))
        {
            return Err(missing_definition(*unknown));
        }

        let inactive_triggers = self
            .definitions
            .keys()
            .copied()
            .filter(|trigger| !active.contains(trigger))
            .collect::<BTreeSet<_>>();
        let mut candidate = self.clone();
        candidate.inactive_triggers = inactive_triggers;
        let (active_overlaps, diagnostics) = candidate.compute_overlaps(entities)?;
        let changed = candidate.inactive_triggers != self.inactive_triggers
            || active_overlaps != self.active_overlaps;
        let revision_before = self.revision;
        let revision_after = if changed {
            next_revision(revision_before)?
        } else {
            revision_before
        };
        candidate.revision = revision_after;
        candidate.active_overlaps = active_overlaps;
        *self = candidate;
        Ok(TriggerRestoreReceipt {
            revision_before,
            revision_after,
            registered_count: self.definitions.len(),
            active_count: self.active_trigger_count(),
            active_overlaps: self.active_overlaps().collect(),
            diagnostics,
        })
    }

    pub fn reconcile(
        &mut self,
        entities: &EntityState,
        tick: u64,
        cause: TriggerReconcileCause,
    ) -> Result<TriggerReconcileReceipt, TriggerVolumeError> {
        let (next, diagnostics) = self.compute_overlaps(entities)?;
        let exits = self
            .active_overlaps
            .difference(&next)
            .copied()
            .collect::<Vec<_>>();
        let enters = next
            .difference(&self.active_overlaps)
            .copied()
            .collect::<Vec<_>>();
        let continued = next
            .intersection(&self.active_overlaps)
            .copied()
            .collect::<Vec<_>>();
        let revision = if exits.is_empty() && enters.is_empty() {
            self.revision
        } else {
            next_revision(self.revision)?
        };
        let mut facts = Vec::with_capacity(exits.len() + enters.len());
        for (kind, pairs) in [
            (TriggerOverlapFactKind::Exit, exits),
            (TriggerOverlapFactKind::Enter, enters),
        ] {
            for pair in pairs {
                let definition = &self.definitions[&pair.trigger_id()];
                facts.push(TriggerOverlapFact {
                    kind,
                    pair,
                    scope: definition.scope.clone(),
                    tags: definition.tags.clone(),
                    tick,
                    cause,
                });
            }
        }
        self.revision = revision;
        self.active_overlaps = next;
        Ok(TriggerReconcileReceipt {
            tick,
            cause,
            revision,
            facts,
            continued,
            active_overlaps: self.active_overlaps().collect(),
            diagnostics,
        })
    }

    pub fn current_overlaps(
        &self,
        trigger: EntityId,
        max_items: usize,
    ) -> Result<TriggerOverlapReadout, TriggerVolumeError> {
        if !self.definitions.contains_key(&trigger) {
            return Err(TriggerVolumeError {
                diagnostics: vec![diagnostic(
                    TriggerVolumeDiagnosticCode::MissingDefinition,
                    Some(trigger),
                    "trigger definition is not registered",
                )],
            });
        }
        let subjects = self
            .active_overlaps
            .range(
                TriggerOverlapPair::new(trigger, EntityId::new(0))
                    ..=TriggerOverlapPair::new(trigger, EntityId::new(u64::MAX)),
            )
            .map(|pair| pair.subject_id())
            .collect::<Vec<_>>();
        let limit = max_items.min(MAX_TRIGGER_READ_ITEMS);
        if subjects.len() > limit {
            return Err(TriggerVolumeError {
                diagnostics: vec![diagnostic(
                    TriggerVolumeDiagnosticCode::QuotaExceeded,
                    Some(trigger),
                    format!("{} overlaps exceed read limit {limit}", subjects.len()),
                )],
            });
        }
        Ok(TriggerOverlapReadout {
            trigger,
            subjects,
            revision: self.revision,
        })
    }

    /// Reads one contiguous page without requiring callers to guess whether a quota hid facts.
    /// The revision fences continuations against trigger-set changes between pages.
    pub fn current_overlaps_page(
        &self,
        trigger: EntityId,
        expected_revision: Option<u64>,
        cursor: usize,
        page_size: usize,
    ) -> Result<TriggerOverlapPage, TriggerVolumeError> {
        if page_size == 0 || page_size > MAX_TRIGGER_READ_ITEMS {
            return Err(TriggerVolumeError {
                diagnostics: vec![diagnostic(
                    TriggerVolumeDiagnosticCode::QuotaExceeded,
                    Some(trigger),
                    format!(
                        "trigger page size {page_size} is outside 1..={MAX_TRIGGER_READ_ITEMS}"
                    ),
                )],
            });
        }
        if !self.definitions.contains_key(&trigger) {
            return Err(TriggerVolumeError {
                diagnostics: vec![diagnostic(
                    TriggerVolumeDiagnosticCode::MissingDefinition,
                    Some(trigger),
                    "trigger definition is not registered",
                )],
            });
        }
        if expected_revision.is_some_and(|value| value != self.revision) {
            return Err(TriggerVolumeError {
                diagnostics: vec![diagnostic(
                    TriggerVolumeDiagnosticCode::StaleRevision,
                    Some(trigger),
                    "trigger overlap continuation revision is stale",
                )],
            });
        }
        let subjects = self
            .active_overlaps
            .range(
                TriggerOverlapPair::new(trigger, EntityId::new(0))
                    ..=TriggerOverlapPair::new(trigger, EntityId::new(u64::MAX)),
            )
            .map(|pair| pair.subject_id())
            .collect::<Vec<_>>();
        if cursor > subjects.len() {
            return Err(TriggerVolumeError {
                diagnostics: vec![diagnostic(
                    TriggerVolumeDiagnosticCode::StaleRevision,
                    Some(trigger),
                    format!(
                        "trigger overlap cursor {cursor} exceeds total {}",
                        subjects.len()
                    ),
                )],
            });
        }
        let end = cursor.saturating_add(page_size).min(subjects.len());
        Ok(TriggerOverlapPage {
            trigger,
            subjects: subjects[cursor..end].to_vec(),
            revision: self.revision,
            total: subjects.len(),
            next_cursor: (end < subjects.len()).then_some(end),
        })
    }

    pub fn snapshot(&self) -> crate::TriggerVolumeSnapshot {
        crate::TriggerVolumeSnapshot {
            schema_version: TRIGGER_VOLUME_SNAPSHOT_SCHEMA_VERSION,
            revision: self.revision,
            definitions: self.definitions.values().cloned().collect(),
            inactive_triggers: self
                .inactive_triggers
                .iter()
                .map(|value| value.raw())
                .collect(),
            active_overlaps: self.active_overlaps().collect(),
        }
    }

    pub fn from_snapshot(
        snapshot: crate::TriggerVolumeSnapshot,
    ) -> Result<Self, TriggerVolumeError> {
        let mut diagnostics = Vec::new();
        if snapshot.schema_version != 1
            && snapshot.schema_version != TRIGGER_VOLUME_SNAPSHOT_SCHEMA_VERSION
        {
            diagnostics.push(diagnostic(
                TriggerVolumeDiagnosticCode::SnapshotVersion,
                None,
                format!("unsupported schema version {}", snapshot.schema_version),
            ));
        }
        if snapshot.definitions.len() > MAX_TRIGGER_DEFINITIONS
            || snapshot.active_overlaps.len() > MAX_ACTIVE_TRIGGER_OVERLAPS
        {
            diagnostics.push(diagnostic(
                TriggerVolumeDiagnosticCode::QuotaExceeded,
                None,
                "snapshot exceeds trigger definition or overlap limits",
            ));
        }
        let mut definitions = BTreeMap::new();
        for definition in &snapshot.definitions {
            diagnostics.extend(validate_definition(definition));
            if definition.tags.windows(2).any(|pair| pair[0] >= pair[1]) {
                diagnostics.push(diagnostic(
                    TriggerVolumeDiagnosticCode::SnapshotInvariant,
                    Some(definition.trigger_id()),
                    "snapshot definition tags must be sorted and unique",
                ));
            }
            if definitions
                .insert(definition.trigger_id(), definition.clone())
                .is_some()
            {
                diagnostics.push(diagnostic(
                    TriggerVolumeDiagnosticCode::DuplicateDefinition,
                    Some(definition.trigger_id()),
                    "snapshot repeats a trigger definition",
                ));
            }
        }
        let canonical_definitions = definitions.values().cloned().collect::<Vec<_>>();
        if canonical_definitions != snapshot.definitions {
            diagnostics.push(diagnostic(
                TriggerVolumeDiagnosticCode::SnapshotInvariant,
                None,
                "definitions and tags must be sorted and unique",
            ));
        }
        let active_overlaps = snapshot
            .active_overlaps
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if active_overlaps.iter().copied().collect::<Vec<_>>() != snapshot.active_overlaps
            || active_overlaps.iter().any(|pair| {
                pair.trigger == pair.subject || !definitions.contains_key(&pair.trigger_id())
            })
        {
            diagnostics.push(diagnostic(
                TriggerVolumeDiagnosticCode::SnapshotInvariant,
                None,
                "overlaps must be sorted, unique, non-self, and reference definitions",
            ));
        }
        let inactive_triggers = snapshot
            .inactive_triggers
            .iter()
            .copied()
            .map(EntityId::new)
            .collect::<BTreeSet<_>>();
        if inactive_triggers
            .iter()
            .map(|trigger| trigger.raw())
            .collect::<Vec<_>>()
            != snapshot.inactive_triggers
            || inactive_triggers
                .iter()
                .any(|trigger| !definitions.contains_key(trigger))
            || active_overlaps
                .iter()
                .any(|pair| inactive_triggers.contains(&pair.trigger_id()))
        {
            diagnostics.push(diagnostic(
                TriggerVolumeDiagnosticCode::SnapshotInvariant,
                None,
                "inactive triggers must be sorted, unique, registered, and have no overlaps",
            ));
        }
        if !diagnostics.is_empty() {
            return Err(TriggerVolumeError { diagnostics });
        }
        Ok(Self {
            definitions,
            inactive_triggers,
            active_overlaps,
            revision: snapshot.revision,
        })
    }

    fn compute_overlaps(
        &self,
        entities: &EntityState,
    ) -> Result<(BTreeSet<TriggerOverlapPair>, Vec<TriggerVolumeDiagnostic>), TriggerVolumeError>
    {
        let trigger_ids = self.definitions.keys().copied().collect::<BTreeSet<_>>();
        let mut next = BTreeSet::new();
        let mut diagnostics = Vec::new();
        for definition in self.definitions.values() {
            let trigger = definition.trigger_id();
            if self.inactive_triggers.contains(&trigger) {
                continue;
            }
            let Some(trigger_bounds) = live_aabb(
                entities,
                trigger,
                true,
                &mut diagnostics,
                definition.geometry,
            ) else {
                continue;
            };
            for entity in entities.entities() {
                if entity.id == trigger || trigger_ids.contains(&entity.id) {
                    continue;
                }
                let Some(subject_bounds) = live_aabb(
                    entities,
                    entity.id,
                    false,
                    &mut diagnostics,
                    TriggerGeometrySource::ActiveCollision,
                ) else {
                    continue;
                };
                if trigger_bounds.overlaps(subject_bounds) {
                    next.insert(TriggerOverlapPair::new(trigger, entity.id));
                    if next.len() > MAX_ACTIVE_TRIGGER_OVERLAPS {
                        return Err(TriggerVolumeError {
                            diagnostics: vec![diagnostic(
                                TriggerVolumeDiagnosticCode::QuotaExceeded,
                                None,
                                format!("active overlap limit is {MAX_ACTIVE_TRIGGER_OVERLAPS}"),
                            )],
                        });
                    }
                }
            }
        }
        diagnostics.sort_by(|left, right| {
            left.entity
                .cmp(&right.entity)
                .then(left.code.cmp(&right.code))
                .then(left.message.cmp(&right.message))
        });
        diagnostics.dedup();
        Ok((next, diagnostics))
    }

    fn require_revision(&self, expected_revision: u64) -> Result<(), TriggerVolumeError> {
        if self.revision == expected_revision {
            return Ok(());
        }
        Err(TriggerVolumeError {
            diagnostics: vec![diagnostic(
                TriggerVolumeDiagnosticCode::StaleRevision,
                None,
                format!(
                    "expected trigger revision {expected_revision}, actual {}",
                    self.revision
                ),
            )],
        })
    }
}

fn missing_definition(trigger: EntityId) -> TriggerVolumeError {
    TriggerVolumeError {
        diagnostics: vec![diagnostic(
            TriggerVolumeDiagnosticCode::MissingDefinition,
            Some(trigger),
            "trigger definition is not registered",
        )],
    }
}

fn next_revision(revision: u64) -> Result<u64, TriggerVolumeError> {
    revision.checked_add(1).ok_or_else(|| TriggerVolumeError {
        diagnostics: vec![diagnostic(
            TriggerVolumeDiagnosticCode::RevisionOverflow,
            None,
            "trigger revision cannot advance",
        )],
    })
}

fn overlaps_for(
    overlaps: &BTreeSet<TriggerOverlapPair>,
    trigger: EntityId,
) -> Vec<TriggerOverlapPair> {
    overlaps
        .range(
            TriggerOverlapPair::new(trigger, EntityId::new(0))
                ..=TriggerOverlapPair::new(trigger, EntityId::new(u64::MAX)),
        )
        .copied()
        .collect()
}

fn validate_definition(definition: &KinematicTriggerDefinition) -> Vec<TriggerVolumeDiagnostic> {
    let mut diagnostics = Vec::new();
    if !valid_identifier(&definition.scope) {
        diagnostics.push(diagnostic(
            TriggerVolumeDiagnosticCode::InvalidIdentifier,
            Some(definition.trigger_id()),
            "scope must be a non-empty dot, dash, or underscore identifier",
        ));
    }
    for tag in &definition.tags {
        if !valid_identifier(tag) {
            diagnostics.push(diagnostic(
                TriggerVolumeDiagnosticCode::InvalidTag,
                Some(definition.trigger_id()),
                format!("invalid trigger tag `{tag}`"),
            ));
        }
    }
    diagnostics
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

pub(crate) fn diagnostic(
    code: TriggerVolumeDiagnosticCode,
    entity: Option<EntityId>,
    message: impl Into<String>,
) -> TriggerVolumeDiagnostic {
    TriggerVolumeDiagnostic {
        code,
        entity,
        message: message.into(),
    }
}
