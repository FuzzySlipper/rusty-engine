//! Exact queries against a GLB model's own triangles (`QueryModelSurface`,
//! `CastModelRays`): the nearest surface to points, and where rays meet it,
//! with the model placed where the caller says, in the caller's space. The
//! model's mesh nodes are taken at rest under their node transforms (an
//! animated model in its bind pose). Nothing is retained between calls and
//! nothing enters collision: a held viewmodel can be measured against hands
//! in camera space.

use super::*;
use engine_spatial::{ModelSurface, StaticMeshTransform};

fn surface_error(message: String) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_MODEL_SURFACE", message)
}

/// The model's triangles placed by `placement`.
fn placed_surface(
    bridge: &RuntimeAppearanceBridge,
    model: NativeRenderResourceHandle,
    placement: NativeTransform,
) -> Result<ModelSurface, CsharpEngineServicesError> {
    let resource = bridge.resource(model.value)?;
    if resource.animated_mesh().is_none() {
        return Err(CsharpEngineServicesError::new(
            "CSHARP_ANIMATION_RESOURCE",
            "resource is not a GLB model (open it with OpenAnimatedMesh)",
        ));
    }
    let model = asset_import::glb_model_triangles(resource.bytes()).map_err(|error| {
        surface_error(format!("the model's triangles could not be read: {error}"))
    })?;
    let vector = |value: NativeVec3| [value.x, value.y, value.z].map(f64::from);
    let rotation = placement.rotation;
    let placement = StaticMeshTransform {
        translation: vector(placement.translation),
        rotation: [rotation.x, rotation.y, rotation.z, rotation.w].map(f64::from),
        scale: vector(placement.scale),
    };
    ModelSurface::new(&model.positions, model.triangles, placement)
        .map_err(|error| surface_error(format!("the placed model is not a surface: {error:?}")))
}

fn native(value: [f64; 3]) -> NativeVec3 {
    NativeVec3 {
        x: value[0] as f32,
        y: value[1] as f32,
        z: value[2] as f32,
    }
}

fn point(value: NativeVec3) -> [f64; 3] {
    [value.x, value.y, value.z].map(f64::from)
}

pub(crate) unsafe extern "C" fn query_model_surface(
    context: *mut c_void,
    request: *const NativeModelSurfaceRequest,
    result: *mut NativeModelSurfacePointsResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        let request = unsafe { *request };
        animation_result(context, result, |bridge| {
            // SAFETY: the caller keeps the points valid for this call.
            let points = unsafe {
                borrowed_slice(request.points, request.points_len, "model surface points")?
            };
            let surface = placed_surface(bridge, request.model, request.placement)?;
            let max_distance = f64::from(request.max_distance);
            let found: Vec<NativeModelSurfacePoint> = points
                .iter()
                .map(|query| {
                    surface.nearest(point(*query), max_distance).map_or_else(
                        NativeModelSurfacePoint::default,
                        |nearest| NativeModelSurfacePoint {
                            found: true,
                            point: native(nearest.point),
                            normal: native(nearest.normal),
                            distance: nearest.distance as f32,
                            inside: nearest.inside,
                        },
                    )
                })
                .collect();
            let result = NativeModelSurfacePointsResult {
                points: found.as_ptr(),
                points_len: found.len(),
            };
            bridge.borrowed.hold(found);
            Ok(result)
        })
    })
}

