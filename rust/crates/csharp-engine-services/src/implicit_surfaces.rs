//! Staged general-purpose fields. Generated output uses the canonical Graphics
//! mesh resource path, including renderer recovery and resource lifetimes.
use crate::{
    appearance::RuntimeAppearanceBridge,
    composition::{borrowed_slice, ABI_OK},
    CsharpEngineServicesError,
};
use csharp_engine_abi::*;
use std::{collections::BTreeMap, ffi::c_void, sync::Arc, time::Instant};
use svc_implicit::{
    surface::{self, MaterialBoundaryMode, MaterialRegion, MaterialSampling, SurfaceOptions},
    volume::{SampledVolume, VolumeDescriptor},
    Bounds, Field, GenerateOptions, Geometry, Node,
};

type Result<T> = std::result::Result<T, CsharpEngineServicesError>;
fn error(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_IMPLICIT_SURFACE", message)
}
fn kernel(e: svc_implicit::Error) -> CsharpEngineServicesError {
    error(e.to_string())
}
fn v(p: NativeVec3) -> [f32; 3] {
    [p.x, p.y, p.z]
}
fn nv(p: [f32; 3]) -> NativeVec3 {
    NativeVec3 {
        x: p[0],
        y: p[1],
        z: p[2],
    }
}
fn native_volume_descriptor(value: VolumeDescriptor) -> NativeSampledVolumeDescriptor {
    NativeSampledVolumeDescriptor {
        origin: nv(value.origin),
        spacing: value.spacing,
        width: value.dimensions[0],
        height: value.dimensions[1],
        depth: value.dimensions[2],
        revision: value.revision,
    }
}

#[derive(Clone)]
struct RetainedField {
    field: Arc<Field>,
    nodes: BTreeMap<u64, Node>,
    generation: Option<NativeImplicitGenerationReadout>,
}
impl RetainedField {
    fn node(&self, token: NativeImplicitNode) -> Result<Node> {
        self.nodes
            .get(&token.value)
            .copied()
            .ok_or_else(|| error("unknown implicit field node"))
    }
}

#[derive(Clone)]
struct RetainedVolume {
    volume: Arc<SampledVolume>,
    generation: Option<NativeImplicitGenerationReadout>,
}

struct DensitySnapshotLease {
    samples: Vec<NativeDensitySample>,
}

#[derive(Clone, Copy)]
struct SurfaceGenerationOptions {
    crease_angle_degrees: f32,
    uv_scale: f32,
    default_material: NativeMaterialHandle,
    material_boundary_mode: NativeImplicitMaterialBoundaryMode,
    material_sample_spacing: f32,
}

