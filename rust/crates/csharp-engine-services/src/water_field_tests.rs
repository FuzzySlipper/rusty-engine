use super::*;
use crate::composition::{parse_runtime_appearance_catalog, EngineServiceSet};
use crate::operation_diagnostics::{empty_receipt, receipt_codes};
use runtime_diagnostics::RuntimeDiagnosticsSink;
use runtime_lifecycle::{RuntimeControlRevision, RuntimeGeneration, RuntimeInstanceId};
use runtime_ui::RuntimeUiRuntimeBinding;
use std::time::{Duration, Instant};

const STEP_HZ: u32 = 30;
const STEPS_PER_CALL: u32 = 4;
const WORKER_PATIENCE: Duration = Duration::from_secs(120);
const UNBOUNDED_BACKLOG_SECONDS: f32 = 1.0e6;

fn binding() -> RuntimeUiRuntimeBinding {
    RuntimeUiRuntimeBinding::new(
        RuntimeInstanceId::new(1),
        RuntimeGeneration::new(1),
        RuntimeControlRevision::new(1),
    )
}

fn services() -> EngineServiceSet {
    EngineServiceSet::new(
        parse_runtime_appearance_catalog(None).expect("default catalog"),
        BTreeMap::new(),
        None,
        RuntimeDiagnosticsSink::new(Default::default()).unwrap(),
    )
    .expect("service set")
}

fn update(step: u64, steps: u32) -> NativeProductUpdateFacts {
    NativeProductUpdateFacts {
        lifecycle_state: NativeProductLifecycleState::Running,
        generation: 1,
        control_revision: 1,
        observed_host_time_nanoseconds: step * 1_000_000,
        simulation_step: step,
        fixed_step_hz: STEP_HZ,
        admitted_step_count: steps,
        dropped_step_count: 0,
        fixed_delta_seconds: 1.0 / f64::from(STEP_HZ),
        gameplay_time_selected: false,
        gameplay_rate: 1.0,
        gameplay_advance_remaining_steps: 0,
        host_elapsed_seconds: 0.0,
    }
}

/// The valley of the kernel's storm scene, in 4 m cells: a channel down the
/// middle, banks, and a floodplain sloping back to it.
const WIDTH: u32 = 48;
const LENGTH: u32 = 24;
const CELL: f32 = 4.0;
const CHANNEL: std::ops::Range<u32> = 10..14;
const CHANNEL_DEPTH: f32 = 1.5;
const BASE_FLOW: f32 = 5.0;
const STORM_FLOW: f32 = 80.0;

fn valley_bed() -> Vec<f32> {
    (0..LENGTH)
        .flat_map(|z| (0..WIDTH).map(move |x| (x, z)))
        .map(|(x, z)| {
            let valley = 0.002 * (WIDTH - x) as f32 * CELL;
            if CHANNEL.contains(&z) {
                return valley;
            }
            let from_bank = if z < CHANNEL.start {
                CHANNEL.start - z
            } else {
                z + 1 - CHANNEL.end
            };
            valley + CHANNEL_DEPTH + 0.01 * from_bank as f32 * CELL
        })
        .collect()
}

struct Valley {
    services: EngineServiceSet,
    field: NativeWaterFieldHandle,
    step: u64,
}

impl Valley {
    fn new() -> Self {
        let mut services = services();
        services.begin_call(binding());
        let api = services.api();
        let bed = valley_bed();
        let request = NativeWaterFieldCreateRequest {
            width: WIDTH,
            length: LENGTH,
            cell_size: CELL,
            bed: bed.as_ptr(),
            bed_len: bed.len(),
            depth: std::ptr::null(),
            depth_len: 0,
            session: NativeOptionalSpatialSession::default(),
            anchor: NativeWorldOriginGlobalPosition::default(),
            origin: NativeVec3 {
                x: 100.0,
                y: 10.0,
                z: -50.0,
            },
            // These tests admit time far faster than the wall clock; a
            // debug build on a slow runner would owe more work than the
            // default backlog keeps. The backlog has its own test.
            policy: NativeWaterFieldPolicy {
                max_backlog_seconds: UNBOUNDED_BACKLOG_SECONDS,
                ..NativeWaterFieldPolicy::default()
            },
        };
        let mut field = NativeWaterFieldHandle::default();
        let water = &api.water_field;
        assert_eq!(
            unsafe { (water.create)(water.context, &request, &mut field, std::ptr::null_mut()) },
            ABI_OK
        );
        let mut readout = NativeWaterFieldReadout::default();
        let open = NativeWaterFieldBoundaryRequest {
            field,
            side: NativeWaterFieldSide::XMax,
            first: 0,
            count: 0,
            kind: NativeWaterFieldBoundaryKind::Open,
            value: 0.0,
        };
        assert_eq!(
            unsafe {
                (water.set_boundary)(water.context, open, &mut readout, std::ptr::null_mut())
            },
            ABI_OK
        );
        services.finish_call().expect("create call");
        let mut valley = Self {
            services,
            field,
            step: 0,
        };
        valley.inflow(BASE_FLOW);
        valley
    }

