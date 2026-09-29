use std::collections::{BTreeMap, BTreeSet};

use core_ids::EntityId;
use core_math::Vec3;
/// Selects whether a registered trigger needs enabled collision to sense.
///
/// `ActiveCollision` senses only while the trigger's collider is enabled.
/// `EntityBounds` senses from the collider's AABB whatever its collision
/// flag, so the trigger never has to become a solid motion obstacle. Subjects
/// always need enabled collision, whatever the trigger's geometry source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TriggerGeometrySource {
    #[default]
    ActiveCollision,
    EntityBounds,
}

/// One product collider for a reconcile or restore call: a world-space AABB
/// and whether its collision is enabled. Registered trigger entities use
/// their row as trigger geometry; every other row is a potential subject.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriggerCollider {
    pub entity: EntityId,
    pub min: Vec3,
    pub max: Vec3,
    pub collision_enabled: bool,
}

impl TriggerCollider {
    fn overlaps(&self, other: &Self) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KinematicTriggerDefinition {
    pub trigger: u64,
    pub scope: String,
    pub tags: Vec<String>,
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
    /// from its AABB without requiring enabled collision.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    InactiveCollision,
    DuplicateLifecycle,
}

impl TriggerVolumeDiagnosticCode {
    pub const fn code(self) -> &'static str {
        match self {
            Self::DuplicateDefinition => "duplicate-trigger-definition",
            Self::MissingDefinition => "missing-trigger-definition",
            Self::InvalidIdentifier => "invalid-trigger-identifier",
            Self::InvalidTag => "invalid-trigger-tag",
            Self::StaleEntity => "stale-trigger-entity",
            Self::InactiveCollision => "trigger-inactive-collision",
            Self::DuplicateLifecycle => "duplicate-trigger-lifecycle",
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerReconcileReceipt {
    pub tick: u64,
    pub cause: TriggerReconcileCause,
    pub facts: Vec<TriggerOverlapFact>,
    pub continued: Vec<TriggerOverlapPair>,
    pub active_overlaps: Vec<TriggerOverlapPair>,
    pub diagnostics: Vec<TriggerVolumeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerLifecycleReceipt {
    pub trigger: EntityId,
    pub active: bool,
    pub removed_overlaps: Vec<TriggerOverlapPair>,
    pub facts: Vec<TriggerOverlapFact>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerRestoreReceipt {
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

    pub fn set_active(
        &mut self,
        trigger: EntityId,
        active: bool,
        tick: u64,
    ) -> Result<TriggerLifecycleReceipt, TriggerVolumeError> {
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
        Ok(TriggerLifecycleReceipt {
            trigger,
            active,
            removed_overlaps,
            facts,
        })
    }

    pub fn restore(
        &mut self,
        active_triggers: &[EntityId],
        colliders: impl IntoIterator<Item = TriggerCollider>,
    ) -> Result<TriggerRestoreReceipt, TriggerVolumeError> {
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
        let (active_overlaps, diagnostics) = candidate.compute_overlaps(colliders);
        candidate.active_overlaps = active_overlaps;
        *self = candidate;
        Ok(TriggerRestoreReceipt {
            registered_count: self.definitions.len(),
            active_count: self.active_trigger_count(),
            active_overlaps: self.active_overlaps().collect(),
            diagnostics,
        })
    }

    pub fn reconcile(
        &mut self,
        colliders: impl IntoIterator<Item = TriggerCollider>,
        tick: u64,
        cause: TriggerReconcileCause,
    ) -> TriggerReconcileReceipt {
        let (next, diagnostics) = self.compute_overlaps(colliders);
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
        self.active_overlaps = next;
        TriggerReconcileReceipt {
            tick,
            cause,
            facts,
            continued,
            active_overlaps: self.active_overlaps().collect(),
            diagnostics,
        }
    }

    pub fn current_overlaps(
        &self,
        trigger: EntityId,
    ) -> Result<TriggerOverlapReadout, TriggerVolumeError> {
        if !self.definitions.contains_key(&trigger) {
            return Err(missing_definition(trigger));
        }
        let subjects = overlaps_for(&self.active_overlaps, trigger)
            .into_iter()
            .map(TriggerOverlapPair::subject_id)
            .collect();
        Ok(TriggerOverlapReadout { trigger, subjects })
    }

    fn compute_overlaps(
        &self,
        colliders: impl IntoIterator<Item = TriggerCollider>,
    ) -> (BTreeSet<TriggerOverlapPair>, Vec<TriggerVolumeDiagnostic>) {
        let mut triggers = BTreeMap::new();
        let mut subjects = Vec::new();
        for collider in colliders {
            if self.definitions.contains_key(&collider.entity) {
                triggers.insert(collider.entity, collider);
            } else if collider.collision_enabled {
                subjects.push(collider);
            }
        }
        let mut next = BTreeSet::new();
        let mut diagnostics = Vec::new();
        for definition in self.definitions.values() {
            let trigger = definition.trigger_id();
            if self.inactive_triggers.contains(&trigger) {
                continue;
            }
            let Some(bounds) = triggers.get(&trigger) else {
                diagnostics.push(diagnostic(
                    TriggerVolumeDiagnosticCode::StaleEntity,
                    Some(trigger),
                    "trigger entity has no collider row",
                ));
                continue;
            };
            if definition.geometry == TriggerGeometrySource::ActiveCollision
                && !bounds.collision_enabled
            {
                diagnostics.push(diagnostic(
                    TriggerVolumeDiagnosticCode::InactiveCollision,
                    Some(trigger),
                    "trigger collision is disabled",
                ));
                continue;
            }
            next.extend(
                subjects
                    .iter()
                    .filter(|subject| bounds.overlaps(subject))
                    .map(|subject| TriggerOverlapPair::new(trigger, subject.entity)),
            );
        }
        (next, diagnostics)
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