#[derive(Clone, Default)]
pub(crate) struct RuntimeImplicitCall {
    fields: BTreeMap<u64, RetainedField>,
    volumes: BTreeMap<u64, RetainedVolume>,
}
pub(crate) struct RuntimeImplicitBridge {
    state: RuntimeImplicitCall,
    staged: Option<RuntimeImplicitCall>,
    // Monotonic even across rollback: stale managed values cannot alias a new arena.
    next_field: u64,
    // Public node identities are not the local svc-implicit arena indices.
    // Keep this outside staged state so discarded nodes cannot be reused.
    next_node: u64,
    // Volume identities are independent from field/node identities and never
    // reused after staged rollback.
    next_volume: u64,
    appearance: Option<*mut RuntimeAppearanceBridge>,
    callback_error: Option<CsharpEngineServicesError>,
    diagnostic_leases: BTreeMap<u64, Box<GenerationDiagnosticLease>>,
    next_diagnostic_lease: u64,
    density_snapshot_leases: BTreeMap<u64, DensitySnapshotLease>,
    next_density_snapshot_lease: u64,
}
impl RuntimeImplicitBridge {
    pub(crate) fn new() -> Self {
        Self {
            state: RuntimeImplicitCall::default(),
            staged: None,
            next_field: 1,
            next_node: 1,
            next_volume: 1,
            appearance: None,
            callback_error: None,
            diagnostic_leases: BTreeMap::new(),
            next_diagnostic_lease: 1,
            density_snapshot_leases: BTreeMap::new(),
            next_density_snapshot_lease: 1,
        }
    }
    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(self.state.clone());
        self.callback_error = None;
    }
    pub(crate) fn discard_call(&mut self) {
        self.staged = None;
        self.callback_error = None;
    }
    pub(crate) fn take_call(&mut self) -> Result<RuntimeImplicitCall> {
        if let Some(e) = self.callback_error.take() {
            return Err(e);
        }
        self.staged
            .take()
            .ok_or_else(|| error("implicit call was not staged"))
    }
    pub(crate) fn commit_call(&mut self, call: RuntimeImplicitCall) {
        self.state = call;
    }
    fn stage(&mut self) -> Result<&mut RuntimeImplicitCall> {
        self.staged
            .as_mut()
            .ok_or_else(|| error("implicit operations require an active product call"))
    }
    fn retained(&mut self, handle: NativeImplicitFieldHandle) -> Result<&mut RetainedField> {
        self.stage()?
            .fields
            .get_mut(&handle.value)
            .ok_or_else(|| error("unknown implicit field"))
    }
    fn retained_volume(
        &mut self,
        handle: NativeSampledVolumeHandle,
    ) -> Result<&mut RetainedVolume> {
        self.stage()?
            .volumes
            .get_mut(&handle.value)
            .ok_or_else(|| error("unknown sampled volume"))
    }
    fn allocate_volume(&mut self) -> Result<u64> {
        let value = self.next_volume;
        self.next_volume = value
            .checked_add(1)
            .ok_or_else(|| error("sampled volume identity exhausted"))?;
        Ok(value)
    }
    fn retain_density_snapshot(
        &mut self,
        descriptor: VolumeDescriptor,
        start: u32,
        samples: Vec<NativeDensitySample>,
    ) -> Result<NativeDensitySnapshotLease> {
        let value = self.next_density_snapshot_lease;
        self.next_density_snapshot_lease = value
            .checked_add(1)
            .ok_or_else(|| error("density snapshot lease identity exhausted"))?;
        self.density_snapshot_leases
            .insert(value, DensitySnapshotLease { samples });
        let retained = self
            .density_snapshot_leases
            .get(&value)
            .expect("inserted density snapshot lease");
        Ok(NativeDensitySnapshotLease {
            handle: NativeDensitySnapshotLeaseHandle { value },
            descriptor: native_volume_descriptor(descriptor),
            start,
            samples: retained.samples.as_ptr(),
            samples_len: retained.samples.len(),
        })
    }
    fn node(
        &mut self,
        handle: NativeImplicitFieldHandle,
        token: NativeImplicitNode,
    ) -> Result<Node> {
        self.retained(handle)?.node(token)
    }
    fn allocate_node(&mut self) -> Result<u64> {
        let value = self.next_node;
        self.next_node = value
            .checked_add(1)
            .ok_or_else(|| error("implicit node identity exhausted"))?;
        Ok(value)
    }
    fn edit(
        &mut self,
        handle: NativeImplicitFieldHandle,
        action: impl FnOnce(&mut Field) -> std::result::Result<Node, svc_implicit::Error>,
    ) -> Result<NativeImplicitNode> {
        let token = self.allocate_node()?;
        let retained = self.retained(handle)?;
        let value = action(Arc::make_mut(&mut retained.field)).map_err(kernel)?;
        retained.nodes.insert(token, value);
        Ok(NativeImplicitNode { value: token })
    }
    unsafe fn admit_geometry(
        &mut self,
        field: &Field,
        geometry: &Geometry,
        region_nodes: &[(Node, NativeMaterialHandle)],
        options: SurfaceGenerationOptions,
        generation_seconds: f64,
    ) -> Result<(NativeMeshResourceHandle, NativeImplicitGenerationReadout)> {
        let material_sampling = if !options.material_sample_spacing.is_finite()
            || options.material_sample_spacing < 0.0
        {
            return Err(error(
                "material sample spacing must be finite and non-negative",
            ));
        } else if options.material_sample_spacing > 0.0 {
            if options.material_boundary_mode != NativeImplicitMaterialBoundaryMode::Interpolated {
                return Err(error(
                    "material sample spacing requires interpolated material boundaries",
                ));
            }
            Some(MaterialSampling {
                max_edge_length: options.material_sample_spacing,
                max_vertices: 262_144,
                max_triangles: 262_144,
            })
        } else {
            None
        };
        if geometry.triangles.is_empty() {
            return Err(error(
                "field has no extractable surface in the selected domain",
            ));
        }
        let topology = geometry.topology();
        let mut materials = vec![options.default_material];
        let regions: Vec<_> = region_nodes
            .iter()
            .map(|(node, material)| {
                let slot = materials
                    .iter()
                    .position(|m| m.value == material.value)
                    .unwrap_or_else(|| {
                        materials.push(*material);
                        materials.len() - 1
                    });
                MaterialRegion {
                    node: *node,
                    slot: slot as u32,
                }
            })
            .collect();
        let mesh = surface::assemble(
            field,
            geometry,
            &regions,
            SurfaceOptions {
                crease_angle_degrees: options.crease_angle_degrees,
                uv_scale: options.uv_scale,
                default_slot: 0,
                material_boundary_mode: match options.material_boundary_mode {
                    NativeImplicitMaterialBoundaryMode::Centroid => MaterialBoundaryMode::Centroid,
                    NativeImplicitMaterialBoundaryMode::Interpolated => {
                        MaterialBoundaryMode::Interpolated
                    }
                },
                material_sampling,
            },
        )
        .map_err(kernel)?;
        let positions: Vec<_> = mesh.positions.iter().copied().map(nv).collect();
        let normals: Vec<_> = mesh.normals.iter().copied().map(nv).collect();
        let uvs: Vec<_> = mesh
            .uvs
            .iter()
            .map(|p| NativeVec2 { x: p[0], y: p[1] })
            .collect();
        let groups: Vec<_> = mesh
            .groups
            .iter()
            .map(|g| NativeMeshGroup {
                material_slot: g.slot,
                start: g.index_start,
                count: g.index_count,
            })
            .collect();
        let bindings: Vec<_> = mesh
            .groups
            .iter()
            .map(|g| NativeMeshMaterialBinding {
                material_slot: g.slot,
                material: materials[g.slot as usize],
            })
            .collect();
        let raw = NativeMeshResourceCreateRequest {
            positions: positions.as_ptr(),
            positions_len: positions.len(),
            normals: normals.as_ptr(),
            normals_len: normals.len(),
            uvs: uvs.as_ptr(),
            uvs_len: uvs.len(),
            colors: std::ptr::null(),
            colors_len: 0,
            indices: mesh.indices.as_ptr(),
            indices_len: mesh.indices.len(),
            groups: groups.as_ptr(),
            groups_len: groups.len(),
            bindings: bindings.as_ptr(),
            bindings_len: bindings.len(),
        };
        let appearance = self
            .appearance
            .ok_or_else(|| error("Graphics is not bound"))?;
        // The EngineServiceSet refreshes this sibling pointer for the current
        // synchronous call. Admission copies all streams before returning.
        let handle = unsafe { (&mut *appearance).create_mesh_resource(&raw) }?;
        Ok((
            handle,
            NativeImplicitGenerationReadout {
                node_count: field.node_count() as u32,
                vertices: positions.len() as u32,
                triangles: mesh.indices.len() as u32 / 3,
                material_groups: groups.len() as u32,
                octree_depth: geometry.depth,
                sample_spacing: geometry.cell_size[0],
                generation_seconds,
                reoriented_triangles: geometry.reoriented_triangles,
                degenerate_triangles: geometry.degenerate_triangles,
                boundary_edges: topology.boundary_edges,
                non_manifold_edges: topology.non_manifold_edges,
                inconsistent_winding_edges: topology.inconsistent_winding_edges,
                // This remains specific to adaptive expression-field octree
                // leaves. Sampled-volume extraction reports zero because its
                // uniform QEF fallbacks have distinct semantics.
                bounded_leaf_vertices: geometry.bounded_leaf_vertices,
            },
        ))
    }
    unsafe fn generate(
        &mut self,
        request: &NativeImplicitGenerateRequest,
    ) -> Result<NativeMeshResourceHandle> {
        let started = Instant::now();
        let regions = unsafe {
            borrowed_slice(
                request.regions,
                request.regions_len,
                "implicit material regions",
            )
        }?;
        if regions.len() > 255 {
            return Err(error("at most 255 material regions are supported per mesh"));
        }
        let (field, source, region_nodes) = {
            let retained = self.retained(request.field)?;
            let source = retained.node(request.source)?;
            let region_nodes = regions
                .iter()
                .map(|region| Ok((retained.node(region.node)?, region.material)))
                .collect::<Result<Vec<_>>>()?;
            (retained.field.clone(), source, region_nodes)
        };
        let geometry = field
            .generate(
                source,
                GenerateOptions {
                    bounds: Bounds {
                        min: v(request.minimum),
                        max: v(request.maximum),
                    },
                    cell_size: request.cell_size,
                    max_vertices: 262_144,
                    max_triangles: 262_144,
                },
            )
            .map_err(kernel)?;
        let (handle, readout) = unsafe {
            self.admit_geometry(
                &field,
                &geometry,
                &region_nodes,
                SurfaceGenerationOptions {
                    crease_angle_degrees: request.crease_angle_degrees,
                    uv_scale: request.uv_scale,
                    default_material: request.default_material,
                    material_boundary_mode: request.material_boundary_mode,
                    material_sample_spacing: request.material_sample_spacing,
                },
                started.elapsed().as_secs_f64(),
            )
        }?;
        self.retained(request.field)?.generation = Some(readout);
        Ok(handle)
    }
}

