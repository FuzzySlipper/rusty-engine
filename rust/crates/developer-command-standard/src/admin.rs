//! Privileged adapters that preserve the named mechanics owner API exactly.

use entity_state::EntityState;
use gameplay_mechanics::{
    EffectApplyRequest, EffectMutationReceipt, EffectRemovalRequest, EffectService,
    MechanicsCatalog, MechanicsError, StatBaseMutationReceipt, StatBaseMutationRequest,
    StatService, TrackService, TrackSetReceipt, TrackSetRequest,
};

/// Calls `StatService::set_base` without wrapping its receipt or error.
pub fn admin_set_stat_base(
    state: &mut EntityState,
    catalog: &MechanicsCatalog,
    request: StatBaseMutationRequest,
) -> Result<StatBaseMutationReceipt, MechanicsError> {
    StatService::set_base(state, catalog, request)
}

/// Calls `TrackService::set_under_policy` without choosing a policy for the product.
pub fn admin_set_track(
    state: &mut EntityState,
    catalog: &MechanicsCatalog,
    request: TrackSetRequest,
) -> Result<TrackSetReceipt, MechanicsError> {
    TrackService::set_under_policy(state, catalog, request)
}

/// Calls `EffectService::apply` without wrapping its receipt or error.
pub fn admin_apply_effect(
    state: &mut EntityState,
    catalog: &MechanicsCatalog,
    request: EffectApplyRequest,
) -> Result<EffectMutationReceipt, MechanicsError> {
    EffectService::apply(state, catalog, request)
}

/// Calls `EffectService::remove` without wrapping its receipt or error.
pub fn admin_remove_effect(
    state: &mut EntityState,
    catalog: &MechanicsCatalog,
    request: EffectRemovalRequest,
) -> Result<EffectMutationReceipt, MechanicsError> {
    EffectService::remove(state, catalog, request)
}
