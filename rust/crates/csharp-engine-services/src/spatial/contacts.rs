//! Contact readouts (`OverlapContacts`, `SweepContacts`): every entity
//! collider, posed entity part and world source a query shape overlaps, or
//! meets as it moves and turns between two poses, in one ordered list.

use super::*;
use engine_spatial::{ContactPose, ContactShape, ContactSource, ContactTarget};

fn contact_error(message: impl Into<String>) -> CsharpEngineServicesError {
    spatial_error("CSHARP_SPATIAL_CONTACT", message)
}

fn shape(value: NativeSpatialShape) -> ContactShape {
    match value.kind {
        NativeSpatialShapeKind::Box => ContactShape::Box {
            center: native_array(value.center),
            half_extents: native_array(value.half_extents),
        },
        NativeSpatialShapeKind::Capsule => ContactShape::Capsule {
            a: native_array(value.a),
            b: native_array(value.b),
            radius: value.radius,
        },
        NativeSpatialShapeKind::Sphere => ContactShape::Sphere {
            center: native_array(value.center),
            radius: value.radius,
        },
        NativeSpatialShapeKind::Segment => ContactShape::Segment {
            a: native_array(value.a),
            b: native_array(value.b),
        },
    }
}

fn pose(value: NativeSpatialPose) -> ContactPose {
    let NativeQuat { x, y, z, w } = value.rotation;
    let rotation = if [x, y, z, w] == [0.0; 4] {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [x, y, z, w].map(f64::from)
    };
    ContactPose {
        translation: native_array(value.translation),
        rotation,
    }
}

fn part_filter_matches(value: &NativeSpatialEntityPart, filter: NativeSpatialQueryFilter) -> bool {
    filter_matches(
        NativeSpatialEntityCollider {
            collision_group: value.collision_group,
            collision_mask: value.collision_mask,
            ..Default::default()
        },
        filter,
    )
}

/// The request's colliders and parts as contact targets, after its filter,
/// its ignored entities and the enabled and trigger rules.
fn targets(
    request: &NativeSpatialContactQueryRequest,
) -> Result<Vec<ContactTarget>, CsharpEngineServicesError> {
    let entities = unsafe {
        borrowed_slice(
            request.entities,
            request.entities_len,
            "spatial query entities",
        )
    }?;
    let parts = unsafe { borrowed_slice(request.parts, request.parts_len, "spatial parts") }?;
    let ignored = unsafe {
        borrowed_slice(
            request.ignored_entities,
            request.ignored_entities_len,
            "spatial ignored entities",
        )
    }?;
    let ignored = ignored_set(ignored)?;
    let colliders = filtered_entities(entities, request.filter, &ignored)
        .into_iter()
        .filter(|value| value.enabled && (request.include_triggers || !value.trigger))
        .map(|value| {
            let (min, max) = collider_bounds(value);
            ContactTarget {
                entity: value.entity,
                part: 0,
                holder: 0,
                shape: ContactShape::Box {
                    center: [0.0; 3],
                    half_extents: [0, 1, 2].map(|axis| (max[axis] - min[axis]).abs() * 0.5),
                },
                pose: ContactPose::at([0, 1, 2].map(|axis| (min[axis] + max[axis]) * 0.5)),
            }
        });
    let ignores = |entity: u64| entity != 0 && ignored.contains(&EntityId::new(entity));
    let parts = parts
        .iter()
        .filter(|value| {
            value.enabled
                && part_filter_matches(value, request.filter)
                && !ignores(value.entity)
                && !ignores(value.holder)
        })
        .map(|value| ContactTarget {
            entity: value.entity,
            part: value.part,
            holder: value.holder,
            shape: shape(value.shape),
            pose: pose(value.pose),
        });
    Ok(colliders.chain(parts).collect())
}

