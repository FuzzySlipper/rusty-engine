use core_ids::EntityId;
use std::collections::BTreeSet;

use entity_state::{decode_snapshot_with_registry, ComponentRegistry, EntityState};

use crate::{
    combined_gameplay_component_registry, continuous_mechanics_component_registry,
    ContinuousActiveEffectsComponent, ContinuousCatalogVersion, ContinuousMechanicsCatalog,
    ContinuousMechanicsError, ContinuousStatsComponent, ContinuousTracksComponent,
};

pub fn decode_snapshot_with_continuous_catalog(
    input: &str,
    catalog: &ContinuousMechanicsCatalog,
) -> Result<EntityState, ContinuousSnapshotError> {
    let state = decode_snapshot_with_registry(
        input,
        continuous_mechanics_component_registry().expect("fixed continuous registrations"),
    )?;
    validate_state_against_continuous_catalog(&state, catalog)?;
    Ok(state)
}

/// Strictly reconstruct an EntityState carrying both the frozen exact mechanics family and
/// this opt-in continuous family. Neither catalog is merged or re-fingerprinted.
pub fn decode_snapshot_with_catalogs(
    input: &str,
    exact: &gameplay_mechanics::MechanicsCatalog,
    continuous: &ContinuousMechanicsCatalog,
) -> Result<EntityState, ContinuousSnapshotError> {
    let state = decode_snapshot_with_registry(
        input,
        combined_gameplay_component_registry().expect("fixed combined registrations"),
    )?;
    gameplay_mechanics::validate_state_against_catalog(&state, exact)?;
    validate_state_against_continuous_catalog(&state, continuous)?;
    Ok(state)
}

pub fn decode_snapshot_with_continuous_catalog_and_registry(
    input: &str,
    registry: ComponentRegistry,
    catalog: &ContinuousMechanicsCatalog,
) -> Result<EntityState, ContinuousSnapshotError> {
    let state = decode_snapshot_with_registry(input, registry)?;
    validate_state_against_continuous_catalog(&state, catalog)?;
    Ok(state)
}

pub fn validate_state_against_continuous_catalog(
    state: &EntityState,
    catalog: &ContinuousMechanicsCatalog,
) -> Result<(), ContinuousMechanicsError> {
    validate_state_continuous_components(state, catalog, |_| true)
}

/// Validates only the continuous component facts belonging to the supplied
/// exact-world entity subset.  Multi-catalog composition keeps catalog
/// association outside the component payload, so validating an entire shared
/// world against one catalog would reject valid facts owned by another one.
pub fn validate_state_entities_against_continuous_catalog(
    state: &EntityState,
    catalog: &ContinuousMechanicsCatalog,
    entities: &BTreeSet<EntityId>,
) -> Result<(), ContinuousMechanicsError> {
    validate_state_continuous_components(state, catalog, |entity| entities.contains(&entity))
}

fn validate_state_continuous_components(
    state: &EntityState,
    catalog: &ContinuousMechanicsCatalog,
    included: impl Fn(EntityId) -> bool,
) -> Result<(), ContinuousMechanicsError> {
    for (entity, component) in state.components::<ContinuousStatsComponent>()? {
        if !included(entity) {
            continue;
        }
        ensure_version(
            catalog,
            entity,
            ContinuousStatsComponent::LABEL,
            component.catalog_version(),
        )?;
        for value in component.values() {
            let definition = catalog
                .stat(value.stat())
                .ok_or_else(|| ContinuousMechanicsError::UnknownStat(value.stat().clone()))?;
            if value.base() < definition.minimum() || value.base() > definition.maximum() {
                return Err(ContinuousMechanicsError::OutOfBounds {
                    subject: value.stat().to_string(),
                    bits: value.base().bits(),
                    minimum: definition.minimum().bits(),
                    maximum: definition.maximum().bits(),
                });
            }
        }
    }
    for (entity, component) in state.components::<crate::ContinuousIntrinsicSourcesComponent>()? {
        if !included(entity) {
            continue;
        }
        ensure_version(
            catalog,
            entity,
            crate::ContinuousIntrinsicSourcesComponent::LABEL,
            component.catalog_version(),
        )?;
        for binding in component.bindings() {
            if catalog.source(binding.definition()).is_none() {
                return Err(ContinuousMechanicsError::UnknownSource(
                    binding.definition().clone(),
                ));
            }
        }
    }
    for (entity, component) in state.components::<ContinuousActiveEffectsComponent>()? {
        if !included(entity) {
            continue;
        }
        ensure_version(
            catalog,
            entity,
            ContinuousActiveEffectsComponent::LABEL,
            component.catalog_version(),
        )?;
        for effect in component.effects() {
            if catalog.effect(effect.definition()).is_none() {
                return Err(ContinuousMechanicsError::UnknownEffect(
                    effect.definition().clone(),
                ));
            }
        }
    }
    for (entity, component) in state.components::<ContinuousTracksComponent>()? {
        if !included(entity) {
            continue;
        }
        ensure_version(
            catalog,
            entity,
            ContinuousTracksComponent::LABEL,
            component.catalog_version(),
        )?;
        for value in component.values() {
            let (minimum, maximum) =
                crate::continuous_track_bounds(state, catalog, entity, value.track())?;
            if value.current() < minimum || value.current() > maximum {
                return Err(ContinuousMechanicsError::OutOfBounds {
                    subject: value.track().to_string(),
                    bits: value.current().bits(),
                    minimum: minimum.bits(),
                    maximum: maximum.bits(),
                });
            }
        }
    }
    Ok(())
}
fn ensure_version(
    catalog: &ContinuousMechanicsCatalog,
    entity: EntityId,
    component: &'static str,
    actual: &ContinuousCatalogVersion,
) -> Result<(), ContinuousMechanicsError> {
    if actual != catalog.version() {
        return Err(ContinuousMechanicsError::CatalogVersion {
            entity,
            component,
            expected: catalog.version().to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

#[derive(Debug)]
pub enum ContinuousSnapshotError {
    Entity(entity_state::EntityStateSnapshotError),
    Mechanics(ContinuousMechanicsError),
    Exact(gameplay_mechanics::MechanicsError),
}
impl std::fmt::Display for ContinuousSnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "continuous mechanics snapshot rejected: {self:?}")
    }
}
impl std::error::Error for ContinuousSnapshotError {}
impl From<entity_state::EntityStateSnapshotError> for ContinuousSnapshotError {
    fn from(value: entity_state::EntityStateSnapshotError) -> Self {
        Self::Entity(value)
    }
}
impl From<ContinuousMechanicsError> for ContinuousSnapshotError {
    fn from(value: ContinuousMechanicsError) -> Self {
        Self::Mechanics(value)
    }
}
impl From<gameplay_mechanics::MechanicsError> for ContinuousSnapshotError {
    fn from(value: gameplay_mechanics::MechanicsError) -> Self {
        Self::Exact(value)
    }
}
