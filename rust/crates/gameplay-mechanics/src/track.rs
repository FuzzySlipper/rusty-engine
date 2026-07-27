use core_ids::EntityId;
use entity_state::{ComponentRevision, EntityAuthoringService, EntityState};

use crate::{
    stat::track_bounds, MechanicsCatalog, MechanicsComponentKind, MechanicsError, MechanicsScalar,
    ObservedComponentRevision, OperationId, SourceCollectionCost, SourceInstanceIdentity, TrackId,
    TracksComponent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackAdjustmentKind {
    Spend,
    Restore,
}

#[derive(Debug, Clone)]
pub struct TrackMutationRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub entity: EntityId,
    pub track: TrackId,
    pub amount: MechanicsScalar,
    pub kind: TrackAdjustmentKind,
    pub expected_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMutationReceipt {
    pub catalog_version: crate::CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub entity: EntityId,
    pub track: TrackId,
    pub kind: TrackAdjustmentKind,
    pub requested_amount: MechanicsScalar,
    pub applied_amount: MechanicsScalar,
    pub before: MechanicsScalar,
    pub after: MechanicsScalar,
    pub minimum: MechanicsScalar,
    pub maximum: MechanicsScalar,
    pub observed_tracks_revision: u64,
    pub committed_tracks_revision: u64,
    pub observed_revisions: Vec<ObservedComponentRevision>,
    pub source_cost: SourceCollectionCost,
}

#[derive(Debug, Clone)]
pub struct TrackReconciliationRequest {
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub entity: EntityId,
    pub track: TrackId,
    pub prospective_maximum: MechanicsScalar,
    pub expected_revision: Option<ComponentRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackReconciliationReceipt {
    pub catalog_version: crate::CatalogVersion,
    pub catalog_fingerprint: String,
    pub operation: OperationId,
    pub source: SourceInstanceIdentity,
    pub entity: EntityId,
    pub track: TrackId,
    pub minimum: MechanicsScalar,
    pub current_maximum: MechanicsScalar,
    pub prospective_maximum: MechanicsScalar,
    pub before: MechanicsScalar,
    pub after: MechanicsScalar,
    pub observed_tracks_revision: u64,
    pub committed_tracks_revision: u64,
    pub observed_revisions: Vec<ObservedComponentRevision>,
    pub source_cost: SourceCollectionCost,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct TrackService;

impl TrackService {
    pub fn spend(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        mut request: TrackMutationRequest,
    ) -> Result<TrackMutationReceipt, MechanicsError> {
        request.kind = TrackAdjustmentKind::Spend;
        Self::adjust(state, catalog, request)
    }

    pub fn restore(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        mut request: TrackMutationRequest,
    ) -> Result<TrackMutationReceipt, MechanicsError> {
        request.kind = TrackAdjustmentKind::Restore;
        Self::adjust(state, catalog, request)
    }

    pub fn adjust(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: TrackMutationRequest,
    ) -> Result<TrackMutationReceipt, MechanicsError> {
        let amount = request.amount.require_nonnegative()?;
        let actual_revision = state.component_revision::<TracksComponent>(request.entity)?;
        if let Some(expected) = &request.expected_revision {
            ensure_revision(expected, &actual_revision)?;
        }
        let publish_revision = request
            .expected_revision
            .clone()
            .unwrap_or_else(|| actual_revision.clone());
        let component = state.component::<TracksComponent>(request.entity)?.ok_or(
            MechanicsError::MissingComponent {
                entity: request.entity,
                component: TracksComponent::LABEL,
            },
        )?;
        crate::source::ensure_catalog_version(
            catalog,
            request.entity,
            TracksComponent::LABEL,
            component.catalog_version(),
        )?;
        let before =
            component
                .current(&request.track)
                .ok_or_else(|| MechanicsError::MissingTrack {
                    entity: request.entity,
                    track: request.track.clone(),
                })?;
        let (minimum, maximum, mut observed_revisions, source_cost) = track_bounds(
            state,
            catalog,
            request.entity,
            &request.track,
            &request.operation,
        )?;
        if before < minimum || before > maximum {
            return Err(MechanicsError::TrackOutOfBounds {
                entity: request.entity,
                track: request.track.clone(),
                attempted: before.get(),
                minimum: minimum.get(),
                maximum: maximum.get(),
            });
        }

        let after = match request.kind {
            TrackAdjustmentKind::Spend => {
                let attempted = before.checked_sub(amount)?;
                if attempted < minimum {
                    return Err(MechanicsError::TrackOutOfBounds {
                        entity: request.entity,
                        track: request.track.clone(),
                        attempted: attempted.get(),
                        minimum: minimum.get(),
                        maximum: maximum.get(),
                    });
                }
                attempted
            }
            TrackAdjustmentKind::Restore => {
                let room = maximum.checked_sub(before)?;
                if amount >= room {
                    maximum
                } else {
                    before.checked_add(amount)?
                }
            }
        };
        let applied_amount = match request.kind {
            TrackAdjustmentKind::Spend => before.checked_sub(after)?,
            TrackAdjustmentKind::Restore => after.checked_sub(before)?,
        };
        let mut candidate = component.clone();
        assert!(candidate.set_current(&request.track, after));
        EntityAuthoringService.replace_component(
            state,
            publish_revision,
            request.entity,
            candidate,
        )?;
        let committed_revision = state.component_revision::<TracksComponent>(request.entity)?;
        observed_revisions.push(ObservedComponentRevision {
            entity: request.entity,
            component: MechanicsComponentKind::Tracks,
            revision: actual_revision.revision(),
        });
        observed_revisions.sort_by_key(|value| (value.entity, value.component));
        observed_revisions.dedup();

        Ok(TrackMutationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            source: request.source,
            entity: request.entity,
            track: request.track,
            kind: request.kind,
            requested_amount: amount,
            applied_amount,
            before,
            after,
            minimum,
            maximum,
            observed_tracks_revision: actual_revision.revision(),
            committed_tracks_revision: committed_revision.revision(),
            observed_revisions,
            source_cost,
        })
    }

    /// Lowers a current value before a separate source/effect change lowers its bound.
    ///
    /// The prospective bound is supplied by the owner staging that later source change. It may
    /// only tighten the currently admitted bound. This makes the intermediate state valid and
    /// lets the later component mutation reject safely without a cross-component transaction.
    pub fn reconcile_to_maximum(
        state: &mut EntityState,
        catalog: &MechanicsCatalog,
        request: TrackReconciliationRequest,
    ) -> Result<TrackReconciliationReceipt, MechanicsError> {
        let actual_revision = state.component_revision::<TracksComponent>(request.entity)?;
        if let Some(expected) = &request.expected_revision {
            ensure_revision(expected, &actual_revision)?;
        }
        let publish_revision = request
            .expected_revision
            .clone()
            .unwrap_or_else(|| actual_revision.clone());
        let component = state.component::<TracksComponent>(request.entity)?.ok_or(
            MechanicsError::MissingComponent {
                entity: request.entity,
                component: TracksComponent::LABEL,
            },
        )?;
        crate::source::ensure_catalog_version(
            catalog,
            request.entity,
            TracksComponent::LABEL,
            component.catalog_version(),
        )?;
        let before =
            component
                .current(&request.track)
                .ok_or_else(|| MechanicsError::MissingTrack {
                    entity: request.entity,
                    track: request.track.clone(),
                })?;
        let (minimum, current_maximum, mut observed_revisions, source_cost) = track_bounds(
            state,
            catalog,
            request.entity,
            &request.track,
            &request.operation,
        )?;
        if request.prospective_maximum < minimum || request.prospective_maximum > current_maximum {
            return Err(MechanicsError::InvalidResolvedTrackBounds {
                entity: request.entity,
                track: request.track.clone(),
                minimum: minimum.get(),
                maximum: request.prospective_maximum.get(),
            });
        }
        if before < minimum || before > current_maximum {
            return Err(MechanicsError::TrackOutOfBounds {
                entity: request.entity,
                track: request.track.clone(),
                attempted: before.get(),
                minimum: minimum.get(),
                maximum: current_maximum.get(),
            });
        }
        let after = before.min(request.prospective_maximum);
        let mut candidate = component.clone();
        assert!(candidate.set_current(&request.track, after));
        EntityAuthoringService.replace_component(
            state,
            publish_revision,
            request.entity,
            candidate,
        )?;
        let committed_revision = state.component_revision::<TracksComponent>(request.entity)?;
        observed_revisions.push(ObservedComponentRevision {
            entity: request.entity,
            component: MechanicsComponentKind::Tracks,
            revision: actual_revision.revision(),
        });
        observed_revisions.sort_by_key(|value| (value.entity, value.component));
        observed_revisions.dedup();

        Ok(TrackReconciliationReceipt {
            catalog_version: catalog.version().clone(),
            catalog_fingerprint: catalog.fingerprint().to_string(),
            operation: request.operation,
            source: request.source,
            entity: request.entity,
            track: request.track,
            minimum,
            current_maximum,
            prospective_maximum: request.prospective_maximum,
            before,
            after,
            observed_tracks_revision: actual_revision.revision(),
            committed_tracks_revision: committed_revision.revision(),
            observed_revisions,
            source_cost,
        })
    }
}

pub(crate) fn ensure_revision(
    expected: &ComponentRevision,
    actual: &ComponentRevision,
) -> Result<(), MechanicsError> {
    if expected.entity() != actual.entity() || expected.component() != actual.component() {
        return Err(MechanicsError::ComponentRevisionScopeMismatch {
            expected_entity: expected.entity(),
            actual_entity: actual.entity(),
            expected_component: expected.component().to_string(),
            actual_component: actual.component().to_string(),
        });
    }
    if expected.revision() != actual.revision() {
        return Err(MechanicsError::StaleComponentRevision {
            expected: expected.revision(),
            actual: actual.revision(),
        });
    }
    Ok(())
}