pub(crate) fn api(
    bridge: &mut RuntimeImplicitBridge,
    appearance: &mut RuntimeAppearanceBridge,
) -> NativeImplicitSurfacesApi {
    bridge.appearance = Some(appearance as *mut _);
    NativeImplicitSurfacesApi {
        context: (bridge as *mut RuntimeImplicitBridge).cast(),
        create_field,
        destroy_field,
        create_sampled_volume,
        destroy_sampled_volume,
        describe_sampled_volume,
        write_sampled_volume,
        read_sampled_volume,
        destroy_density_snapshot_lease,
        sample_sampled_volume,
        rasterize_sampled_volume,
        generate_sampled_volume,
        read_sampled_volume_generation,
        add_box,
        add_sphere,
        add_ellipsoid,
        add_capsule,
        add_frustum,
        add_plane,
        union,
        intersection,
        difference,
        smooth_union,
        offset,
        transform,
        sample,
        generate,
        read_generation,
        destroy_operation_diagnostic_lease,
    }
}
fn call<T>(
    context: *mut c_void,
    result: *mut T,
    action: impl FnOnce(&mut RuntimeImplicitBridge) -> Result<T>,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeImplicitBridge>() };
    // Fidget/JIT failures must not unwind through the ABI. Normal errors retain
    // their actual detail for the owning failed product call.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| action(bridge))) {
        Ok(Ok(value)) => {
            unsafe { *result = value };
            ABI_OK
        }
        Ok(Err(e)) => {
            bridge.callback_error = Some(e);
            0
        }
        Err(_) => {
            bridge.callback_error = Some(error(
                "implicit backend panicked during generation or evaluation",
            ));
            0
        }
    }
}
fn call_operation<T>(
    context: *mut c_void,
    result: *mut T,
    receipt: *mut NativeOperationErrorReceipt,
    operation: &'static [u8],
    action: impl FnOnce(&mut RuntimeImplicitBridge) -> Result<T>,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe {
        *receipt = std::mem::zeroed();
    }
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeImplicitBridge>() };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| action(bridge))) {
        Ok(Ok(value)) => {
            unsafe { *result = value };
            ABI_OK
        }
        Ok(Err(error)) if error.code() == "CSHARP_SPATIAL_POINTER" => {
            bridge.callback_error = Some(error);
            0
        }
        Ok(Err(error)) => {
            bridge.retain_operation_error(operation, error, receipt);
            0
        }
        Err(_) => {
            bridge.callback_error = Some(error("implicit backend panicked during operation"));
            0
        }
    }
}
unsafe extern "C" fn create_field(
    context: *mut c_void,
    result: *mut NativeImplicitFieldHandle,
) -> i32 {
    call(context, result, |b| {
        let value = b.next_field;
        b.next_field = value
            .checked_add(1)
            .ok_or_else(|| error("field identity exhausted"))?;
        b.stage()?.fields.insert(
            value,
            RetainedField {
                field: Arc::new(Field::new()),
                nodes: BTreeMap::new(),
                generation: None,
            },
        );
        Ok(NativeImplicitFieldHandle { value })
    })
}
unsafe extern "C" fn destroy_field(context: *mut c_void, field: NativeImplicitFieldHandle) -> i32 {
    call(context, &mut (), |b| {
        b.stage()?
            .fields
            .remove(&field.value)
            .ok_or_else(|| error("unknown implicit field"))?;
        Ok(())
    })
}
unsafe extern "C" fn create_sampled_volume(
    context: *mut c_void,
    request: NativeSampledVolumeCreateRequest,
    result: *mut NativeSampledVolumeHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, result, receipt, b"CreateSampledVolume", |b| {
        let volume = SampledVolume::new(
            v(request.origin),
            request.spacing,
            [request.width, request.height, request.depth],
            request.initial_value,
        )
        .map_err(kernel)?;
        let value = b.allocate_volume()?;
        b.stage()?.volumes.insert(
            value,
            RetainedVolume {
                volume: Arc::new(volume),
                generation: None,
            },
        );
        Ok(NativeSampledVolumeHandle { value })
    })
}
unsafe extern "C" fn destroy_sampled_volume(
    context: *mut c_void,
    volume: NativeSampledVolumeHandle,
) -> i32 {
    call(context, &mut (), |b| {
        b.stage()?
            .volumes
            .remove(&volume.value)
            .ok_or_else(|| error("unknown sampled volume"))?;
        Ok(())
    })
}
unsafe extern "C" fn describe_sampled_volume(
    context: *mut c_void,
    volume: NativeSampledVolumeHandle,
    result: *mut NativeSampledVolumeDescriptor,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, result, receipt, b"DescribeSampledVolume", |b| {
        Ok(native_volume_descriptor(
            b.retained_volume(volume)?.volume.descriptor(),
        ))
    })
}
unsafe extern "C" fn write_sampled_volume(
    context: *mut c_void,
    request: *const NativeSampledVolumeWriteRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, &mut (), receipt, b"WriteSampledVolume", |b| {
        if request.is_null() {
            return Err(error("sampled volume write request was null"));
        }
        let request = unsafe { &*request };
        let samples = unsafe {
            borrowed_slice(
                request.samples,
                request.samples_len,
                "sampled volume write samples",
            )
        }?;
        let samples: Vec<_> = samples.iter().map(|sample| sample.value).collect();
        let retained = b.retained_volume(request.volume)?;
        Arc::make_mut(&mut retained.volume)
            .write(request.start as usize, &samples)
            .map_err(kernel)?;
        retained.generation = None;
        Ok(())
    })
}
unsafe extern "C" fn read_sampled_volume(
    context: *mut c_void,
    request: NativeSampledVolumeReadRequest,
    result: *mut NativeDensitySnapshotLease,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, result, receipt, b"ReadSampledVolume", |b| {
        let (descriptor, samples) = {
            let retained = b.retained_volume(request.volume)?;
            let samples = retained
                .volume
                .read(request.start as usize, request.count as usize)
                .map_err(kernel)?
                .iter()
                .copied()
                .map(|value| NativeDensitySample { value })
                .collect();
            (retained.volume.descriptor(), samples)
        };
        b.retain_density_snapshot(descriptor, request.start, samples)
    })
}
unsafe extern "C" fn destroy_density_snapshot_lease(
    context: *mut c_void,
    handle: NativeDensitySnapshotLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeImplicitBridge>() };
    i32::from(
        handle.value != 0
            && bridge
                .density_snapshot_leases
                .remove(&handle.value)
                .is_some(),
    )
}
unsafe extern "C" fn sample_sampled_volume(
    context: *mut c_void,
    request: NativeSampledVolumeSampleRequest,
    result: *mut NativeDensitySample,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, result, receipt, b"SampleSampledVolume", |b| {
        let value = b
            .retained_volume(request.volume)?
            .volume
            .sample(v(request.position))
            .map_err(kernel)?;
        Ok(NativeDensitySample { value })
    })
}
unsafe extern "C" fn rasterize_sampled_volume(
    context: *mut c_void,
    request: NativeSampledVolumeRasterizeRequest,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, &mut (), receipt, b"RasterizeSampledVolume", |b| {
        let (field, source) = {
            let retained = b.retained(request.field)?;
            (retained.field.clone(), retained.node(request.source)?)
        };
        let retained = b.retained_volume(request.volume)?;
        Arc::make_mut(&mut retained.volume)
            .rasterize(&field, source)
            .map_err(kernel)?;
        retained.generation = None;
        Ok(())
    })
}
unsafe extern "C" fn generate_sampled_volume(
    context: *mut c_void,
    request: *const NativeSampledVolumeGenerateRequest,
    result: *mut NativeMeshResourceHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, result, receipt, b"GenerateSampledVolume", |b| {
        if request.is_null() {
            return Err(error("sampled volume generate request was null"));
        }
        let request = unsafe { &*request };
        let regions = unsafe {
            borrowed_slice(
                request.regions,
                request.regions_len,
                "sampled volume material regions",
            )
        }?;
        if regions.len() > 255 {
            return Err(error("at most 255 material regions are supported per mesh"));
        }
        let (field, region_nodes) = {
            let retained = b.retained(request.field)?;
            let region_nodes = regions
                .iter()
                .map(|region| Ok((retained.node(region.node)?, region.material)))
                .collect::<Result<Vec<_>>>()?;
            (retained.field.clone(), region_nodes)
        };
        let started = Instant::now();
        let geometry = b
            .retained_volume(request.volume)?
            .volume
            .generate(request.isovalue, 262_144, 262_144)
            .map_err(kernel)?;
        let (mesh, readout) = unsafe {
            b.admit_geometry(
                &field,
                &geometry,
                &region_nodes,
                SurfaceGenerationOptions {
                    crease_angle_degrees: request.crease_angle_degrees,
                    uv_scale: request.uv_scale,
                    default_material: request.default_material,
                    material_boundary_mode: request.material_boundary_mode,
                    material_sample_spacing: request.material_sample_spacing,
                },
                started.elapsed().as_secs_f64(),
            )
        }?;
        b.retained_volume(request.volume)?.generation = Some(readout);
        Ok(mesh)
    })
}
unsafe extern "C" fn read_sampled_volume_generation(
    context: *mut c_void,
    volume: NativeSampledVolumeHandle,
    result: *mut NativeImplicitGenerationReadout,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(
        context,
        result,
        receipt,
        b"ReadSampledVolumeGeneration",
        |b| {
            b.retained_volume(volume)?
                .generation
                .ok_or_else(|| error("sampled volume has not produced a mesh"))
        },
    )
}
unsafe extern "C" fn generate(
    context: *mut c_void,
    request: *const NativeImplicitGenerateRequest,
    result: *mut NativeMeshResourceHandle,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    if receipt.is_null() {
        return 0;
    }
    unsafe {
        *receipt = std::mem::zeroed();
    }
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeImplicitBridge>() };
    // Generate validates/extracts before publishing a retained mesh. Expected
    // rejections belong to this operation, so a managed caller may catch them
    // and keep its prior scene. A panic still poisons the owning callback.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        bridge.generate(&*request)
    })) {
        Ok(Ok(mesh)) => {
            unsafe {
                *result = mesh;
            }
            ABI_OK
        }
        Ok(Err(error)) if error.code() == "CSHARP_SPATIAL_POINTER" => {
            // Pointer/length incoherence is an ABI failure, not an authoring
            // request the product may catch and continue past.
            bridge.callback_error = Some(error);
            0
        }
        Ok(Err(error)) => {
            bridge.retain_operation_error(b"Generate", error, receipt);
            0
        }
        Err(_) => {
            bridge.callback_error = Some(error("implicit backend panicked during generation"));
            0
        }
    }
}

