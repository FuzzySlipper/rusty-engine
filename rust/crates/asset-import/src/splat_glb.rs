//! Splat primitives in an animated GLB (`docs/splat-characters.md`): a
//! `POINTS` primitive whose points are Gaussians, with `POSITION` (centres),
//! `_ROTATION` (xyzw), `_SCALE` (standard deviations in metres) and `_COLOR`
//! (sRGB-encoded rgb, linear opacity), skinned by `JOINTS_0`/`WEIGHTS_0`
//! when the node that draws it has a skin. The renderer reads the bytes as
//! admitted (`render-wgpu` `glb.rs`); admission measures the bounds.

use voxel_convert::{MeshSourceBounds, ParsedGltf};

use crate::{ImportCode, ImportDiagnostic};

/// How far past its centre a Gaussian's bounds reach, in standard
/// deviations: its drawn quad's reach, 2·sqrt(2)σ, rounded up.
const BOUNDS_SIGMAS: f64 = 3.0;

/// The attributes a splat primitive carries besides `POSITION`.
const SPLAT_ATTRIBUTES: [&str; 3] = ["ROTATION", "SCALE", "COLOR"];

/// Whether `primitive` is a splat primitive: `POINTS` with `_SCALE`.
pub(crate) fn is_splat_primitive(primitive: &gltf::Primitive<'_>) -> bool {
    primitive.mode() == gltf::mesh::Mode::Points
        && primitive
            .get(&gltf::Semantic::Extras("SCALE".to_owned()))
            .is_some()
}

/// `None` for a GLB without splat primitives. For one with them: the
/// Gaussians' bounds at rest, every node's splats in its model space and a
/// skinned node's in bind space, or why the GLB is refused.
pub(crate) fn measure(
    parsed: &ParsedGltf<'_>,
) -> Option<Result<MeshSourceBounds, ImportDiagnostic>> {
    let document = &parsed.document;
    let splats = document
        .meshes()
        .flat_map(|mesh| mesh.primitives().collect::<Vec<_>>())
        .any(|primitive| is_splat_primitive(&primitive));
    if !splats {
        return None;
    }
    Some(measure_splats(parsed))
}

fn measure_splats(parsed: &ParsedGltf<'_>) -> Result<MeshSourceBounds, ImportDiagnostic> {
    let document = &parsed.document;
    if document.animations().next().is_some() {
        return Err(ImportDiagnostic::error(
            ImportCode::UnsupportedFeature,
            "source.animations",
            "a GLB with splat primitives does not carry clips of its own yet",
            "export the clips as a clip pack and the splats without animations",
        ));
    }
    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let path = format!(
                "source.meshes[{}].primitives[{}]",
                mesh.index(),
                primitive.index()
            );
            if !is_splat_primitive(&primitive) {
                return Err(ImportDiagnostic::error(
                    ImportCode::UnsupportedFeature,
                    path,
                    "a GLB with splat primitives has only splat primitives",
                    "export triangle meshes in a GLB of their own",
                ));
            }
            check_splat_primitive(&primitive, &path)?;
        }
    }
    let blob = parsed.blob.as_deref();
    let data = |buffer: gltf::Buffer<'_>| match buffer.source() {
        gltf::buffer::Source::Bin => blob,
        gltf::buffer::Source::Uri(_) => None,
    };
    let parents = parents(document);
    let mut min = [f64::MAX; 3];
    let mut max = [f64::MIN; 3];
    for node in document.nodes() {
        let Some(mesh) = node.mesh() else {
            continue;
        };
        let model = if node.skin().is_some() {
            IDENTITY
        } else {
            model_matrix(document, &parents, node.index())
        };
        for primitive in mesh.primitives() {
            let attribute = |semantic: gltf::Semantic| {
                primitive
                    .get(&semantic)
                    .expect("checked by check_splat_primitive")
            };
            let centres = floats::<3>(attribute(gltf::Semantic::Positions), data)?;
            let scales = floats::<3>(attribute(gltf::Semantic::Extras("SCALE".to_owned())), data)?;
            for (centre, scale) in centres.iter().zip(&scales) {
                let reach = BOUNDS_SIGMAS * scale.iter().fold(0.0f64, |a, b| a.max(b.abs()));
                let point = transform_point(&model, *centre);
                // The reach is a sphere; under the model's scale it reaches
                // as far as its largest axis stretches it.
                let reach = reach * max_axis_scale(&model);
                for axis in 0..3 {
                    min[axis] = min[axis].min(point[axis] - reach);
                    max[axis] = max[axis].max(point[axis] + reach);
                }
            }
        }
    }
    if min[0] > max[0] {
        return Err(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            "source.nodes",
            "no node draws the GLB's splat primitives",
            "place the splat mesh on a node of the scene",
        ));
    }
    Ok(MeshSourceBounds { min, max })
}

