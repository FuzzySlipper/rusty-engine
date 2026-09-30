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
    surface::{
        self, MaterialBoundaryMode, MaterialRegion, MaterialSampling, SurfaceOptions,
        TextureMapping, TextureProjection,
    },
    volume::{SampledVolume, VolumeDescriptor},
    Bounds, Field, GenerateOptions, Geometry, Node,
};

#[path = "implicit_audit.rs"]
mod audit_bridge;
use audit_bridge::*;

const DEFAULT_EXTRACTION_CAPACITY: u32 = 262_144;
fn extraction_capacity(requested: u32) -> u32 {
    if requested == 0 {
        DEFAULT_EXTRACTION_CAPACITY
    } else {
        requested
    }
}

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

#[derive(Clone, Copy)]
struct SurfaceGenerationOptions {
    crease_angle_degrees: f32,
    texture_mapping: TextureMapping,
    default_material: NativeMaterialHandle,
    material_boundary_mode: NativeImplicitMaterialBoundaryMode,
    material_sample_spacing: f32,
}

fn texture_mapping(legacy_uv_scale: f32, mapping: NativeImplicitTextureMapping) -> TextureMapping {
    // Existing generated constructors supply a disabled mapping. Keep their
    // established major-axis chart and scalar UV output exactly intact.
    if !mapping.enabled {
        return TextureMapping::legacy(legacy_uv_scale);
    }
    let u_axis = v(mapping.u_axis);
    let v_axis = v(mapping.v_axis);
    let scale = [mapping.scale.x, mapping.scale.y];
    let offset = [mapping.offset.x, mapping.offset.y];
    TextureMapping {
        projection: match mapping.projection {
            NativeImplicitTextureProjection::MajorAxis => TextureProjection::MajorAxis,
            NativeImplicitTextureProjection::Basis => TextureProjection::Basis { u_axis, v_axis },
        },
        scale,
        offset,
    }
}