fn native_contact(contact: &ShapeContact) -> NativeSpatialContact {
    let mut native = NativeSpatialContact {
        point: native_f64_vec3(contact.point),
        normal: native_f64_vec3(contact.normal),
        fraction: contact.fraction,
        penetration_depth: contact.penetration_depth,
        start_solid: contact.start_solid,
        ..Default::default()
    };
    match contact.source {
        ContactSource::Voxel => native.kind = NativeSpatialHitKind::Voxel,
        ContactSource::StaticMesh {
            instance,
            asset,
            geometry_hash,
        } => {
            native.kind = NativeSpatialHitKind::StaticMesh;
            native.instance = instance.0;
            native.asset = asset.0;
            native.geometry_hash = geometry_hash;
        }
        ContactSource::Entity {
            entity,
            part,
            holder,
        } => {
            native.kind = NativeSpatialHitKind::Entity;
            native.entity = entity;
            native.part = part;
            native.holder = holder;
        }
    }
    native
}

impl RuntimeSpatialBridge {
    pub(super) fn contact_query(
        &mut self,
        request: &NativeSpatialContactQueryRequest,
        sweep: bool,
    ) -> Result<NativeSpatialContactResult, CsharpEngineServicesError> {
        let targets = targets(request)?;
        let scene = self.session_mut(request.session)?.scene.clone();
        let (query, world) = (shape(request.shape), !request.exclude_world);
        let contacts = if sweep {
            scene.sweep_contacts(
                query,
                pose(request.start),
                pose(request.end),
                native_array(request.pivot),
                &targets,
                world,
            )
        } else {
            scene.overlap_contacts(query, pose(request.start), &targets, world)
        }
        .map_err(|error| contact_error(error.to_string()))?;
        let world_contact = contacts
            .iter()
            .any(|contact| !matches!(contact.source, ContactSource::Entity { .. }));
        let keep = match request.max_contacts {
            0 => contacts.len(),
            bound => contacts.len().min(bound as usize),
        };
        let readout = contacts[..keep]
            .iter()
            .map(native_contact)
            .collect::<Box<[_]>>();
        let result = NativeSpatialContactResult {
            contacts: readout.as_ptr(),
            contacts_len: readout.len(),
            world_contact,
            truncated: keep < contacts.len(),
            projection_version: scene.projection_version(),
            source_revision: scene.source_revision().raw(),
        };
        self.borrowed.hold(readout);
        Ok(result)
    }
}

unsafe fn contact_call(
    context: *mut c_void,
    request: *const NativeSpatialContactQueryRequest,
    result: *mut NativeSpatialContactResult,
    error: *mut NativeOperationErrorReceipt,
    sweep: bool,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeSpatialBridge>() }
        .contact_query(unsafe { &*request }, sweep)
    {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}

pub(super) unsafe extern "C" fn overlap_contacts(
    context: *mut c_void,
    request: *const NativeSpatialContactQueryRequest,
    result: *mut NativeSpatialContactResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { contact_call(context, request, result, error, false) }
}

