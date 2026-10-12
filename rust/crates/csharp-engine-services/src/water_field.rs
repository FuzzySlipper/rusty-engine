//! Water fields (#9884): regions of column water the product describes (bed,
//! edges, sources) and the Engine advances. Each field's grid
//! (`svc_water::WaterGrid`) lives on one worker thread the bridge owns. As a
//! call finishes, the bridge hands every field the simulated time the
//! lifecycle admitted for it, so fields pause with a gameplay hold and slow
//! with slow motion and never run from wall time. The worker steps through
//! that time and publishes a complete snapshot; the next call reads the
//! latest one without waiting. Drawn surfaces are projected by
//! `render_projection::WaterSurfaceProjector`.

use std::{collections::BTreeMap, ffi::c_void, sync::Arc};

use csharp_engine_abi::*;
use render_model::{RenderDiff, RenderFrameDiff, RenderLayer};
use render_projection::{
    water_surface_mesh, WaterSurfaceCells, WaterSurfacePlacement, WaterSurfaceProjector,
};
use svc_water::{Boundary, FlowParams, Side, WaterGrid};

use crate::{
    appearance::RuntimeAppearanceBridge,
    composition::{borrowed_slice, ABI_OK},
    operation_diagnostics::{clear_receipt, refuse, BorrowedResult, OperationDiagnostics},
    spatial::SpatialCollisionSource,
    CsharpEngineServicesError,
};

const DEFAULT_COURANT: f32 = svc_water::DEFAULT_COURANT;
const DEFAULT_THREADS: usize = 1;
const DEFAULT_MAX_BACKLOG_SECONDS: f64 = 1.0;
const DEFAULT_DRAWN_DEPTH: f32 = 0.01;
/// Admitted time a field may leave unadvanced at the end of a batch: the
/// last step lands on the admitted boundary to within this.
pub(crate) const TIME_EPSILON: f64 = 1.0e-9;

pub(crate) fn refused(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_WATER_FIELD", message)
}

/// A field's advancement policy, defaults resolved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Policy {
    courant: f32,
    max_step: f32,
    threads: usize,
    max_backlog: f64,
    roughness: f32,
}

impl Policy {
    pub(crate) fn of(policy: NativeWaterFieldPolicy) -> Result<Self, CsharpEngineServicesError> {
        let NativeWaterFieldPolicy {
            courant,
            max_step_seconds,
            threads,
            max_backlog_seconds,
            roughness,
        } = policy;
        let finite = [courant, max_step_seconds, max_backlog_seconds, roughness]
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0);
        if !finite || courant > 1.0 {
            return Err(refused(
                "a water field's policy is finite and not negative, with a Courant number of at most 1",
            ));
        }
        let or = |value: f32, default: f32| if value == 0.0 { default } else { value };
        Ok(Self {
            courant: or(courant, DEFAULT_COURANT),
            max_step: or(max_step_seconds, f32::INFINITY),
            threads: if threads == 0 {
                DEFAULT_THREADS
            } else {
                threads as usize
            },
            max_backlog: if max_backlog_seconds == 0.0 {
                DEFAULT_MAX_BACKLOG_SECONDS
            } else {
                f64::from(max_backlog_seconds)
            },
            roughness: or(roughness, svc_water::NATURAL_CHANNEL_MANNING_N),
        })
    }

    fn flow(self) -> FlowParams {
        FlowParams {
            manning_n: self.roughness,
            ..FlowParams::default()
        }
    }
}

#[path = "water_field_worker.rs"]
mod worker;
use worker::{FieldShared, Snapshot, Totals, Worker};

/// How a presented field is drawn.
#[derive(Debug, Clone)]
struct Drawn {
    material: u64,
    material_id: String,
    layer: RenderLayer,
    drawn_depth: f32,
    /// The snapshot last drawn, and with which material and depth.
    drawn: Option<(u64, u64, u32)>,
    drawn_cells: u32,
}