#[derive(Clone, Default)]
pub(crate) struct RuntimeImplicitCall {
    fields: BTreeMap<u64, RetainedField>,
    audits: BTreeMap<u64, AuditCollection>,
    volumes: BTreeMap<u64, RetainedVolume>,
}
pub(crate) struct RuntimeImplicitBridge {
    state: RuntimeImplicitCall,
    staged: Option<RuntimeImplicitCall>,
    next_field: u64,
    // Public node identities are not the local svc-implicit arena indices.
    next_node: u64,
    // Volume identities are independent from field/node identities.
    next_volume: u64,
    appearance: Option<*mut RuntimeAppearanceBridge>,
    operation_diagnostics: crate::operation_diagnostics::OperationDiagnostics,
    next_audit: u64,
    borrowed: crate::operation_diagnostics::BorrowedResult,
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
            operation_diagnostics: Default::default(),
            next_audit: 1,
            borrowed: Default::default(),
        }
    }
    pub(crate) fn begin_call(&mut self) {
        // The call owns the state until it finishes; nothing is copied.
        self.staged = Some(std::mem::take(&mut self.state));
    }
    pub(crate) fn take_call(&mut self) -> Result<RuntimeImplicitCall> {
        self.staged
            .take()
            .ok_or_else(|| error("implicit call was not staged"))
    }
    pub(crate) fn commit_call(&mut self, call: RuntimeImplicitCall) {
        self.state = call;
    }
    /// Ends the open call, keeping its state.
    #[cfg(test)]
    pub(crate) fn end_call(&mut self) {
        let call = self.take_call().expect("an open implicit call");
        self.commit_call(call);
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
    ) -> Result<NativeDensitySnapshotResult> {
        let result = NativeDensitySnapshotResult {
            descriptor: native_volume_descriptor(descriptor),
            start,
            samples: samples.as_ptr(),
            samples_len: samples.len(),
        };
        self.borrowed.hold(samples);
        Ok(result)
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
        started: Instant,
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
                texture_mapping: options.texture_mapping,
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
                generation_seconds: started.elapsed().as_secs_f64(),
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
                    max_vertices: extraction_capacity(request.max_extraction_vertices),
                    max_triangles: extraction_capacity(request.max_extraction_triangles),
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
                    texture_mapping: texture_mapping(request.uv_scale, request.texture_mapping),
                    default_material: request.default_material,
                    material_boundary_mode: request.material_boundary_mode,
                    material_sample_spacing: request.material_sample_spacing,
                },
                started,
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
        displace_waves,
        offset,
        transform,
        sample,
        generate,
        read_generation,
        create_audit,
        destroy_audit,
        capture_audit_piece,
        read_audit,
        read_mesh_integrity,
        read_expected_join,
        read_enclosure,
    }
}
fn call<T>(
    context: *mut c_void,
    result: *mut T,
    operation_error: *mut NativeOperationErrorReceipt,
    action: impl FnOnce(&mut RuntimeImplicitBridge) -> Result<T>,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeImplicitBridge>() };
    // Fidget/JIT failures must not unwind through the ABI. A panic is reported
    // as the operation's error, like any other refusal.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| action(bridge))) {
        Ok(Ok(value)) => {
            unsafe { *result = value };
            ABI_OK
        }
        Ok(Err(e)) => {
            if !operation_error.is_null() {
                bridge.operation_diagnostics.retain(&e, operation_error);
            }
            0
        }
        Err(_) => {
            bridge.operation_diagnostics.retain(
                &error("implicit backend panicked during generation or evaluation"),
                operation_error,
            );
            0
        }
    }
}
fn call_operation<T>(
    context: *mut c_void,
    result: *mut T,
    receipt: *mut NativeOperationErrorReceipt,
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
        Ok(Err(error)) => {
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
        Err(_) => {
            bridge.operation_diagnostics.retain(
                &error("implicit backend panicked during operation"),
                receipt,
            );
            0
        }
    }
}
unsafe extern "C" fn create_field(
    context: *mut c_void,
    result: *mut NativeImplicitFieldHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
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
unsafe extern "C" fn destroy_field(
    context: *mut c_void,
    field: NativeImplicitFieldHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, &mut (), operation_error, |b| {
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
    call_operation(context, result, receipt, |b| {
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
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, &mut (), operation_error, |b| {
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
    call_operation(context, result, receipt, |b| {
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
    call_operation(context, &mut (), receipt, |b| {
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
    result: *mut NativeDensitySnapshotResult,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, result, receipt, |b| {
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
unsafe extern "C" fn sample_sampled_volume(
    context: *mut c_void,
    request: NativeSampledVolumeSampleRequest,
    result: *mut NativeDensitySample,
    receipt: *mut NativeOperationErrorReceipt,
) -> i32 {
    call_operation(context, result, receipt, |b| {
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
    call_operation(context, &mut (), receipt, |b| {
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
    call_operation(context, result, receipt, |b| {
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
                    texture_mapping: TextureMapping::legacy(request.uv_scale),
                    default_material: request.default_material,
                    material_boundary_mode: request.material_boundary_mode,
                    material_sample_spacing: request.material_sample_spacing,
                },
                started,
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
    call_operation(context, result, receipt, |b| {
        b.retained_volume(volume)?
            .generation
            .ok_or_else(|| error("sampled volume has not produced a mesh"))
    })
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
    // and keep its prior scene. A panic is caught here and reported as this
    // operation's error too, which C# sees as an `EngineCallException` like
    // any other refusal.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        bridge.generate(&*request)
    })) {
        Ok(Ok(mesh)) => {
            unsafe {
                *result = mesh;
            }
            ABI_OK
        }
        Ok(Err(error)) => {
            bridge.operation_diagnostics.retain(&error, receipt);
            0
        }
        Err(_) => {
            bridge.operation_diagnostics.retain(
                &error("implicit backend panicked during generation"),
                receipt,
            );
            0
        }
    }
}

unsafe extern "C" fn read_generation(
    context: *mut c_void,
    field: NativeImplicitFieldHandle,
    result: *mut NativeImplicitGenerationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        b.retained(field)?
            .generation
            .ok_or_else(|| error("field has not produced a mesh"))
    })
}
unsafe extern "C" fn sample(
    context: *mut c_void,
    r: NativeImplicitSampleRequest,
    result: *mut NativeImplicitSample,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
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
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
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
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        b.edit(r.field, |f| f.sphere(v(r.center), r.radius))
    })
}
unsafe extern "C" fn add_ellipsoid(
    context: *mut c_void,
    r: NativeImplicitEllipsoidRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        b.edit(r.field, |f| f.ellipsoid(v(r.center), v(r.radii)))
    })
}
unsafe extern "C" fn add_capsule(
    context: *mut c_void,
    r: NativeImplicitCapsuleRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        b.edit(r.field, |f| f.capsule(v(r.start), v(r.end), r.radius))
    })
}
unsafe extern "C" fn add_frustum(
    context: *mut c_void,
    r: NativeImplicitFrustumRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        b.edit(r.field, |f| {
            f.frustum(v(r.start), v(r.end), r.start_radius, r.end_radius)
        })
    })
}
unsafe extern "C" fn add_plane(
    context: *mut c_void,
    r: NativeImplicitPlaneRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        b.edit(r.field, |f| f.plane(v(r.normal), r.offset))
    })
}
unsafe extern "C" fn union(
    context: *mut c_void,
    r: NativeImplicitBinaryRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        let left = b.node(r.field, r.left)?;
        let right = b.node(r.field, r.right)?;
        b.edit(r.field, |f| f.union(left, right))
    })
}
unsafe extern "C" fn intersection(
    context: *mut c_void,
    r: NativeImplicitBinaryRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        let left = b.node(r.field, r.left)?;
        let right = b.node(r.field, r.right)?;
        b.edit(r.field, |f| f.intersection(left, right))
    })
}
unsafe extern "C" fn difference(
    context: *mut c_void,
    r: NativeImplicitBinaryRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        let left = b.node(r.field, r.left)?;
        let right = b.node(r.field, r.right)?;
        b.edit(r.field, |f| f.difference(left, right))
    })
}
unsafe extern "C" fn smooth_union(
    context: *mut c_void,
    r: NativeImplicitBlendRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        let left = b.node(r.field, r.left)?;
        let right = b.node(r.field, r.right)?;
        b.edit(r.field, |f| f.smooth_union(left, right, r.radius))
    })
}
unsafe extern "C" fn displace_waves(
    context: *mut c_void,
    r: NativeImplicitWaveRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        let source = b.node(r.field, r.source)?;
        b.edit(r.field, |f| {
            f.displace_waves(
                source,
                svc_implicit::WaveDisplacement {
                    frequency: v(r.frequency),
                    amplitude: r.amplitude,
                    octaves: r.octaves,
                    lacunarity: r.lacunarity,
                    gain: r.gain,
                    seed: r.seed,
                },
            )
        })
    })
}
unsafe extern "C" fn offset(
    context: *mut c_void,
    r: NativeImplicitOffsetRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
        let source = b.node(r.field, r.source)?;
        b.edit(r.field, |f| f.offset(source, r.amount))
    })
}
unsafe extern "C" fn transform(
    context: *mut c_void,
    r: NativeImplicitTransformRequest,
    result: *mut NativeImplicitNode,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    call(context, result, operation_error, |b| {
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