pub(super) unsafe extern "C" fn sweep_contacts(
    context: *mut c_void,
    request: *const NativeSpatialContactQueryRequest,
    result: *mut NativeSpatialContactResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { contact_call(context, request, result, error, true) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32, z: f32) -> NativeVec3 {
        NativeVec3 { x, y, z }
    }

    fn cube(entity: u64, x: f32, z: f32) -> NativeSpatialEntityCollider {
        NativeSpatialEntityCollider {
            entity,
            min: v(x - 0.25, 1.0, z - 0.25),
            max: v(x + 0.25, 2.0, z + 0.25),
            enabled: true,
            ..Default::default()
        }
    }

    fn sphere(radius: f64) -> NativeSpatialShape {
        NativeSpatialShape {
            kind: NativeSpatialShapeKind::Sphere,
            radius,
            ..Default::default()
        }
    }

    fn at(x: f32, y: f32, z: f32) -> NativeSpatialPose {
        NativeSpatialPose {
            translation: v(x, y, z),
            ..Default::default()
        }
    }

    struct Query<'a> {
        entities: &'a [NativeSpatialEntityCollider],
        parts: &'a [NativeSpatialEntityPart],
        ignored: &'a [u64],
    }

    impl Query<'_> {
        fn request(
            &self,
            session: NativeSpatialSessionHandle,
            shape: NativeSpatialShape,
            start: NativeSpatialPose,
        ) -> NativeSpatialContactQueryRequest {
            NativeSpatialContactQueryRequest {
                session,
                shape,
                start,
                end: start,
                pivot: NativeVec3::default(),
                filter: NativeSpatialQueryFilter::default(),
                entities: self.entities.as_ptr(),
                entities_len: self.entities.len(),
                parts: self.parts.as_ptr(),
                parts_len: self.parts.len(),
                ignored_entities: self.ignored.as_ptr(),
                ignored_entities_len: self.ignored.len(),
                include_triggers: false,
                exclude_world: false,
                max_contacts: 0,
            }
        }
    }

    /// Copies a borrowed result before the next call on the bridge.
    fn read(
        bridge: &mut RuntimeSpatialBridge,
        request: &NativeSpatialContactQueryRequest,
        sweep: bool,
    ) -> (Vec<NativeSpatialContact>, bool, bool) {
        let result = bridge.contact_query(request, sweep).unwrap();
        let contacts =
            unsafe { std::slice::from_raw_parts(result.contacts, result.contacts_len) }.to_vec();
        (contacts, result.world_contact, result.truncated)
    }

    fn named(contacts: &[NativeSpatialContact]) -> Vec<(NativeSpatialHitKind, u64, u32)> {
        contacts
            .iter()
            .map(|contact| (contact.kind, contact.entity, contact.part))
            .collect()
    }

    #[test]
    fn an_overlap_lists_every_entity_it_touches_with_the_world_after_filters_and_ignores() {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = {
            let api = api(&mut bridge);
            let mut session = NativeSpatialSessionHandle::default();
            unsafe {
                (api.create_session)(
                    api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                    std::ptr::null_mut(),
                )
            };
            session
        };
        // One solid voxel, from (1, 1, 1) to (2, 2, 2).
        bridge.sessions.get_mut(&session.value).unwrap().scene =
            Arc::new(VoxelCollisionScene::from_solid_voxels(1.0, 8, [[1, 1, 1]]).unwrap());
        let mut filtered = cube(11, 0.0, 0.6);
        filtered.collision_group = 0b10;
        let mut disabled = cube(13, 0.0, -0.6);
        disabled.enabled = false;
        let mut trigger = cube(15, -0.6, 0.0);
        trigger.trigger = true;
        let entities = [
            cube(7, 0.6, 0.0),
            cube(3, -0.3, 0.3),
            cube(9, 0.3, -0.3),
            cube(5, 0.0, 0.0),
            cube(21, 4.0, 0.0),
            filtered,
            disabled,
            trigger,
        ];
        // A shield held by the ignored entity 9, and a hand part of 5.
        let held = NativeSpatialEntityPart {
            entity: 30,
            holder: 9,
            shape: sphere(0.1),
            pose: at(0.0, 1.5, 0.5),
            enabled: true,
            ..Default::default()
        };
        let hand = NativeSpatialEntityPart {
            entity: 5,
            part: 4,
            shape: NativeSpatialShape {
                kind: NativeSpatialShapeKind::Box,
                half_extents: v(0.05, 0.05, 0.05),
                ..Default::default()
            },
            pose: at(-0.5, 1.5, 0.0),
            enabled: true,
            ..Default::default()
        };
        let parts = [held, hand];
        let query = Query {
            entities: &entities,
            parts: &parts,
            ignored: &[9],
        };
        let mut request = query.request(session, sphere(1.5), at(0.0, 1.5, 0.0));
        request.filter = NativeSpatialQueryFilter {
            collision_group: 0b1,
            collision_mask: 0b1,
        };
        let (contacts, world, truncated) = read(&mut bridge, &request, false);
        use NativeSpatialHitKind::{Entity, Voxel};
        assert_eq!(
            named(&contacts),
            vec![
                (Voxel, 0, 0),
                (Entity, 3, 0),
                (Entity, 5, 0),
                (Entity, 5, 4),
                (Entity, 7, 0)
            ]
        );
        assert!(world && !truncated);
        assert!(contacts
            .iter()
            .all(|contact| contact.penetration_depth > 0.0));

        request.include_triggers = true;
        request.exclude_world = true;
        request.max_contacts = 3;
        let (contacts, world, truncated) = read(&mut bridge, &request, false);
        assert_eq!(
            named(&contacts),
            vec![(Entity, 3, 0), (Entity, 5, 0), (Entity, 5, 4)]
        );
        assert!(!world && truncated);
        request.max_contacts = 0;
        let (contacts, _, _) = read(&mut bridge, &request, false);
        assert!(named(&contacts).contains(&(Entity, 15, 0)));

        // An AABB volume is a box shape with no rotation.
        let mut box_request = query.request(
            session,
            NativeSpatialShape {
                kind: NativeSpatialShapeKind::Box,
                half_extents: v(0.1, 0.1, 0.1),
                ..Default::default()
            },
            at(0.6, 1.5, 0.0),
        );
        box_request.filter = request.filter;
        let (contacts, world, _) = read(&mut bridge, &box_request, false);
        assert_eq!(named(&contacts), vec![(Entity, 7, 0)]);
        assert!(!world);
    }

    #[test]
    fn a_sweep_orders_a_held_item_before_its_holder_and_reports_the_part() {
        let mut bridge = RuntimeSpatialBridge::new();
        let session = {
            let api = api(&mut bridge);
            let mut session = NativeSpatialSessionHandle::default();
            unsafe {
                (api.create_session)(
                    api.context,
                    NativeSpatialSessionConfig {
                        collision_voxel_size: 1.0,
                        collision_chunk_size: 8,
                        voxel_surface_mode: NativeVoxelSurfaceMode::GreedyCubes,
                    },
                    &mut session,
                    std::ptr::null_mut(),
                )
            };
            session
        };
        let body = NativeSpatialEntityPart {
            entity: 40,
            part: 2,
            shape: NativeSpatialShape {
                kind: NativeSpatialShapeKind::Capsule,
                a: v(0.0, -0.5, 0.0),
                b: v(0.0, 0.5, 0.0),
                radius: 0.3,
                ..Default::default()
            },
            pose: at(3.0, 0.0, 0.0),
            enabled: true,
            ..Default::default()
        };
        let shield = NativeSpatialEntityPart {
            entity: 41,
            part: 0,
            holder: 40,
            shape: NativeSpatialShape {
                kind: NativeSpatialShapeKind::Box,
                half_extents: v(0.05, 0.5, 0.5),
                ..Default::default()
            },
            pose: at(2.4, 0.0, 0.0),
            enabled: true,
            ..Default::default()
        };
        let parts = [body, shield];
        let query = Query {
            entities: &[],
            parts: &parts,
            ignored: &[],
        };
        let blade = NativeSpatialShape {
            kind: NativeSpatialShapeKind::Segment,
            a: v(0.0, 0.0, 0.0),
            b: v(0.0, 0.0, 0.0),
            ..Default::default()
        };
        let mut request = query.request(
            session,
            NativeSpatialShape {
                b: v(0.5, 0.0, 0.0),
                ..blade
            },
            at(0.0, 0.0, 0.0),
        );
        request.end = at(4.0, 0.0, 0.0);
        let (contacts, _, _) = read(&mut bridge, &request, true);
        let order: Vec<_> = contacts
            .iter()
            .map(|contact| (contact.entity, contact.part, contact.holder))
            .collect();
        assert_eq!(order, vec![(41, 0, 40), (40, 2, 0)]);
        assert!(contacts[0].fraction < contacts[1].fraction);
        assert!((contacts[0].point.x - 2.35).abs() < 1e-3);
        assert!((contacts[0].normal.x + 1.0).abs() < 1e-3);
        // Ignoring the holder ignores what it holds.
        let ignored = [40];
        request.ignored_entities = ignored.as_ptr();
        request.ignored_entities_len = 1;
        let (contacts, _, _) = read(&mut bridge, &request, true);
        assert!(contacts.is_empty());
    }
}