    /// Runs `operation` in an ordinary call with no admitted time.
    fn call<T>(
        &mut self,
        operation: impl FnOnce(&NativeWaterFieldApi, NativeWaterFieldHandle) -> T,
    ) -> T {
        self.services.begin_call(binding());
        let api = self.services.api();
        let value = operation(&api.water_field, self.field);
        self.services.finish_call().expect("call finishes");
        value
    }

    fn inflow(&mut self, discharge: f32) {
        self.call(|water, field| {
            let request = NativeWaterFieldBoundaryRequest {
                field,
                side: NativeWaterFieldSide::XMin,
                first: CHANNEL.start,
                count: CHANNEL.end - CHANNEL.start,
                kind: NativeWaterFieldBoundaryKind::Inflow,
                value: discharge,
            };
            let mut readout = NativeWaterFieldReadout::default();
            assert_eq!(
                unsafe {
                    (water.set_boundary)(water.context, request, &mut readout, std::ptr::null_mut())
                },
                ABI_OK
            );
        });
    }

    fn read(&mut self) -> NativeWaterFieldReadout {
        self.call(read)
    }

    /// Admits `seconds` of updates, `STEPS_PER_CALL` steps a call, then waits
    /// for the worker to publish all of it.
    fn run(&mut self, seconds: f64) {
        let calls = (seconds * f64::from(STEP_HZ) / f64::from(STEPS_PER_CALL)).round() as u64;
        for _ in 0..calls {
            self.step += u64::from(STEPS_PER_CALL);
            self.services
                .begin_update_call(binding(), update(self.step, STEPS_PER_CALL));
            self.services.finish_call().expect("update finishes");
        }
        self.settle();
    }

