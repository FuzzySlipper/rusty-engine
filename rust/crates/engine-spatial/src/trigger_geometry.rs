use core_ids::EntityId;
use core_math::Vec3;
use entity_state::{EntityLifecycle, EntityState};

use crate::trigger::diagnostic;
use crate::{TriggerVolumeDiagnostic, TriggerVolumeDiagnosticCode};

#[derive(Debug, Clone, Copy)]
pub(crate) struct WorldAabb {
    min: Vec3,
    max: Vec3,
}

impl WorldAabb {
    pub(crate) fn overlaps(self, other: Self) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
            && self.min.z < other.max.z
            && self.max.z > other.min.z
    }
}

pub(crate) fn live_aabb(
    entities: &EntityState,
    entity: EntityId,
    report: bool,
    diagnostics: &mut Vec<TriggerVolumeDiagnostic>,
) -> Option<WorldAabb> {
    let Some(core) = entities.core(entity) else {
        report_diagnostic(
            report,
            diagnostics,
            TriggerVolumeDiagnosticCode::StaleEntity,
            entity,
            "entity is missing",
        );
        return None;
    };
    if core.lifecycle != EntityLifecycle::Active {
        report_diagnostic(
            report,
            diagnostics,
            TriggerVolumeDiagnosticCode::StaleEntity,
            entity,
            "entity is not active",
        );
        return None;
    }
    if entities.collision(entity).is_none() {
        report_diagnostic(
            report,
            diagnostics,
            TriggerVolumeDiagnosticCode::MissingCollision,
            entity,
            "collision capability is missing",
        );
        return None;
    }
    if entities.active_collision(entity).is_none() {
        report_diagnostic(
            report,
            diagnostics,
            TriggerVolumeDiagnosticCode::InactiveCollision,
            entity,
            "collision capability is inactive",
        );
        return None;
    }
    let Some(bounds) = entities.bounds(entity) else {
        report_diagnostic(
            report,
            diagnostics,
            TriggerVolumeDiagnosticCode::MissingBounds,
            entity,
            "bounds capability is missing",
        );
        return None;
    };
    let Some(transform) = entities.world_transform(entity) else {
        report_diagnostic(
            report,
            diagnostics,
            TriggerVolumeDiagnosticCode::MissingTransform,
            entity,
            "world transform is missing",
        );
        return None;
    };
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for x in [bounds.min.x, bounds.max.x] {
        for y in [bounds.min.y, bounds.max.y] {
            for z in [bounds.min.z, bounds.max.z] {
                let point = transform.transform_point(Vec3::new(x, y, z));
                min.x = min.x.min(point.x);
                min.y = min.y.min(point.y);
                min.z = min.z.min(point.z);
                max.x = max.x.max(point.x);
                max.y = max.y.max(point.y);
                max.z = max.z.max(point.z);
            }
        }
    }
    Some(WorldAabb { min, max })
}

fn report_diagnostic(
    report: bool,
    diagnostics: &mut Vec<TriggerVolumeDiagnostic>,
    code: TriggerVolumeDiagnosticCode,
    entity: EntityId,
    message: &'static str,
) {
    if report {
        diagnostics.push(diagnostic(code, Some(entity), message));
    }
}