struct Field {
    width: usize,
    length: usize,
    cell_size: f32,
    /// The spatial session the anchor resolves in (0: none, `origin` as given).
    session: u64,
    anchor: core_space::GlobalPosition,
    origin: [f64; 3],
    incarnation: u64,
    admitted: f64,
    /// The policy the latest change selected; its backlog bounds pending work.
    policy: Policy,
    shared: Arc<FieldShared>,
    latest: Arc<Snapshot>,
    drawn: Option<Drawn>,
}

impl Field {
    /// Where the field's corner stands in its layer now.
    fn placement(&self, spatial: &SpatialCollisionSource) -> [f64; 3] {
        if self.session == 0 {
            return self.origin;
        }
        let origin = spatial
            .world_origin(NativeSpatialSessionHandle {
                value: self.session,
            })
            .unwrap_or(core_space::WorldOrigin::ZERO);
        let cell = origin.cell();
        std::array::from_fn(|axis| {
            (i128::from(self.anchor.cell()[axis]) - i128::from(cell[axis])) as f64
                + self.anchor.offset()[axis]
        })
    }

    fn index(&self, x: u32, z: u32) -> Result<usize, CsharpEngineServicesError> {
        let (x, z) = (x as usize, z as usize);
        if x >= self.width || z >= self.length {
            return Err(refused(format!(
                "cell ({x}, {z}) is outside the {} by {} water field",
                self.width, self.length
            )));
        }
        Ok(z * self.width + x)
    }

    fn readout(&self, placement: [f64; 3]) -> NativeWaterFieldReadout {
        let latest = &self.latest;
        let pending = self.shared.pending.lock().expect("water field pending");
        NativeWaterFieldReadout {
            origin: NativeVec3 {
                x: placement[0] as f32,
                y: placement[1] as f32,
                z: placement[2] as f32,
            },
            width: self.width as u32,
            length: self.length as u32,
            cell_size: self.cell_size,
            incarnation: self.incarnation,
            snapshot_incarnation: latest.incarnation,
            snapshots: latest.sequence,
            time: latest.time,
            admitted_time: self.admitted,
            age: (self.admitted - latest.time).max(0.0),
            dropped_seconds: latest.dropped_seconds + pending.dropped,
            pending_seconds: pending.seconds(),
            coalesced_changes: pending.coalesced,
            volume: latest.volume,
            boundary_inflow: latest.totals.inflow,
            boundary_outflow: latest.totals.outflow,
            source_volume: latest.totals.source,
            sink_volume: latest.totals.sink,
            wet_cells: latest.wet_cells,
            steps: latest.steps,
            step_microseconds: latest.step_microseconds,
            load: latest.load,
            drawn_cells: self.drawn.as_ref().map_or(0, |drawn| drawn.drawn_cells),
        }
    }

    /// The water at a point of the field's frame, between the four nearest
    /// cell centres (see `NativeWaterFieldSample`).
    fn sample(&self, x: f32, z: f32, lift: f32) -> NativeWaterFieldSample {
        let snapshot = &self.latest;
        let extent = |cells: usize| cells as f32 * self.cell_size;
        let inside = x >= 0.0 && z >= 0.0 && x <= extent(self.width) && z <= extent(self.length);
        let age = (self.admitted - snapshot.time).max(0.0);
        if !inside {
            return NativeWaterFieldSample {
                time: snapshot.time,
                age,
                ..NativeWaterFieldSample::default()
            };
        }
        let axis = |position: f32, cells: usize| {
            let at = (position / self.cell_size - 0.5).clamp(0.0, (cells - 1) as f32);
            let low = at.floor() as usize;
            (low, (low + 1).min(cells - 1), at - low as f32)
        };
        let (x0, x1, fx) = axis(x, self.width);
        let (z0, z1, fz) = axis(z, self.length);
        let corners = [
            (x0, z0, (1.0 - fx) * (1.0 - fz)),
            (x1, z0, fx * (1.0 - fz)),
            (x0, z1, (1.0 - fx) * fz),
            (x1, z1, fx * fz),
        ];
        let dry_depth = svc_water::DEFAULT_DRY_DEPTH;
        let (mut bed, mut wet_weight, mut surface, mut velocity) = (0.0, 0.0, 0.0, [0.0f32; 2]);
        for (cx, cz, weight) in corners {
            let i = cz * self.width + cx;
            bed += weight * snapshot.bed[i];
            if snapshot.depth[i] > dry_depth {
                wet_weight += weight;
                surface += weight * (snapshot.bed[i] + snapshot.depth[i]);
                velocity[0] += weight * snapshot.velocity[i][0];
                velocity[1] += weight * snapshot.velocity[i][1];
            }
        }
        if wet_weight == 0.0 {
            return NativeWaterFieldSample {
                inside: true,
                surface: bed + lift,
                bed: bed + lift,
                time: snapshot.time,
                age,
                ..NativeWaterFieldSample::default()
            };
        }
        let surface = surface / wet_weight;
        let depth = (surface - bed).max(0.0);
        NativeWaterFieldSample {
            inside: true,
            wet: depth > dry_depth,
            surface: surface + lift,
            bed: bed + lift,
            depth,
            velocity_x: velocity[0] / wet_weight,
            velocity_z: velocity[1] / wet_weight,
            time: snapshot.time,
            age,
        }
    }
}