fn check_splat_primitive(
    primitive: &gltf::Primitive<'_>,
    path: &str,
) -> Result<(), ImportDiagnostic> {
    let Some(positions) = primitive.get(&gltf::Semantic::Positions) else {
        return Err(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            format!("{path}.attributes.POSITION"),
            "a splat primitive needs POSITION, its Gaussians' centres",
            "write POSITION, _ROTATION, _SCALE and _COLOR for every Gaussian",
        ));
    };
    if positions.count() == 0 {
        return Err(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            format!("{path}.attributes.POSITION"),
            "a splat primitive has no Gaussians",
            "remove the empty primitive",
        ));
    }
    for name in SPLAT_ATTRIBUTES.iter().chain(&OPTIONAL_SPLAT_ATTRIBUTES) {
        let Some(accessor) = primitive.get(&gltf::Semantic::Extras((*name).to_owned())) else {
            if OPTIONAL_SPLAT_ATTRIBUTES.contains(name) {
                continue;
            }
            return Err(ImportDiagnostic::error(
                ImportCode::MalformedSource,
                format!("{path}.attributes._{name}"),
                format!("a splat primitive needs _{name}"),
                "write POSITION, _ROTATION, _SCALE and _COLOR for every Gaussian",
            ));
        };
        if accessor.count() != positions.count() {
            return Err(ImportDiagnostic::error(
                ImportCode::AttributeLengthMismatch,
                format!("{path}.attributes._{name}"),
                format!(
                    "_{name} has {} values for {} Gaussians",
                    accessor.count(),
                    positions.count()
                ),
                "write one value of each attribute per Gaussian",
            ));
        }
    }
    Ok(())
}

/// Attributes a splat primitive may carry: `_NORMAL`, each Gaussian's
/// surface normal, for lighting.
const OPTIONAL_SPLAT_ATTRIBUTES: [&str; 1] = ["NORMAL"];

const IDENTITY: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

fn parents(document: &gltf::Document) -> Vec<Option<usize>> {
    let mut parents = vec![None; document.nodes().count()];
    for node in document.nodes() {
        for child in node.children() {
            parents[child.index()] = Some(node.index());
        }
    }
    parents
}

/// A node's transform to the scene's space at rest, column-major.
fn model_matrix(document: &gltf::Document, parents: &[Option<usize>], node: usize) -> [f64; 16] {
    let mut matrix = IDENTITY;
    let mut current = Some(node);
    // Guard against a malformed cycle: no chain is longer than the nodes.
    let mut steps = parents.len();
    while let (Some(index), true) = (current, steps > 0) {
        steps -= 1;
        let local = document
            .nodes()
            .nth(index)
            .map(|node| node.transform().matrix())
            .unwrap_or([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]);
        let local: Vec<f64> = local.iter().flatten().map(|v| f64::from(*v)).collect();
        matrix = multiply(local.as_slice().try_into().expect("16 values"), &matrix);
        current = parents[index];
    }
    matrix
}

/// `a · b`, column-major.
fn multiply(a: &[f64; 16], b: &[f64; 16]) -> [f64; 16] {
    let mut out = [0.0; 16];
    for column in 0..4 {
        for row in 0..4 {
            out[column * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[column * 4 + k]).sum();
        }
    }
    out
}

fn transform_point(matrix: &[f64; 16], point: [f64; 3]) -> [f64; 3] {
    let mut out = [0.0; 3];
    for (row, value) in out.iter_mut().enumerate() {
        *value = matrix[row] * point[0]
            + matrix[4 + row] * point[1]
            + matrix[8 + row] * point[2]
            + matrix[12 + row];
    }
    out
}