struct GenerationDiagnosticLease {
    _code: Box<str>,
    _message: Box<str>,
    diagnostic: NativeEngineDiagnostic,
}

fn native_utf8(value: &[u8]) -> NativeUtf8Slice {
    NativeUtf8Slice {
        bytes: value.as_ptr(),
        len: value.len(),
    }
}

impl RuntimeImplicitBridge {
    fn retain_operation_error(
        &mut self,
        operation: &'static [u8],
        failure: CsharpEngineServicesError,
        receipt: *mut NativeOperationErrorReceipt,
    ) {
        let value = self.next_diagnostic_lease;
        let Some(next) = value.checked_add(1) else {
            self.callback_error = Some(error("implicit diagnostic identity exhausted"));
            return;
        };
        let code: Box<str> = failure.code().into();
        let message: Box<str> = failure.detail().into();
        // Box the lease itself so additional leases cannot move this readout
        // while a native caller retains its pointer until exact release.
        let lease = Box::new(GenerationDiagnosticLease {
            diagnostic: NativeEngineDiagnostic {
                code: native_utf8(code.as_bytes()),
                message: native_utf8(message.as_bytes()),
                source: native_utf8(b""),
            },
            _code: code,
            _message: message,
        });
        let diagnostics = NativeEngineDiagnosticLease {
            handle: NativeEngineDiagnosticLeaseHandle { value },
            diagnostics: std::ptr::from_ref(&lease.diagnostic),
            diagnostics_len: 1,
        };
        self.diagnostic_leases.insert(value, lease);
        self.next_diagnostic_lease = next;
        unsafe {
            *receipt = NativeOperationErrorReceipt {
                service: native_utf8(b"ImplicitSurfaces"),
                operation: native_utf8(operation),
                status: 0,
                diagnostics,
            };
        }
    }
}

unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeImplicitBridge>() };
    i32::from(handle.value != 0 && bridge.diagnostic_leases.remove(&handle.value).is_some())
}
unsafe extern "C" fn read_generation(
    context: *mut c_void,
    field: NativeImplicitFieldHandle,
    result: *mut NativeImplicitGenerationReadout,
) -> i32 {
    call(context, result, |b| {
        b.retained(field)?
            .generation
            .ok_or_else(|| error("field has not produced a mesh"))
    })
}
unsafe extern "C" fn sample(
    context: *mut c_void,
    r: NativeImplicitSampleRequest,
    result: *mut NativeImplicitSample,
) -> i32 {
    call(context, result, |b| {
        let source = b.node(r.field, r.source)?;
        let values = b
            .retained(r.field)?
            .field
            .sample(source, &[v(r.position)])
            .map_err(kernel)?;
        Ok(NativeImplicitSample { value: values[0] })
    })
}
unsafe extern "C" fn add_box(
    context: *mut c_void,
    r: NativeImplicitBoxRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        b.edit(r.field, |f| {
            f.box_shape(Bounds {
                min: v(r.minimum),
                max: v(r.maximum),
            })
        })
    })
}
unsafe extern "C" fn add_sphere(
    context: *mut c_void,
    r: NativeImplicitSphereRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        b.edit(r.field, |f| f.sphere(v(r.center), r.radius))
    })
}
unsafe extern "C" fn add_ellipsoid(
    context: *mut c_void,
    r: NativeImplicitEllipsoidRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        b.edit(r.field, |f| f.ellipsoid(v(r.center), v(r.radii)))
    })
}
unsafe extern "C" fn add_capsule(
    context: *mut c_void,
    r: NativeImplicitCapsuleRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        b.edit(r.field, |f| f.capsule(v(r.start), v(r.end), r.radius))
    })
}
unsafe extern "C" fn add_frustum(
    context: *mut c_void,
    r: NativeImplicitFrustumRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        b.edit(r.field, |f| {
            f.frustum(v(r.start), v(r.end), r.start_radius, r.end_radius)
        })
    })
}
unsafe extern "C" fn add_plane(
    context: *mut c_void,
    r: NativeImplicitPlaneRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        b.edit(r.field, |f| f.plane(v(r.normal), r.offset))
    })
}
unsafe extern "C" fn union(
    context: *mut c_void,
    r: NativeImplicitBinaryRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        let left = b.node(r.field, r.left)?;
        let right = b.node(r.field, r.right)?;
        b.edit(r.field, |f| f.union(left, right))
    })
}
unsafe extern "C" fn intersection(
    context: *mut c_void,
    r: NativeImplicitBinaryRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        let left = b.node(r.field, r.left)?;
        let right = b.node(r.field, r.right)?;
        b.edit(r.field, |f| f.intersection(left, right))
    })
}
unsafe extern "C" fn difference(
    context: *mut c_void,
    r: NativeImplicitBinaryRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        let left = b.node(r.field, r.left)?;
        let right = b.node(r.field, r.right)?;
        b.edit(r.field, |f| f.difference(left, right))
    })
}
unsafe extern "C" fn smooth_union(
    context: *mut c_void,
    r: NativeImplicitBlendRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        let left = b.node(r.field, r.left)?;
        let right = b.node(r.field, r.right)?;
        b.edit(r.field, |f| f.smooth_union(left, right, r.radius))
    })
}
unsafe extern "C" fn offset(
    context: *mut c_void,
    r: NativeImplicitOffsetRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        let source = b.node(r.field, r.source)?;
        b.edit(r.field, |f| f.offset(source, r.amount))
    })
}
unsafe extern "C" fn transform(
    context: *mut c_void,
    r: NativeImplicitTransformRequest,
    result: *mut NativeImplicitNode,
) -> i32 {
    call(context, result, |b| {
        let source = b.node(r.field, r.source)?;
        b.edit(r.field, |f| {
            f.transform_trs(
                source,
                v(r.transform.translation),
                [
                    r.transform.rotation.x,
                    r.transform.rotation.y,
                    r.transform.rotation.z,
                    r.transform.rotation.w,
                ],
                v(r.transform.scale),
            )
        })
    })
}

#[cfg(test)]
#[path = "implicit_surfaces_tests.rs"]
mod tests;