pub(crate) struct RuntimeWaterFieldBridge {
    spatial: SpatialCollisionSource,
    fields: BTreeMap<u64, Field>,
    next: u64,
    worker: Option<Worker>,
    projector: WaterSurfaceProjector,
    in_call: bool,
    admitted_seconds: f64,
    /// The call's drawn-surface work: removals of what was made before the
    /// call go ahead of its Graphics work (`early`), the rest after.
    pub(crate) frames: Vec<RenderFrameDiff>,
    pub(crate) early: Vec<RenderDiff>,
    appearance: Option<*mut RuntimeAppearanceBridge>,
    operation_diagnostics: OperationDiagnostics,
    borrowed: BorrowedResult,
}

impl RuntimeWaterFieldBridge {
    pub(crate) fn new(spatial: SpatialCollisionSource) -> Self {
        Self {
            spatial,
            fields: BTreeMap::new(),
            next: 1,
            worker: None,
            projector: WaterSurfaceProjector::default(),
            in_call: false,
            admitted_seconds: 0.0,
            frames: Vec::new(),
            early: Vec::new(),
            appearance: None,
            operation_diagnostics: OperationDiagnostics::default(),
            borrowed: BorrowedResult::default(),
        }
    }

    pub(crate) fn bind_appearance(&mut self, appearance: &mut RuntimeAppearanceBridge) {
        // The sibling bridge is retained by EngineServiceSet; the pointer is
        // refreshed while assembling the call table and used only during a
        // synchronous callback.
        self.appearance = Some(appearance as *mut RuntimeAppearanceBridge);
    }

    /// A call begins: it admits `update`'s steps (none without an update),
    /// and reads the snapshots the worker has published since the last.
    pub(crate) fn begin_call(&mut self, update: Option<NativeProductUpdateFacts>) {
        self.in_call = true;
        self.admitted_seconds = update.map_or(0.0, |facts| {
            facts.fixed_delta_seconds * f64::from(facts.admitted_step_count)
        });
        self.projector.begin_call();
        for field in self.fields.values_mut() {
            let published = field
                .shared
                .published
                .lock()
                .expect("water field slot")
                .clone();
            // A snapshot finished before the latest bed edit reached the
            // worker is dropped: the field reads the edit from its next one.
            if published.sequence != field.latest.sequence
                && published.incarnation == field.incarnation
            {
                field.latest = published;
            }
        }
    }