    /// Waits until the latest snapshot covers the admitted time.
    fn settle(&mut self) -> NativeWaterFieldReadout {
        let started = Instant::now();
        loop {
            let readout = self.read();
            if readout.age < 1.0e-6 && readout.snapshot_incarnation == readout.incarnation {
                return readout;
            }
            assert!(
                started.elapsed() < WORKER_PATIENCE,
                "the worker fell behind: {readout:?}"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn sample(&mut self, points: &[NativeWaterFieldPoint]) -> Vec<NativeWaterFieldSample> {
        self.call(|water, field| {
            let request = NativeWaterFieldSampleRequest {
                field,
                points: points.as_ptr(),
                points_len: points.len(),
            };
            let mut result = NativeWaterFieldSamplesResult {
                samples: std::ptr::null(),
                samples_len: 0,
            };
            assert_eq!(
                unsafe {
                    (water.sample)(water.context, &request, &mut result, std::ptr::null_mut())
                },
                ABI_OK
            );
            unsafe { std::slice::from_raw_parts(result.samples, result.samples_len) }.to_vec()
        })
    }

    /// Cells of the floodplain deeper than `depth`.
    fn floodplain_wet(&mut self, depth: f32) -> usize {
        let cells = self.call(|water, field| {
            let mut result = NativeWaterFieldCellsResult {
                cells: std::ptr::null(),
                cells_len: 0,
            };
            assert_eq!(
                unsafe {
                    (water.read_cells)(water.context, field, &mut result, std::ptr::null_mut())
                },
                ABI_OK
            );
            unsafe { std::slice::from_raw_parts(result.cells, result.cells_len) }.to_vec()
        });
        cells
            .iter()
            .enumerate()
            .filter(|(i, _)| !CHANNEL.contains(&(*i as u32 / WIDTH)))
            .filter(|(_, cell)| cell.depth > depth)
            .count()
    }
}

fn read(water: &NativeWaterFieldApi, field: NativeWaterFieldHandle) -> NativeWaterFieldReadout {
    let mut readout = NativeWaterFieldReadout::default();
    assert_eq!(
        unsafe { (water.read)(water.context, field, &mut readout, std::ptr::null_mut()) },
        ABI_OK
    );
    readout
}

/// A point of the scene over cell (x, z)'s centre, for the valley's origin.
fn over(x: u32, z: u32) -> NativeWaterFieldPoint {
    NativeWaterFieldPoint {
        x: 100.0 + (x as f32 + 0.5) * CELL,
        z: -50.0 + (z as f32 + 0.5) * CELL,
    }
}

#[test]
fn fields_advance_only_by_admitted_time() {
    let mut valley = Valley::new();
    valley.run(2.0);
    let readout = valley.settle();
    assert!((readout.time - 2.0).abs() < 1.0e-6, "{readout:?}");
    assert!((readout.admitted_time - 2.0).abs() < 1.0e-9);
    assert!(readout.snapshots >= 1);
    // The inflow is the discharge over the admitted time, however it is
    // spread over the channel's edge cells.
    let expected = f64::from(BASE_FLOW) * readout.time;
    assert!(
        (readout.boundary_inflow - expected).abs() < 1.0e-3 * expected,
        "{} m³ entered, {expected} m³ expected",
        readout.boundary_inflow
    );

    // A held update admits no steps; calls without an update admit none.
    for _ in 0..5 {
        valley
            .services
            .begin_update_call(binding(), update(valley.step, 0));
        valley.services.finish_call().unwrap();
    }
    std::thread::sleep(Duration::from_millis(20));
    let held = valley.read();
    assert_eq!(held.time, readout.time);
    assert_eq!(held.snapshots, readout.snapshots);
}

#[test]
fn a_storm_overbanks_and_drains_back_through_the_service() {
    let mut valley = Valley::new();
    valley.run(300.0);
    assert_eq!(valley.floodplain_wet(0.01), 0, "base flow left the channel");
    let channel = valley.sample(&[over(20, 11)])[0];
    assert!(channel.inside && channel.wet, "{channel:?}");
    assert!(
        channel.velocity_x > 0.1,
        "the river runs downstream: {channel:?}"
    );
    // Heights come back in the scene: the origin lifts the field 10 units.
    assert!(channel.bed > 10.0 && channel.surface > channel.bed);

    valley.inflow(STORM_FLOW);
    valley.run(300.0);
    let flooded = valley.floodplain_wet(0.05);
    assert!(
        flooded > (WIDTH * 4) as usize,
        "the storm wetted {flooded} floodplain cells"
    );
    let plain = valley.sample(&[over(20, 6)])[0];
    assert!(plain.wet && plain.depth > 0.05, "{plain:?}");

    valley.inflow(BASE_FLOW);
    valley.run(900.0);
    assert_eq!(valley.floodplain_wet(0.05), 0, "the floodplain kept water");
    let outside = valley.sample(&[NativeWaterFieldPoint { x: 0.0, z: 0.0 }])[0];
    assert!(!outside.inside && !outside.wet);
}

#[test]
fn a_dam_is_a_bed_edit_that_backs_water_up() {
    const DAM_X: u32 = 30;
    let mut valley = Valley::new();
    valley.run(300.0);
    let behind = over(DAM_X - 1, 11);
    let before = valley.sample(&[behind])[0];
    let edited = valley.call(|water, field| {
        let cells: Vec<NativeWaterFieldCellValue> = (0..LENGTH)
            .map(|z| NativeWaterFieldCellValue {
                x: DAM_X,
                z,
                value: valley_bed()[(z * WIDTH + DAM_X) as usize] + 4.0,
            })
            .collect();
        let request = NativeWaterFieldBedRequest {
            field,
            cells: cells.as_ptr(),
            cells_len: cells.len(),
        };
        let mut readout = NativeWaterFieldReadout::default();
        assert_eq!(
            unsafe {
                (water.edit_bed)(water.context, &request, &mut readout, std::ptr::null_mut())
            },
            ABI_OK
        );
        readout
    });
    assert_eq!(edited.incarnation, 1);
    assert_eq!(
        edited.snapshot_incarnation, 0,
        "the edit waits for the worker"
    );
    valley.run(120.0);
    let readout = valley.settle();
    assert_eq!(readout.snapshot_incarnation, 1);
    let after = valley.sample(&[behind])[0];
    assert!(
        after.surface > before.surface + 0.3,
        "behind the dam: {before:?} then {after:?}"
    );
}

#[test]
fn work_beyond_the_backlog_drops_admitted_time_and_reports_it() {
    let mut valley = Valley::new();
    // A first advance measures what a simulated second costs.
    valley.run(1.0);
    valley.call(|water, field| {
        let request = NativeWaterFieldPolicyRequest {
            field,
            policy: NativeWaterFieldPolicy {
                max_backlog_seconds: 1.0e-7,
                ..NativeWaterFieldPolicy::default()
            },
        };
        let mut readout = NativeWaterFieldReadout::default();
        assert_eq!(
            unsafe {
                (water.set_policy)(water.context, request, &mut readout, std::ptr::null_mut())
            },
            ABI_OK
        );
    });
    let before = valley.read();
    // One update admits 3 s at once: far more work than the backlog allows.
    valley.step += 90;
    valley
        .services
        .begin_update_call(binding(), update(valley.step, 90));
    valley.services.finish_call().unwrap();
    // Until the worker has run what was kept, the policy change's snapshot
    // or a batch in flight may be the latest; wait for nothing to be pending.
    let started = Instant::now();
    let readout = loop {
        let readout = valley.read();
        if readout.pending_seconds < 1.0e-6 {
            break readout;
        }
        assert!(
            started.elapsed() < WORKER_PATIENCE,
            "the worker fell behind: {readout:?}"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    assert!(readout.dropped_seconds > 2.9, "{readout:?}");
    assert!(readout.time - before.time < 0.01, "{readout:?}");
    assert!((readout.time + readout.dropped_seconds - readout.admitted_time).abs() < 1.0e-6);
}

/// Time the worker has taken from the queue is pending until its snapshot is
/// read: every admitted second is in the snapshot, dropped or pending.
#[test]
fn time_the_worker_has_taken_is_still_pending() {
    let mut valley = Valley::new();
    valley.run(1.0);
    let bridge = |water: &NativeWaterFieldApi| unsafe {
        &mut *water.context.cast::<RuntimeWaterFieldBridge>()
    };
    // Without a worker, take the next admission as the worker takes a batch.
    valley.call(|water, _| bridge(water).worker = None);
    valley.step += u64::from(STEPS_PER_CALL);
    valley
        .services
        .begin_update_call(binding(), update(valley.step, STEPS_PER_CALL));
    valley.services.finish_call().unwrap();
    let taken = valley.call(|water, field| {
        let shared = &bridge(water).field(field).unwrap().shared;
        std::mem::take(&mut shared.pending.lock().unwrap().segments)
    });
    assert_eq!(taken.len(), 1);
    let readout = valley.read();
    assert!(
        (readout.pending_seconds - taken[0].seconds).abs() < 1.0e-6,
        "{readout:?}"
    );
    assert!(
        (readout.time + readout.dropped_seconds + readout.pending_seconds - readout.admitted_time)
            .abs()
            < 1.0e-6
    );
}

#[test]
fn a_presented_field_draws_its_wet_cells_and_holds_its_material() {
    let mut valley = Valley::new();
    valley.run(60.0);
    valley.services.begin_call(binding());
    let api = valley.services.api();
    let material = material(&api);
    let water = &api.water_field;
    let request = NativeWaterFieldPresentRequest {
        field: valley.field,
        material,
        layer: NativeRenderLayer::Scene,
        drawn_depth: 0.0,
    };
    let mut readout = NativeWaterFieldReadout::default();
    assert_eq!(
        unsafe { (water.present)(water.context, request, &mut readout, std::ptr::null_mut()) },
        ABI_OK
    );
    let mut call = valley.services.finish_call().unwrap();
    let ops: Vec<_> = call
        .take_output()
        .frames
        .into_iter()
        .flat_map(|frame| frame.ops)
        .collect();
    let defined = ops.iter().find_map(|op| match op {
        RenderDiff::DefineStaticMesh { asset } => Some(asset.clone()),
        _ => None,
    });
    let defined = defined.expect("the surface mesh is defined");
    assert_eq!(
        defined.asset,
        render_projection::water_surface_asset(valley.field.value)
    );
    assert!(defined.payload.layout.index_count > 0);
    assert!(ops.iter().any(|op| matches!(
        op,
        RenderDiff::CreateStaticMeshInstance { instance, .. }
            if instance.transform.translation == [100.0, 10.0, -50.0]
    )));
    assert!(valley.read().drawn_cells > 0);

    // The material is held while the surface draws with it.
    valley.services.begin_call(binding());
    let api = valley.services.api();
    let graphics = &api.graphics;
    let mut receipt = empty_receipt();
    assert_eq!(
        unsafe { (graphics.destroy_material)(graphics.context, material, &mut receipt) },
        0
    );
    assert_eq!(receipt_codes(&receipt), ["CSHARP_MATERIAL_IN_USE"]);
    let water = &api.water_field;
    assert_eq!(
        unsafe { (water.destroy)(water.context, valley.field, std::ptr::null_mut()) },
        ABI_OK
    );
    assert_eq!(
        unsafe { (graphics.destroy_material)(graphics.context, material, std::ptr::null_mut()) },
        ABI_OK
    );
    let mut call = valley.services.finish_call().unwrap();
    let output = call.take_output();
    let removed = output.appearance.iter().any(|item| match item {
        crate::composition::CsharpAppearanceCallOutput::Frame(frame) => frame
            .ops
            .iter()
            .any(|op| matches!(op, RenderDiff::ReleaseStaticMesh { .. })),
        _ => false,
    });
    assert!(removed, "the surface goes ahead of the material's release");
}

#[test]
fn malformed_fields_and_calls_outside_a_product_call_are_refused() {
    let mut services = services();
    services.begin_call(binding());
    let api = services.api();
    let water = &api.water_field;
    let bed = [0.0f32; 3];
    let request = NativeWaterFieldCreateRequest {
        width: 2,
        length: 2,
        cell_size: 1.0,
        bed: bed.as_ptr(),
        bed_len: bed.len(),
        depth: std::ptr::null(),
        depth_len: 0,
        session: NativeOptionalSpatialSession::default(),
        anchor: NativeWorldOriginGlobalPosition::default(),
        origin: NativeVec3::default(),
        policy: NativeWaterFieldPolicy::default(),
    };
    let mut field = NativeWaterFieldHandle::default();
    let mut receipt = empty_receipt();
    assert_eq!(
        unsafe { (water.create)(water.context, &request, &mut field, &mut receipt) },
        0
    );
    assert_eq!(receipt_codes(&receipt), ["CSHARP_WATER_FIELD"]);
    services.finish_call().unwrap();
    let mut readout = NativeWaterFieldReadout::default();
    let mut receipt = empty_receipt();
    assert_eq!(
        unsafe { (water.read)(water.context, field, &mut readout, &mut receipt) },
        0
    );
    assert_eq!(receipt_codes(&receipt), ["CSHARP_WATER_FIELD"]);
}

fn material(api: &NativeEngineApi) -> NativeMaterialHandle {
    let white = NativeColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    let request = NativeMaterialRequest {
        texture_scale: NativeVec2::default(),
        texture_offset: NativeVec2::default(),
        stochastic_tiling: 0.0,
        shader: Default::default(),
        triplanar_sharpness: 0.0,
        color: white,
        texture: NativeRenderResourceReference::default(),
        roughness: 0.1,
        texture_tint: white,
        emission_color: NativeVec3::default(),
        emission_intensity: 0.0,
        double_sided: false,
        alpha_mode: NativeMaterialAlphaMode::Blend,
        alpha_cutoff: 0.5,
        metalness: 0.0,
        normal_map: NativeRenderResourceReference::default(),
        normal_scale: 1.0,
        unlit: false,
        emission_map: NativeRenderResourceReference::default(),
        occlusion_map: NativeRenderResourceReference::default(),
        orm_map: Default::default(),
        occlusion_strength: 0.0,
        flat_shading: false,
        keep_dry: false,
        wind_bend: 0.0,
        wind_flutter: 0.0,
        water: Default::default(),
        translucent_shadow: false,
    };
    let mut handle = NativeMaterialHandle::default();
    let graphics = &api.graphics;
    assert_eq!(
        unsafe {
            (graphics.create_material)(graphics.context, request, &mut handle, std::ptr::null_mut())
        },
        ABI_OK
    );
    handle
}

/// A box named as floating in a Dynamics world whose water is this field
/// floats on the river at its Archimedes draft and drifts downstream.
#[test]
fn dynamics_bodies_float_on_a_water_field() {
    const HALF: f32 = 0.5;
    const MASS: f32 = 300.0;
    const DENSITY: f32 = 1000.0;
    let mut valley = Valley::new();
    valley.run(300.0);
    let start = over(20, 11);
    let river = valley.sample(&[start])[0];
    assert!(river.wet && river.depth > 0.3, "{river:?}");

    valley.services.begin_call(binding());
    let api = valley.services.api();
    let dynamics = &api.dynamics;
    let mut world = NativeDynamicsWorldHandle::default();
    let config = NativeDynamicsWorldConfig {
        gravity: NativeVec3 {
            x: 0.0,
            y: -9.81,
            z: 0.0,
        },
    };
    assert_eq!(
        unsafe {
            (dynamics.create_world)(dynamics.context, config, &mut world, std::ptr::null_mut())
        },
        ABI_OK
    );
    let request = NativeDynamicsCreateBodyRequest {
        world,
        body: NativeDynamicsBodyConfig {
            transform: NativeTransform {
                translation: NativeVec3 {
                    x: start.x,
                    y: river.surface + 0.5,
                    z: start.z,
                },
                rotation: NativeQuat {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                },
                scale: NativeVec3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },
            },
            half_extents: NativeVec3 {
                x: HALF,
                y: HALF,
                z: HALF,
            },
            properties: NativeDynamicsBodyProperties {
                mass: MASS,
                mass_policy: NativeDynamicsMassPolicy::default(),
                linear_velocity: NativeVec3::default(),
                angular_velocity: NativeVec3::default(),
                axis_locks: NativeAxisLocks::default(),
                linear_damping: 0.0,
                angular_damping: 0.0,
                gravity_scale: 1.0,
                friction: 0.5,
                restitution: 0.0,
                collision_groups: u32::MAX,
                collision_mask: u32::MAX,
                enabled: true,
                sleeping: false,
                continuous_collision: false,
            },
        },
    };
    let mut body = NativeDynamicsBodyHandle::default();
    assert_eq!(
        unsafe {
            (dynamics.create_body)(dynamics.context, &request, &mut body, std::ptr::null_mut())
        },
        ABI_OK
    );
    let fields = [NativeDynamicsWaterFieldEntry {
        field: valley.field,
        density: DENSITY,
        linear_drag: 4.0,
        angular_drag: 4.0,
    }];
    let bodies = [body];
    let water = NativeDynamicsWaterRequest {
        world,
        planes: std::ptr::null(),
        planes_len: 0,
        fields: fields.as_ptr(),
        fields_len: fields.len(),
        bodies: bodies.as_ptr(),
        bodies_len: bodies.len(),
    };
    let mut receipt = NativeDynamicsWaterReceipt::default();
    assert_eq!(
        unsafe {
            (dynamics.set_water)(dynamics.context, &water, &mut receipt, std::ptr::null_mut())
        },
        ABI_OK
    );
    assert_eq!((receipt.fields, receipt.bodies), (1, 1));
    valley.services.finish_call().unwrap();

    let read = |valley: &mut Valley| {
        valley.services.begin_call(binding());
        let api = valley.services.api();
        let dynamics = &api.dynamics;
        let mut readout = NativeDynamicsReadout::default();
        assert_eq!(
            unsafe {
                (dynamics.read)(
                    dynamics.context,
                    NativeDynamicsReadRequest { body },
                    &mut readout,
                    std::ptr::null_mut(),
                )
            },
            ABI_OK
        );
        valley.services.finish_call().unwrap();
        readout
    };
    for _ in 0..(10 * STEP_HZ / STEPS_PER_CALL) {
        valley.step += u64::from(STEPS_PER_CALL);
        valley
            .services
            .begin_update_call(binding(), update(valley.step, STEPS_PER_CALL));
        let api = valley.services.api();
        let dynamics = &api.dynamics;
        let step = NativeDynamicsStepRequest {
            world,
            step_seconds: 1.0 / STEP_HZ as f32,
            steps: STEPS_PER_CALL,
            actions: std::ptr::null(),
            actions_len: 0,
        };
        let mut receipt = NativeDynamicsStepReceipt::default();
        assert_eq!(
            unsafe { (dynamics.step)(dynamics.context, &step, &mut receipt, std::ptr::null_mut()) },
            ABI_OK
        );
        valley.services.finish_call().unwrap();
    }
    let readout = read(&mut valley);
    let at = readout.transform.translation;
    let surface = valley.sample(&[NativeWaterFieldPoint { x: at.x, z: at.z }])[0].surface;
    let draft = surface - (at.y - HALF);
    let expected = MASS / (DENSITY * 4.0 * HALF * HALF);
    assert!(readout.water.floating, "{readout:?}");
    assert!(
        (draft - expected).abs() < 0.05,
        "draft {draft}, Archimedes {expected}"
    );
    assert!(
        at.x > start.x + 2.0,
        "the river carries the box downstream: {at:?}"
    );
    assert!(
        readout.water.submerged_fraction > 0.2,
        "{:?}",
        readout.water
    );
}
