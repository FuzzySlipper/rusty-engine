//! Picking against the backend's actual retained parts: the nearest
//! triangle a ray hits among shown, filtered parts, at their current world
//! transforms and (for skinned meshes) current pose. Answers the existing
//! `RendererPickRequest` contract; the host supplies the camera it displays.
//!
//! Single-sided parts are hit from their front only, as Three's raycaster
//! respected `FrontSide`. Lines are not picked; sprites belong to #8787.

use glam::{DVec3, Mat4, Vec3};
use render_host_contracts::{
    RendererCompositionCamera, RendererPickHint, RendererPickRay, RendererPickReceipt,
    RendererPickRequest, RendererPickSourceTrace, RendererPickSourceTraceKind,
};

use crate::camera;
use crate::Renderer;

/// Triangle hits closer than this along the ray are ignored (self-hits).
const RAY_EPSILON: f32 = 1e-6;

struct Hit {
    distance: f32,
    part: usize,
    position: Vec3,
    normal: Vec3,
}

impl Renderer {
    /// Pick the nearest retained part along `request.ray`. A viewport ray is
    /// cast through `camera` over a viewport of `width`×`height` pixels.
    pub fn pick(
        &self,
        request: &RendererPickRequest,
        camera: &RendererCompositionCamera,
        width: u32,
        height: u32,
    ) -> RendererPickReceipt {
        let (origin, direction) = match request.ray {
            RendererPickRay::WorldRay { origin, direction } => (
                DVec3::from(origin).as_vec3(),
                DVec3::from(direction).as_vec3().normalize_or_zero(),
            ),
            RendererPickRay::Viewport { point } => {
                let aspect = width.max(1) as f32 / height.max(1) as f32;
                let matrices = camera::camera_matrices(
                    camera::descriptor_pose(camera),
                    &camera.projection,
                    aspect,
                );
                let inverse = matrices.view_proj.inverse();
                let near = inverse.project_point3(Vec3::new(point[0] as f32, point[1] as f32, 0.0));
                let far = inverse.project_point3(Vec3::new(point[0] as f32, point[1] as f32, 1.0));
                (near, (far - near).normalize_or_zero())
            }
        };
        if direction == Vec3::ZERO {
            return RendererPickReceipt {
                diagnostics: vec![render_host_contracts::RendererHostDiagnostic {
                    code: "invalidRay".to_owned(),
                    message: "pick ray has no direction".to_owned(),
                    sequence: None,
                    handle: None,
                }],
                hint: None,
            };
        }
        let max_distance = request.max_distance.map_or(f32::INFINITY, |max| max as f32);
        let mut best: Option<Hit> = None;
        let parts = &self.tables.parts;
        for (index, part) in parts.meta.iter().enumerate() {
            let Some(part) = part else { continue };
            let state = &parts.state[index];
            if !state.shown || state.class.lines {
                continue;
            }
            if let Some(filter) = &request.filter {
                let metadata = self.tables.metadata.get(&part.node);
                let label = metadata.and_then(|metadata| metadata.label.as_ref());
                let tags = metadata.map_or(&[][..], |metadata| metadata.tags.as_slice());
                if (!filter.handles.is_empty() && !filter.handles.contains(&part.node))
                    || (!filter.layers.is_empty() && !filter.layers.contains(&state.layer))
                    || (!filter.labels.is_empty()
                        && !label.is_some_and(|label| filter.labels.contains(label)))
                    || (!filter.tags.is_empty()
                        && !tags.iter().any(|tag| filter.tags.contains(tag)))
                {
                    continue;
                }
            }
            let limit = best.as_ref().map_or(max_distance, |hit| hit.distance);
            if !ray_hits_box(origin, direction, &state.world_bounds, limit) {
                continue;
            }
            let Some(geometry) = self.mesh(&part.mesh).map(|mesh| &mesh.cpu) else {
                continue;
            };
            let world = self.part_world(index);
            let first = part.first_index as usize;
            let last = (first + part.index_count as usize).min(geometry.indices.len());
            for triangle in geometry.indices[first..last].chunks_exact(3) {
                let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                    .map(|vertex| world.transform_point3(geometry.positions[vertex as usize]));
                let Some(distance) = intersect(
                    origin,
                    direction,
                    [a, b, c],
                    state.class.double_sided,
                    state.mirrored,
                ) else {
                    continue;
                };
                if distance < best.as_ref().map_or(max_distance, |hit| hit.distance) {
                    let mut normal = (b - a).cross(c - a).normalize_or_zero();
                    if state.mirrored {
                        normal = -normal;
                    }
                    if normal.dot(direction) > 0.0 {
                        normal = -normal;
                    }
                    best = Some(Hit {
                        distance,
                        part: index,
                        position: origin + direction * distance,
                        normal,
                    });
                }
            }
        }
        let hint = best.and_then(|hit| {
            let part = parts.meta[hit.part].as_ref()?;
            let metadata = self.tables.metadata.get(&part.node);
            Some(RendererPickHint {
                distance: f64::from(hit.distance),
                handle: part.node,
                label: metadata.and_then(|metadata| metadata.label.clone()),
                layer: parts.state[hit.part].layer,
                normal: hit.normal.as_dvec3().to_array(),
                position: hit.position.as_dvec3().to_array(),
                source_trace: metadata
                    .and_then(|metadata| metadata.source_entity)
                    .map(|entity| RendererPickSourceTrace {
                        entity,
                        kind: RendererPickSourceTraceKind::RenderMetadataEntity,
                    }),
                tags: metadata.map_or_else(Vec::new, |metadata| metadata.tags.clone()),
            })
        });
        RendererPickReceipt {
            diagnostics: Vec::new(),
            hint,
        }
    }