    /// As a call finishes: hand every field the time the call admitted, and
    /// draw the presented fields' latest snapshots.
    pub(crate) fn settle(&mut self) -> Result<(), CsharpEngineServicesError> {
        self.in_call = false;
        let seconds = std::mem::take(&mut self.admitted_seconds);
        let mut operations = Vec::new();
        for (&handle, field) in &mut self.fields {
            if seconds > 0.0 {
                field.admitted += seconds;
                let load = field.shared.load();
                field
                    .shared
                    .pending
                    .lock()
                    .expect("water field pending")
                    .admit(seconds, load, field.policy.max_backlog);
            }
            let placement = field.placement(&self.spatial);
            let Some(drawn) = &mut field.drawn else {
                continue;
            };
            let placement = WaterSurfacePlacement {
                origin: placement.map(|value| value as f32),
                layer: drawn.layer,
            };
            let latest = &field.latest;
            let key = (latest.sequence, drawn.material, drawn.drawn_depth.to_bits());
            if drawn.drawn == Some(key) {
                operations.extend(self.projector.place(handle, placement));
                continue;
            }
            let built;
            let mesh = match &latest.surface {
                Some((depth, mesh)) if *depth == drawn.drawn_depth => mesh.as_ref(),
                _ => {
                    built = water_surface_mesh(WaterSurfaceCells {
                        width: field.width,
                        length: field.length,
                        cell_size: field.cell_size,
                        bed: &latest.bed,
                        depth: &latest.depth,
                        drawn_depth: drawn.drawn_depth,
                    });
                    built.as_ref()
                }
            };
            drawn.drawn_cells = mesh.map_or(0, |mesh| mesh.drawn_cells);
            operations.extend(
                self.projector
                    .draw(handle, placement, mesh, &drawn.material_id)
                    .map_err(|error| refused(format!("water surface node: {error:?}")))?,
            );
            drawn.drawn = Some(key);
        }
        if seconds > 0.0 {
            if let Some(worker) = &self.worker {
                worker.wake();
            }
        }
        push_frame(&mut self.frames, operations)
    }

    /// Records changes a call made to a field, in effect from the next
    /// admitted time, and wakes the worker for them.
    fn change(
        &mut self,
        handle: NativeWaterFieldHandle,
        apply: impl FnOnce(&mut worker::Changes),
    ) -> Result<(), CsharpEngineServicesError> {
        self.field(handle)?
            .shared
            .pending
            .lock()
            .expect("water field pending")
            .change(apply);
        if let Some(worker) = &self.worker {
            worker.wake();
        }
        Ok(())
    }

    /// Whether a field lives, for Dynamics water entries.
    pub(crate) fn lives(&self, field: u64) -> bool {
        self.fields.contains_key(&field)
    }

    /// The water at a scene point (x, z) of a live field's latest snapshot,
    /// for Dynamics water; `None` outside it.
    pub(crate) fn sample_at(&self, field: u64, x: f32, z: f32) -> Option<NativeWaterFieldSample> {
        let field = self.fields.get(&field)?;
        let placement = field.placement(&self.spatial).map(|value| value as f32);
        let sample = field.sample(x - placement[0], z - placement[2], placement[1]);
        sample.inside.then_some(sample)
    }

    fn require_call(&self) -> Result<(), CsharpEngineServicesError> {
        if self.in_call {
            Ok(())
        } else {
            Err(refused("water fields were called outside a product call"))
        }
    }

    fn appearance_mut(
        &mut self,
    ) -> Result<&mut RuntimeAppearanceBridge, CsharpEngineServicesError> {
        let appearance = self
            .appearance
            .ok_or_else(|| refused("water fields have no Engine appearance owner"))?;
        // SAFETY: bind_appearance points at the sibling owner retained by
        // EngineServiceSet; callbacks do not retain it beyond this call.
        Ok(unsafe { &mut *appearance })
    }

    fn worker(&mut self) -> Result<&Worker, CsharpEngineServicesError> {
        if self.worker.is_none() {
            self.worker = Some(Worker::spawn()?);
        }
        Ok(self.worker.as_ref().expect("spawned above"))
    }

    fn field(&self, handle: NativeWaterFieldHandle) -> Result<&Field, CsharpEngineServicesError> {
        self.require_call()?;
        self.fields
            .get(&handle.value)
            .ok_or_else(|| refused("the water field does not live"))
    }

    fn field_mut(
        &mut self,
        handle: NativeWaterFieldHandle,
    ) -> Result<&mut Field, CsharpEngineServicesError> {
        self.require_call()?;
        self.fields
            .get_mut(&handle.value)
            .ok_or_else(|| refused("the water field does not live"))
    }

