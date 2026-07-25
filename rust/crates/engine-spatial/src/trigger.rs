use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use entity_state::EntityState;
use serde::{Deserialize, Serialize};

use crate::trigger_geometry::live_aabb;

pub const TRIGGER_VOLUME_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub const MAX_TRIGGER_DEFINITIONS: usize = 4_096;
pub const MAX_ACTIVE_TRIGGER_OVERLAPS: usize = 1_000_000;
const MAX_TRIGGER_READ_ITEMS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KinematicTriggerDefinition {
    pub trigger: u64,
    pub scope: String,
    pub tags: Vec<String>,
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
        }
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TriggerVolumeSystem {
    definitions: BTreeMap<EntityId, KinematicTriggerDefinition>,
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

    pub const fn revision(&self) -> u64 {
        self.revision
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
            self.revision
                .checked_add(1)
                .ok_or_else(|| TriggerVolumeError {
                    diagnostics: vec![diagnostic(
                        TriggerVolumeDiagnosticCode::RevisionOverflow,
                        None,
                        "trigger overlap revision cannot advance",
                    )],
                })?
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

    pub fn snapshot(&self) -> crate::TriggerVolumeSnapshot {
        crate::TriggerVolumeSnapshot {
            schema_version: TRIGGER_VOLUME_SNAPSHOT_SCHEMA_VERSION,
            revision: self.revision,
            definitions: self.definitions.values().cloned().collect(),
            active_overlaps: self.active_overlaps().collect(),
        }
    }

    pub fn from_snapshot(
        snapshot: crate::TriggerVolumeSnapshot,
    ) -> Result<Self, TriggerVolumeError> {
        let mut diagnostics = Vec::new();
        if snapshot.schema_version != TRIGGER_VOLUME_SNAPSHOT_SCHEMA_VERSION {
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
        if !diagnostics.is_empty() {
            return Err(TriggerVolumeError { diagnostics });
        }
        Ok(Self {
            definitions,
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
        for trigger in self.definitions.keys().copied() {
            let Some(trigger_bounds) = live_aabb(entities, trigger, true, &mut diagnostics) else {
                continue;
            };
            for entity in entities.entities() {
                if entity.id == trigger || trigger_ids.contains(&entity.id) {
                    continue;
                }
                let Some(subject_bounds) = live_aabb(entities, entity.id, false, &mut diagnostics)
                else {
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
