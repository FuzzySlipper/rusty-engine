//! Deformed meshes: a static mesh appearance whose instances draw a shape
//! the Engine replaces as it changes (a soft body's surface), with the
//! appearance's material. The shape is retained on the appearance, so every
//! object showing it, snapshot and capture draws the latest one.

use super::*;

/// One material slot, slot 0.
const SLOT: u16 = 0;

/// Triangles with smooth normals and UVs, as a soft body's surface is read.
pub(crate) struct DeformedShape<'a> {
    pub(crate) positions: &'a [[f32; 3]],
    pub(crate) normals: &'a [[f32; 3]],
    pub(crate) uvs: &'a [[f32; 2]],
    pub(crate) indices: &'a [u32],
    pub(crate) bounds_min: [f32; 3],
    pub(crate) bounds_max: [f32; 3],
}

impl DeformedShape<'_> {
    fn payload(&self) -> MeshPayloadDescriptor {
        let flat = |values: &[[f32; 3]]| values.iter().flatten().copied().collect::<Vec<_>>();
        MeshPayloadDescriptor {
            texture_space: None,
            distance_field: None,
            layout: MeshBufferLayout {
                vertex_count: self.positions.len() as u32,
                index_count: self.indices.len() as u32,
                index_width: MeshIndexWidth::U32,
                attributes: [
                    (MeshAttributeName::Position, 3),
                    (MeshAttributeName::Normal, 3),
                    (MeshAttributeName::Uv, 2),
                ]
                .into_iter()
                .map(|(name, components)| MeshAttribute {
                    name,
                    components,
                    kind: MeshAttributeKind::F32,
                })
                .collect(),
            },
            groups: vec![MeshGroupDescriptor {
                material_slot: SLOT,
                start: 0,
                count: self.indices.len() as u32,
            }],
            bounds: MeshBoundsDescriptor {
                min: self.bounds_min,
                max: self.bounds_max,
            },
            source: MeshPayloadSource::Inline {
                positions: flat(self.positions).into(),
                normals: flat(self.normals).into(),
                uvs: Some(
                    self.uvs
                        .iter()
                        .flatten()
                        .copied()
                        .collect::<Vec<_>>()
                        .into(),
                ),
                colors: None,
                indices: self.indices.to_vec().into(),
            },
            provenance: MeshProvenance::Generated,
            layer_weights: false,
            layer_palette: Vec::new(),
            vertex_occlusion: false,
            morph: None,
        }
    }
}

fn deformed_error(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_DEFORMED_MESH", message)
}

impl RuntimeAppearanceBridge {
    /// An appearance drawing `shape` with `material`, to be reshaped with
    /// [`Self::reshape_deformed`].
    pub(crate) fn create_deformed_appearance(
        &mut self,
        material: NativeMaterialHandle,
        shape: &DeformedShape<'_>,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        if shape.indices.is_empty() {
            return Err(deformed_error("the shape has no triangles to draw"));
        }
        let staged = self.staged_mut()?;
        let material_id = staged
            .state
            .material_id(material.value)
            .ok_or_else(|| deformed_error("material handle is not live"))?
            .to_owned();
        // Named as other per-appearance meshes are, so disposing the
        // appearance releases it.
        let handle = staged.state.next_appearance;
        let mesh_id = format!("mesh/native-{handle}");
        let payload = Arc::new(shape.payload());
        let asset = StaticMeshAsset {
            asset: mesh_id.clone(),
            payload: (*payload).clone(),
            material_slots: vec![MeshMaterialSlot {
                slot: SLOT,
                material: material_id,
            }],
            collision: MeshCollisionPolicy::VisualOnly,
        };
        asset
            .validate()
            .map_err(|error| deformed_error(format!("{error:?}")))?;
        staged.state.project_static_mesh(Arc::new(asset));
        let appearance = self.allocate_appearance(Appearance::StaticMesh {
            asset: mesh_id,
            material_overrides: Vec::new(),
            material_parameters: Default::default(),
            deformed: Some(payload),
        })?;
        self.staged_mut()?
            .state
            .bind_appearance_materials(appearance.value, BTreeSet::from([material.value]));
        Ok(appearance)
    }

    /// Gives a deformed appearance its new shape. False once the appearance
    /// is gone; an empty shape (everything torn away) keeps the last one.
    pub(crate) fn reshape_deformed(
        &mut self,
        appearance: u64,
        shape: &DeformedShape<'_>,
    ) -> Result<bool, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let Some(identity) = staged.state.appearances.get(&appearance).cloned() else {
            return Ok(false);
        };
        if shape.indices.is_empty() {
            return Ok(true);
        }
        match staged.state.projector.appearance_mut(&identity) {
            Some(Appearance::StaticMesh { deformed, .. }) => {
                *deformed = Some(Arc::new(shape.payload()));
                // Published as the call ends, whether or not the product
                // publishes its facts again.
                staged.resource_releases_pending = true;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

/// Shows `appearance` as object 7 at the origin in the open call.
#[cfg(test)]
pub(crate) fn show_for_test(
    bridge: &mut RuntimeAppearanceBridge,
    appearance: NativeAppearanceHandle,
) {
    let fact = super::tests::appearance_fact(appearance);
    unsafe { bridge.stage_snapshot(&fact, 1) }.expect("snapshot");
}

/// A plain white material for tests.
#[cfg(test)]
pub(crate) fn material_for_test(bridge: &mut RuntimeAppearanceBridge) -> NativeMaterialHandle {
    let white = NativeColor {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    bridge
        .create_material(NativeMaterialRequest {
            texture_scale: NativeVec2::default(),
            texture_offset: NativeVec2::default(),
            stochastic_tiling: 0.0,
            shader: Default::default(),
            triplanar_sharpness: 0.0,
            color: white,
            texture: NativeRenderResourceReference::default(),
            roughness: 0.7,
            texture_tint: white,
            emission_color: NativeVec3::default(),
            emission_intensity: 0.0,
            double_sided: false,
            alpha_mode: NativeMaterialAlphaMode::Opaque,
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
            sinks_under_snow: false,
            wind_bend: 0.0,
            wind_flutter: 0.0,
            water: Default::default(),
            translucent_shadow: false,
        })
        .expect("material")
}