    fn readout(
        &self,
        handle: NativeWaterFieldHandle,
    ) -> Result<NativeWaterFieldReadout, CsharpEngineServicesError> {
        let field = self.field(handle)?;
        Ok(field.readout(field.placement(&self.spatial)))
    }

    fn create(
        &mut self,
        request: &NativeWaterFieldCreateRequest,
    ) -> Result<NativeWaterFieldHandle, CsharpEngineServicesError> {
        self.require_call()?;
        let bed = unsafe { borrowed_slice(request.bed, request.bed_len, "water field bed")? };
        let depth =
            unsafe { borrowed_slice(request.depth, request.depth_len, "water field depth")? };
        let (width, length) = (request.width as usize, request.length as usize);
        let cells = width * length;
        if cells == 0 {
            return Err(refused("a water field has at least one cell"));
        }
        if !(request.cell_size.is_finite() && request.cell_size > 0.0) {
            return Err(refused("a water field's cell size is finite and above 0"));
        }
        if bed.len() != cells || !bed.iter().all(|height| height.is_finite()) {
            return Err(refused(format!(
                "a water field of {width} by {length} cells has {cells} finite bed heights, not {}",
                bed.len()
            )));
        }
        if !(depth.is_empty()
            || depth.len() == cells && depth.iter().all(|d| d.is_finite() && *d >= 0.0))
        {
            return Err(refused(format!(
                "a water field's starting depths are none or {cells}, finite and not negative"
            )));
        }
        let origin = [request.origin.x, request.origin.y, request.origin.z].map(f64::from);
        if !origin.iter().all(|value| value.is_finite()) {
            return Err(refused("a water field's origin is finite"));
        }
        let anchor = core_space::GlobalPosition::new(
            [
                request.anchor.cell_x,
                request.anchor.cell_y,
                request.anchor.cell_z,
            ],
            [
                request.anchor.offset_x,
                request.anchor.offset_y,
                request.anchor.offset_z,
            ],
        )
        .map_err(|_| refused("a water field's anchor is a finite world position"))?;
        let session = request.session.session;
        if session.value != 0 && self.spatial.world_origin(session).is_none() {
            return Err(refused(
                "the water field's anchor names a spatial session that does not live",
            ));
        }
        let policy = Policy::of(request.policy)?;
        let mut grid = WaterGrid::new(width, length, request.cell_size, bed.to_vec());
        grid.set_params(policy.flow());
        if !depth.is_empty() {
            grid.depth_mut().copy_from_slice(depth);
        }
        let shared_bed: Arc<[f32]> = bed.into();
        let first = Arc::new(Snapshot {
            incarnation: 0,
            sequence: 0,
            time: 0.0,
            bed: shared_bed.clone(),
            depth: grid.depth().to_vec(),
            velocity: grid.velocity().to_vec(),
            volume: grid.total_volume(),
            totals: Totals::default(),
            wet_cells: (0..cells).filter(|&i| grid.is_wet(i)).count() as u32,
            dropped_seconds: 0.0,
            steps: 0,
            step_microseconds: 0,
            load: 0.0,
            surface: None,
        });
        let shared = Arc::new(FieldShared::new(Box::new(grid), first.clone()));
        let value = self.next;
        self.worker()?.register(value, shared.clone(), policy);
        self.next += 1;
        self.fields.insert(
            value,
            Field {
                width,
                length,
                cell_size: request.cell_size,
                session: session.value,
                anchor,
                origin,
                incarnation: 0,
                admitted: 0.0,
                policy,
                shared,
                latest: first,
                drawn: None,
            },
        );
        Ok(NativeWaterFieldHandle { value })
    }

    fn destroy(&mut self, handle: NativeWaterFieldHandle) -> Result<(), CsharpEngineServicesError> {
        self.require_call()?;
        let Some(field) = self.fields.remove(&handle.value) else {
            return Ok(());
        };
        if let Some(worker) = &self.worker {
            worker.retire(handle.value);
        }
        if let Some(drawn) = field.drawn {
            self.stop_drawing(handle.value, drawn.material)?;
        }
        Ok(())
    }