fn max_axis_scale(matrix: &[f64; 16]) -> f64 {
    (0..3)
        .map(|column| {
            (0..3)
                .map(|row| matrix[column * 4 + row].powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .fold(0.0, f64::max)
}

/// An accessor of `N` components as `f64`: floats as they are, normalized
/// integers mapped to [0, 1] or [−1, 1].
fn floats<'a, 's, const N: usize>(
    accessor: gltf::Accessor<'a>,
    data: impl Clone + Fn(gltf::Buffer<'a>) -> Option<&'s [u8]>,
) -> Result<Vec<[f64; N]>, ImportDiagnostic>
where
    [f32; N]: gltf::accessor::Item,
    [u8; N]: gltf::accessor::Item,
    [u16; N]: gltf::accessor::Item,
    [i8; N]: gltf::accessor::Item,
    [i16; N]: gltf::accessor::Item,
{
    use gltf::accessor::{DataType, Iter};
    let index = accessor.index();
    let path = format!("source.accessors[{index}]");
    let refused = |message: String| {
        ImportDiagnostic::error(
            ImportCode::MalformedSource,
            path.clone(),
            message,
            "write splat attributes as floats or normalized integers in the BIN chunk",
        )
    };
    if accessor.dimensions().multiplicity() != N {
        return Err(refused(format!("accessor {index} is not {N} components")));
    }
    let normalized = accessor.normalized();
    let missing = || refused(format!("accessor {index} has no data"));
    let values: Vec<[f64; N]> = match accessor.data_type() {
        DataType::F32 => Iter::<[f32; N]>::new(accessor, data)
            .ok_or_else(missing)?
            .map(|value| value.map(f64::from))
            .collect(),
        DataType::U8 if normalized => Iter::<[u8; N]>::new(accessor, data)
            .ok_or_else(missing)?
            .map(|value| value.map(|c| f64::from(c) / 255.0))
            .collect(),
        DataType::U16 if normalized => Iter::<[u16; N]>::new(accessor, data)
            .ok_or_else(missing)?
            .map(|value| value.map(|c| f64::from(c) / 65535.0))
            .collect(),
        DataType::I8 if normalized => Iter::<[i8; N]>::new(accessor, data)
            .ok_or_else(missing)?
            .map(|value| value.map(|c| (f64::from(c) / 127.0).max(-1.0)))
            .collect(),
        DataType::I16 if normalized => Iter::<[i16; N]>::new(accessor, data)
            .ok_or_else(missing)?
            .map(|value| value.map(|c| (f64::from(c) / 32767.0).max(-1.0)))
            .collect(),
        other => {
            return Err(refused(format!(
                "accessor {index} is {other:?}{}, not a float or normalized integer",
                if normalized { " normalized" } else { "" }
            )))
        }
    };
    if values.iter().flatten().any(|value| !value.is_finite()) {
        return Err(ImportDiagnostic::error(
            ImportCode::NonFiniteValue,
            path,
            format!("accessor {index} holds a non-finite value"),
            "write finite splat attributes",
        ));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use crate::{import_animated_glb_asset, ImportCode, ImportContext, SourceUri};

    /// A GLB of two Gaussians, its primitive's attributes edited by `edit`
    /// and the document by `extra`.
    fn glb(
        edit: impl FnOnce(&mut serde_json::Value),
        extra: impl FnOnce(&mut serde_json::Value),
    ) -> Vec<u8> {
        let floats = |values: &[f32]| {
            values
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<u8>>()
        };
        let mut bin = floats(&[0.0, 1.0, 0.0, 0.2, 1.1, 0.0]);
        bin.extend(floats(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
        bin.extend(floats(&[0.05; 6]));
        bin.extend([200, 100, 50, 255, 50, 100, 200, 255]);
        let view = |offset: usize, length: usize| serde_json::json!({"buffer": 0, "byteOffset": offset, "byteLength": length});
        let mut attributes =
            serde_json::json!({"POSITION": 0, "_ROTATION": 1, "_SCALE": 2, "_COLOR": 3});
        edit(&mut attributes);
        let mut document = serde_json::json!({
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"name": "body", "mesh": 0, "translation": [0.0, 0.0, 2.0]}],
            "meshes": [{"primitives": [{"mode": 0, "attributes": attributes}]}],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 2, "type": "VEC3",
                 "min": [0.0, 1.0, 0.0], "max": [0.2, 1.1, 0.0]},
                {"bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC4"},
                {"bufferView": 2, "componentType": 5126, "count": 2, "type": "VEC3"},
                {"bufferView": 3, "componentType": 5121, "normalized": true, "count": 2,
                 "type": "VEC4"},
            ],
            "bufferViews": [view(0, 24), view(24, 32), view(56, 24), view(80, 8)],
            "buffers": [{"byteLength": bin.len()}],
        });
        extra(&mut document);
        let mut json = serde_json::to_vec(&document).unwrap();
        json.resize(json.len().div_ceil(4) * 4, b' ');
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&((28 + json.len() + bin.len()) as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    fn import(bytes: &[u8]) -> crate::AnimatedGlbImportOutcome {
        import_animated_glb_asset(
            &SourceUri::RelativePath("splats.glb".to_owned()),
            bytes,
            &ImportContext::default(),
        )
    }

    #[test]
    fn a_splat_glb_is_admitted_with_its_gaussians_bounds_in_model_space() {
        let outcome = import(&glb(|_| {}, |_| {}));
        let assets = outcome
            .assets
            .unwrap_or_else(|| panic!("admitted: {:?}", outcome.diagnostics));
        let bounds = assets.animated_mesh.bounds;
        // Centres to three standard deviations, moved by the node.
        assert_eq!(
            bounds.min.map(|v| (v * 1000.0).round()),
            [-150.0, 850.0, 1850.0]
        );
        assert_eq!(
            bounds.max.map(|v| (v * 1000.0).round()),
            [350.0, 1250.0, 2150.0]
        );
        assert!(assets.animated_mesh.clips.is_empty());
    }

    #[test]
    fn a_splat_primitive_without_its_colours_is_refused() {
        let outcome = import(&glb(
            |attributes| {
                attributes.as_object_mut().unwrap().remove("_COLOR");
            },
            |_| {},
        ));
        assert!(outcome.assets.is_none());
        assert!(outcome
            .diagnostics
            .iter()
            .any(|d| d.code == ImportCode::MalformedSource && d.message.contains("_COLOR")));
    }

    /// `_NORMAL` is optional, and when written has one value per Gaussian.
    #[test]
    fn a_splat_normal_stream_is_admitted_and_its_count_checked() {
        let with = import(&glb(
            |attributes| attributes["_NORMAL"] = serde_json::json!(2),
            |_| {},
        ));
        assert!(with.assets.is_some(), "{:?}", with.diagnostics);
        let short = import(&glb(
            |attributes| attributes["_NORMAL"] = serde_json::json!(4),
            |document| {
                document["accessors"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!(
                        {"bufferView": 2, "componentType": 5126, "count": 1, "type": "VEC3"}
                    ));
            },
        ));
        assert!(short.assets.is_none());
        assert!(short.diagnostics.iter().any(|d| {
            d.code == ImportCode::AttributeLengthMismatch && d.message.contains("_NORMAL")
        }));
    }

    #[test]
    fn triangles_beside_splats_and_embedded_clips_are_refused() {
        let triangles = import(&glb(
            |_| {},
            |document| {
                document["meshes"][0]["primitives"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({"attributes": {"POSITION": 0}}));
            },
        ));
        assert!(triangles.assets.is_none());
        assert!(triangles
            .diagnostics
            .iter()
            .any(|d| d.code == ImportCode::UnsupportedFeature));
        let clips = import(&glb(
            |_| {},
            |document| {
                document["animations"] =
                    serde_json::json!([{"name": "idle", "channels": [], "samplers": []}]);
            },
        ));
        assert!(clips.assets.is_none());
    }

    /// A skinned splat GLB without clips names its rig, held in place, so
    /// clip packs can bind to it and its joints can be read.
    #[test]
    fn a_skinned_splat_glb_names_its_rig() {
        let bytes = include_bytes!("../../../../fixtures/render/assets/splats/skinned-arm.glb");
        let outcome = import(bytes);
        let mesh = outcome
            .assets
            .unwrap_or_else(|| panic!("admitted: {:?}", outcome.diagnostics))
            .animated_mesh;
        let rig = mesh.rig.expect("a skinned splat GLB names its rig");
        let ids: Vec<&str> = rig.joints.iter().map(|joint| joint.id.as_str()).collect();
        assert_eq!(ids, ["arm", "root"]);
        assert_eq!(rig.structural_root_ids, ["root"]);
        assert_eq!(
            rig.root_convention,
            render_model::AnimationRootConvention::InPlace
        );
        assert!(mesh.clips.is_empty());
    }
}