pub(crate) unsafe extern "C" fn cast_model_rays(
    context: *mut c_void,
    request: *const NativeModelRayRequest,
    result: *mut NativeModelRayHitsResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        let request = unsafe { *request };
        animation_result(context, result, |bridge| {
            // SAFETY: the caller keeps the rays valid for this call.
            let rays = unsafe { borrowed_slice(request.rays, request.rays_len, "model rays")? };
            let surface = placed_surface(bridge, request.model, request.placement)?;
            let max_distance = f64::from(request.max_distance);
            let hits: Vec<NativeModelRayHit> = rays
                .iter()
                .map(|ray| {
                    surface
                        .raycast(point(ray.origin), point(ray.direction), max_distance)
                        .map_or_else(NativeModelRayHit::default, |hit| NativeModelRayHit {
                            hit: true,
                            point: native(hit.point),
                            normal: native(hit.normal),
                            distance: hit.distance as f32,
                            back_face: hit.back_face,
                        })
                })
                .collect();
            let result = NativeModelRayHitsResult {
                hits: hits.as_ptr(),
                hits_len: hits.len(),
            };
            bridge.borrowed.hold(hits);
            Ok(result)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GLB prop of two closed cubes (side 1, about their origins) on mesh
    /// nodes under one parent: the parent turned 90° about y and raised 1,
    /// the children 1 to either side of it, the second at half size. In the
    /// model, the first cube's centre is (0, 1, 1) and the second's
    /// (0, 1, -1).
    fn two_cube_prop() -> Vec<u8> {
        let mut positions: Vec<f32> = Vec::new();
        let mut normals: Vec<f32> = Vec::new();
        let mut indices: Vec<u16> = Vec::new();
        let faces: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
            ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]),
            ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
            ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            ([0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]),
        ];
        for (normal, u, v) in faces {
            let base = (positions.len() / 3) as u16;
            for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                for axis in 0..3 {
                    positions.push(0.5 * (normal[axis] + u[axis] * a + v[axis] * b));
                }
                normals.extend(normal);
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let mut bin: Vec<u8> = Vec::new();
        positions.iter().for_each(|v| bin.extend(v.to_le_bytes()));
        normals.iter().for_each(|v| bin.extend(v.to_le_bytes()));
        indices.iter().for_each(|v| bin.extend(v.to_le_bytes()));
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let vertex_bytes = positions.len() * 4;
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let json = serde_json::json!({
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [
                {"name": "Prop", "children": [1, 2], "rotation": [0.0, half, 0.0, half], "translation": [0.0, 1.0, 0.0]},
                {"name": "Grip", "mesh": 0, "translation": [-1.0, 0.0, 0.0]},
                {"name": "Barrel", "mesh": 0, "translation": [1.0, 0.0, 0.0], "scale": [0.5, 0.5, 0.5]},
            ],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1}, "indices": 2}]}],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 24, "type": "VEC3",
                 "min": [-0.5, -0.5, -0.5], "max": [0.5, 0.5, 0.5]},
                {"bufferView": 1, "componentType": 5126, "count": 24, "type": "VEC3"},
                {"bufferView": 2, "componentType": 5123, "count": 36, "type": "SCALAR"},
            ],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": vertex_bytes},
                {"buffer": 0, "byteOffset": vertex_bytes, "byteLength": vertex_bytes},
                {"buffer": 0, "byteOffset": 2 * vertex_bytes, "byteLength": indices.len() * 2},
            ],
            "buffers": [{"byteLength": bin.len()}],
        });
        let mut json = serde_json::to_vec(&json).unwrap();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut glb = Vec::new();
        glb.extend(b"glTF");
        glb.extend(2u32.to_le_bytes());
        glb.extend(((12 + 8 + json.len() + 8 + bin.len()) as u32).to_le_bytes());
        glb.extend((json.len() as u32).to_le_bytes());
        glb.extend(b"JSON");
        glb.extend(&json);
        glb.extend((bin.len() as u32).to_le_bytes());
        glb.extend(b"BIN\0");
        glb.extend(&bin);
        glb
    }

    fn vec3(x: f32, y: f32, z: f32) -> NativeVec3 {
        NativeVec3 { x, y, z }
    }

    fn close(a: NativeVec3, b: [f32; 3]) -> bool {
        [a.x, a.y, a.z]
            .iter()
            .zip(b)
            .all(|(a, b)| (a - b).abs() < 1e-4)
    }

    #[test]
    fn a_placed_prop_answers_nearest_points_and_rays_on_its_own_triangles() {
        let glb = two_cube_prop();
        let mut bridge = RuntimeAppearanceBridge::new(
            RuntimeAppearanceCatalog::default(),
            BTreeMap::from([("prop.glb".to_owned(), Arc::from(glb.as_slice()))]),
        );
        bridge.begin_call();
        let path = "prop.glb";
        let model = bridge
            .open_animated_mesh(&NativeAnimatedMeshResourceRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .unwrap();
        // Moved 10 along x and doubled: the grip cube spans x 9..11, y 1..3,
        // z 1..3; the barrel cube x 9.5..10.5, y 1.5..2.5, z -2.5..-1.5.
        let placement = NativeTransform {
            translation: vec3(10.0, 0.0, 0.0),
            rotation: NativeQuat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            scale: vec3(2.0, 2.0, 2.0),
        };
        let context: *mut c_void = (&mut bridge as *mut RuntimeAppearanceBridge).cast();
        let mut receipt = unsafe { std::mem::zeroed::<NativeOperationErrorReceipt>() };

        let points = [
            vec3(10.0, 2.0, 3.5),   // outside the grip, off its +z face
            vec3(10.0, 2.0, 2.2),   // inside the grip, nearest its +z face
            vec3(10.0, 2.0, -2.75), // outside the barrel, off its -z face
            vec3(10.0, 3.0, 2.0),   // on the grip's top
            vec3(10.0, 20.0, 0.0),  // beyond reach
        ];
        let request = NativeModelSurfaceRequest {
            model,
            placement,
            points: points.as_ptr(),
            points_len: points.len(),
            max_distance: 2.0,
        };
        let mut result = NativeModelSurfacePointsResult {
            points: std::ptr::null(),
            points_len: 0,
        };
        assert_eq!(
            unsafe { query_model_surface(context, &request, &mut result, &mut receipt) },
            ABI_OK
        );
        let found = unsafe { std::slice::from_raw_parts(result.points, result.points_len) };
        assert_eq!(found.len(), 5);
        let [outside, inside, barrel, on, beyond] = found else {
            unreachable!()
        };
        assert!(outside.found && !outside.inside, "{outside:?}");
        assert!(close(outside.point, [10.0, 2.0, 3.0]) && close(outside.normal, [0.0, 0.0, 1.0]));
        assert!((outside.distance - 0.5).abs() < 1e-4);
        assert!(inside.found && inside.inside, "{inside:?}");
        assert!(close(inside.point, [10.0, 2.0, 3.0]) && (inside.distance - 0.8).abs() < 1e-4);
        assert!(barrel.found && !barrel.inside, "{barrel:?}");
        assert!(close(barrel.point, [10.0, 2.0, -2.5]) && close(barrel.normal, [0.0, 0.0, -1.0]));
        assert!((barrel.distance - 0.25).abs() < 1e-4);
        assert!(on.found && on.distance < 1e-4, "{on:?}");
        assert!(!beyond.found, "{beyond:?}");

        let rays = [
            NativeModelRay {
                origin: vec3(10.0, 2.0, 0.0),
                direction: vec3(0.0, 0.0, 3.0),
            },
            NativeModelRay {
                origin: vec3(10.0, 2.0, 0.0),
                direction: vec3(0.0, 0.0, -1.0),
            },
            NativeModelRay {
                origin: vec3(10.0, 2.0, 2.0),
                direction: vec3(0.0, 1.0, 0.0),
            },
            NativeModelRay {
                origin: vec3(10.0, 2.0, 0.0),
                direction: vec3(0.0, 1.0, 0.0),
            },
        ];
        let request = NativeModelRayRequest {
            model,
            placement,
            rays: rays.as_ptr(),
            rays_len: rays.len(),
            max_distance: 10.0,
        };
        let mut result = NativeModelRayHitsResult {
            hits: std::ptr::null(),
            hits_len: 0,
        };
        assert_eq!(
            unsafe { cast_model_rays(context, &request, &mut result, &mut receipt) },
            ABI_OK
        );
        let hits = unsafe { std::slice::from_raw_parts(result.hits, result.hits_len) };
        let [grip, barrel, from_inside, miss] = hits else {
            panic!("{hits:?}")
        };
        assert!(grip.hit && !grip.back_face, "{grip:?}");
        assert!(close(grip.point, [10.0, 2.0, 1.0]) && close(grip.normal, [0.0, 0.0, -1.0]));
        assert!((grip.distance - 1.0).abs() < 1e-4);
        assert!(
            barrel.hit && close(barrel.point, [10.0, 2.0, -1.5]),
            "{barrel:?}"
        );
        assert!(close(barrel.normal, [0.0, 0.0, 1.0]) && (barrel.distance - 1.5).abs() < 1e-4);
        assert!(from_inside.hit && from_inside.back_face, "{from_inside:?}");
        assert!(close(from_inside.point, [10.0, 3.0, 2.0]));
        assert!(!miss.hit, "{miss:?}");

        // Only a GLB model answers.
        let request = NativeModelSurfaceRequest {
            model: NativeRenderResourceHandle { value: 9_999 },
            ..NativeModelSurfaceRequest {
                model,
                placement,
                points: points.as_ptr(),
                points_len: points.len(),
                max_distance: 2.0,
            }
        };
        let mut result = NativeModelSurfacePointsResult {
            points: std::ptr::null(),
            points_len: 0,
        };
        assert_eq!(
            unsafe { query_model_surface(context, &request, &mut result, &mut receipt) },
            0
        );
        bridge.end_call();
    }
}