    fn stop_drawing(&mut self, field: u64, material: u64) -> Result<(), CsharpEngineServicesError> {
        let removals = self.projector.remove(field);
        self.early.extend(removals.early);
        push_frame(&mut self.frames, removals.late)?;
        self.appearance_mut()?
            .hold_for_engine_surface(material, false)?;
        Ok(())
    }

    fn edit_bed(
        &mut self,
        request: &NativeWaterFieldBedRequest,
    ) -> Result<NativeWaterFieldReadout, CsharpEngineServicesError> {
        let cells =
            unsafe { borrowed_slice(request.cells, request.cells_len, "water field bed cells")? };
        let field = self.field(request.field)?;
        let edits = cells
            .iter()
            .map(|cell| {
                if !cell.value.is_finite() {
                    return Err(refused("a water field's bed heights are finite"));
                }
                Ok((field.index(cell.x, cell.z)?, cell.value))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let incarnation = field.incarnation + 1;
        self.change(request.field, |changes| {
            changes.bed.extend(edits);
            changes.incarnation = Some(incarnation);
        })?;
        self.field_mut(request.field)?.incarnation = incarnation;
        self.readout(request.field)
    }

    fn set_sources(
        &mut self,
        request: &NativeWaterFieldSourcesRequest,
    ) -> Result<NativeWaterFieldReadout, CsharpEngineServicesError> {
        let cells = unsafe {
            borrowed_slice(request.cells, request.cells_len, "water field source cells")?
        };
        let field = self.field(request.field)?;
        if !request.uniform_rate.is_finite() {
            return Err(refused("a water field's source rates are finite"));
        }
        let mut rates = vec![request.uniform_rate; field.width * field.length];
        for cell in cells {
            if !cell.value.is_finite() {
                return Err(refused("a water field's source rates are finite"));
            }
            rates[field.index(cell.x, cell.z)?] = cell.value;
        }
        self.change(request.field, |changes| changes.sources = Some(rates))?;
        self.readout(request.field)
    }

    fn set_boundary(
        &mut self,
        request: NativeWaterFieldBoundaryRequest,
    ) -> Result<NativeWaterFieldReadout, CsharpEngineServicesError> {
        let field = self.field(request.field)?;
        let side = match request.side {
            NativeWaterFieldSide::XMax => Side::XMax,
            NativeWaterFieldSide::XMin => Side::XMin,
            NativeWaterFieldSide::ZMax => Side::ZMax,
            NativeWaterFieldSide::ZMin => Side::ZMin,
        };
        let side_len = match side {
            Side::XMax | Side::XMin => field.length,
            Side::ZMax | Side::ZMin => field.width,
        };
        let first = request.first as usize;
        let end = if request.count == 0 {
            side_len
        } else {
            first + request.count as usize
        };
        if first >= end || end > side_len {
            return Err(refused(format!(
                "edge cells {first} to {end} are outside a side of {side_len} cells"
            )));
        }
        if !request.value.is_finite() {
            return Err(refused("a water field's boundary value is finite"));
        }
        let boundary = match request.kind {
            NativeWaterFieldBoundaryKind::Wall => Boundary::Wall,
            NativeWaterFieldBoundaryKind::Stage => Boundary::Stage(request.value),
            NativeWaterFieldBoundaryKind::Inflow => {
                if request.value < 0.0 {
                    return Err(refused("an inflow's discharge is not negative"));
                }
                Boundary::Inflow(request.value)
            }
            NativeWaterFieldBoundaryKind::Open => Boundary::Open,
        };
        // Kept per edge cell: an inflow's discharge is spread over its run.
        let per_cell = match boundary {
            Boundary::Inflow(discharge) => Boundary::Inflow(discharge / (end - first) as f32),
            other => other,
        };
        self.change(request.field, |changes| {
            for cell in first..end {
                changes.boundaries.insert((side.index(), cell), per_cell);
            }
        })?;
        self.readout(request.field)
    }

    fn set_policy(
        &mut self,
        request: NativeWaterFieldPolicyRequest,
    ) -> Result<NativeWaterFieldReadout, CsharpEngineServicesError> {
        self.field(request.field)?;
        let policy = Policy::of(request.policy)?;
        self.change(request.field, |changes| changes.policy = Some(policy))?;
        self.field_mut(request.field)?.policy = policy;
        self.readout(request.field)
    }

    fn present(
        &mut self,
        request: NativeWaterFieldPresentRequest,
    ) -> Result<NativeWaterFieldReadout, CsharpEngineServicesError> {
        let handle = request.field.value;
        let previous = self
            .field(request.field)?
            .drawn
            .as_ref()
            .map(|drawn| drawn.material);
        if request.material.value == 0 {
            if let Some(material) = previous {
                self.field_mut(request.field)?.drawn = None;
                self.stop_drawing(handle, material)?;
                self.change(request.field, |changes| changes.draw = Some(None))?;
            }
            return self.readout(request.field);
        }
        let layer = match request.layer {
            NativeRenderLayer::Scene => RenderLayer::Scene,
            NativeRenderLayer::Backdrop => RenderLayer::Backdrop,
            _ => {
                return Err(refused(
                    "a water field draws in the Scene or the Backdrop layer",
                ))
            }
        };
        if !(request.drawn_depth.is_finite() && request.drawn_depth >= 0.0) {
            return Err(refused(
                "a water field's drawn depth is finite and not negative",
            ));
        }
        let drawn_depth = if request.drawn_depth == 0.0 {
            DEFAULT_DRAWN_DEPTH
        } else {
            request.drawn_depth
        };
        let appearance = self.appearance_mut()?;
        let material_id = appearance.hold_for_engine_surface(request.material.value, true)?;
        if let Some(material) = previous {
            appearance.hold_for_engine_surface(material, false)?;
        }
        let field = self.field_mut(request.field)?;
        let kept_layer = field
            .drawn
            .as_ref()
            .is_none_or(|drawn| drawn.layer == layer);
        field.drawn = Some(Drawn {
            material: request.material.value,
            material_id,
            layer,
            drawn_depth,
            drawn: None,
            drawn_cells: 0,
        });
        if !kept_layer {
            // A node keeps the layer it was made in: make it again.
            let removals = self.projector.remove(handle);
            self.early.extend(removals.early);
            push_frame(&mut self.frames, removals.late)?;
        }
        self.change(request.field, |changes| {
            changes.draw = Some(Some(drawn_depth))
        })?;
        self.readout(request.field)
    }

    fn sample(
        &mut self,
        request: &NativeWaterFieldSampleRequest,
    ) -> Result<NativeWaterFieldSamplesResult, CsharpEngineServicesError> {
        let points =
            unsafe { borrowed_slice(request.points, request.points_len, "water field points")? };
        let field = self.field(request.field)?;
        let placement = field.placement(&self.spatial).map(|value| value as f32);
        let samples: Vec<NativeWaterFieldSample> = points
            .iter()
            .map(|point| field.sample(point.x - placement[0], point.z - placement[2], placement[1]))
            .collect();
        let result = NativeWaterFieldSamplesResult {
            samples: samples.as_ptr(),
            samples_len: samples.len(),
        };
        self.borrowed.hold(samples);
        Ok(result)
    }

    fn read_cells(
        &mut self,
        handle: NativeWaterFieldHandle,
    ) -> Result<NativeWaterFieldCellsResult, CsharpEngineServicesError> {
        let latest = &self.field(handle)?.latest;
        let cells: Vec<NativeWaterFieldCell> = latest
            .depth
            .iter()
            .zip(latest.bed.iter())
            .zip(&latest.velocity)
            .map(|((&depth, &bed), velocity)| NativeWaterFieldCell {
                depth,
                surface: bed + depth,
                velocity_x: velocity[0],
                velocity_z: velocity[1],
            })
            .collect();
        let result = NativeWaterFieldCellsResult {
            cells: cells.as_ptr(),
            cells_len: cells.len(),
        };
        self.borrowed.hold(cells);
        Ok(result)
    }
}

fn push_frame(
    frames: &mut Vec<RenderFrameDiff>,
    operations: Vec<RenderDiff>,
) -> Result<(), CsharpEngineServicesError> {
    if operations.is_empty() {
        return Ok(());
    }
    frames.push(
        RenderFrameDiff::try_from_ops(operations)
            .map_err(|error| refused(format!("water surface frame is invalid: {error:?}")))?,
    );
    Ok(())
}

pub(crate) fn api(
    bridge: &mut RuntimeWaterFieldBridge,
    appearance: &mut RuntimeAppearanceBridge,
) -> NativeWaterFieldApi {
    bridge.bind_appearance(appearance);
    NativeWaterFieldApi {
        context: (bridge as *mut RuntimeWaterFieldBridge).cast(),
        create,
        destroy,
        edit_bed,
        set_sources,
        set_boundary,
        set_policy,
        present,
        sample,
        read_cells,
        read,
    }
}

/// Runs one operation: clears the receipt, then writes the result or the
/// refusal.
unsafe fn operate<T>(
    context: *mut c_void,
    output: *mut T,
    error: *mut NativeOperationErrorReceipt,
    operation: impl FnOnce(&mut RuntimeWaterFieldBridge) -> Result<T, CsharpEngineServicesError>,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || output.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeWaterFieldBridge>() };
    match operation(bridge) {
        Ok(value) => {
            unsafe { *output = value };
            ABI_OK
        }
        Err(failure) => {
            bridge.operation_diagnostics.retain(&failure, error);
            0
        }
    }
}

unsafe extern "C" fn create(
    context: *mut c_void,
    request: *const NativeWaterFieldCreateRequest,
    result: *mut NativeWaterFieldHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    unsafe { operate(context, result, error, |bridge| bridge.create(&*request)) }
}

unsafe extern "C" fn destroy(
    context: *mut c_void,
    field: NativeWaterFieldHandle,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeWaterFieldBridge>() };
    match bridge.destroy(field) {
        Ok(()) => ABI_OK,
        Err(failure) => refuse(&failure, error),
    }
}