    /// A part's world transform as its GPU row holds it.
    fn part_world(&self, part: usize) -> Mat4 {
        let floats = &self.tables.parts.gpu[part * crate::tables::PART_ROW_FLOATS..][..16];
        Mat4::from_cols_slice(floats)
    }
}

/// Slab test against a world box, up to `limit` along the ray.
fn ray_hits_box(origin: Vec3, direction: Vec3, bounds: &crate::tables::Aabb, limit: f32) -> bool {
    if bounds.is_empty() {
        return false;
    }
    let inverse = direction.recip();
    let t0 = (bounds.min - origin) * inverse;
    let t1 = (bounds.max - origin) * inverse;
    let near = t0.min(t1).max_element();
    let far = t0.max(t1).min_element();
    far >= near.max(0.0) && near <= limit
}

/// Möller–Trumbore; the distance along a unit ray, if the triangle is hit
/// from a side that draws.
fn intersect(
    origin: Vec3,
    direction: Vec3,
    [a, b, c]: [Vec3; 3],
    double_sided: bool,
    mirrored: bool,
) -> Option<f32> {
    let (edge1, edge2) = (b - a, c - a);
    let p = direction.cross(edge2);
    let determinant = edge1.dot(p);
    // `determinant = -direction · (edge1 × edge2)`: positive when a
    // counter-clockwise (front) face is seen head-on; mirrored parts wind
    // the other way.
    let front = (determinant > 0.0) != mirrored;
    if determinant.abs() < f32::EPSILON || (!double_sided && !front) {
        return None;
    }
    let inverse = 1.0 / determinant;
    let offset = origin - a;
    let u = offset.dot(p) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = offset.cross(edge1);
    let v = direction.dot(q) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = edge2.dot(q) * inverse;
    (distance > RAY_EPSILON).then_some(distance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangles_are_hit_from_their_front_unless_double_sided() {
        // Counter-clockwise seen from +Z.
        let triangle = [
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let from_front = intersect(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::NEG_Z,
            triangle,
            false,
            false,
        );
        assert_eq!(from_front, Some(5.0));
        assert_eq!(
            intersect(Vec3::new(0.0, 0.0, -5.0), Vec3::Z, triangle, false, false),
            None
        );
        assert_eq!(
            intersect(Vec3::new(0.0, 0.0, -5.0), Vec3::Z, triangle, true, false),
            Some(5.0)
        );
        assert_eq!(
            intersect(Vec3::new(3.0, 0.0, 5.0), Vec3::NEG_Z, triangle, true, false),
            None
        );
    }
}