unsafe extern "C" fn edit_bed(
    context: *mut c_void,
    request: *const NativeWaterFieldBedRequest,
    readout: *mut NativeWaterFieldReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    unsafe { operate(context, readout, error, |bridge| bridge.edit_bed(&*request)) }
}

unsafe extern "C" fn set_sources(
    context: *mut c_void,
    request: *const NativeWaterFieldSourcesRequest,
    readout: *mut NativeWaterFieldReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    unsafe {
        operate(context, readout, error, |bridge| {
            bridge.set_sources(&*request)
        })
    }
}

unsafe extern "C" fn set_boundary(
    context: *mut c_void,
    request: NativeWaterFieldBoundaryRequest,
    readout: *mut NativeWaterFieldReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe {
        operate(context, readout, error, |bridge| {
            bridge.set_boundary(request)
        })
    }
}

unsafe extern "C" fn set_policy(
    context: *mut c_void,
    request: NativeWaterFieldPolicyRequest,
    readout: *mut NativeWaterFieldReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.set_policy(request)) }
}

unsafe extern "C" fn present(
    context: *mut c_void,
    request: NativeWaterFieldPresentRequest,
    readout: *mut NativeWaterFieldReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.present(request)) }
}

unsafe extern "C" fn sample(
    context: *mut c_void,
    request: *const NativeWaterFieldSampleRequest,
    result: *mut NativeWaterFieldSamplesResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    unsafe { operate(context, result, error, |bridge| bridge.sample(&*request)) }
}

unsafe extern "C" fn read_cells(
    context: *mut c_void,
    field: NativeWaterFieldHandle,
    result: *mut NativeWaterFieldCellsResult,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, result, error, |bridge| bridge.read_cells(field)) }
}

unsafe extern "C" fn read(
    context: *mut c_void,
    field: NativeWaterFieldHandle,
    readout: *mut NativeWaterFieldReadout,
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    unsafe { operate(context, readout, error, |bridge| bridge.readout(field)) }
}

#[cfg(test)]
#[path = "water_field_tests.rs"]
mod tests;
