//! Splat primitives in an animated GLB (`docs/splat-characters.md`): a
//! `POINTS` primitive whose points are Gaussians, with `POSITION` (centres),
//! `_ROTATION` (xyzw), `_SCALE` (standard deviations in metres) and `_COLOR`
//! (sRGB-encoded rgb, linear opacity), or cards (`extras.splatKind =
//! "billboard"`): textured rectangles with `POSITION`, `_ROTATION`, `_SCALE`
//! (half-extents) and `_TILE_ORIGIN` (their tile of an embedded atlas). Either
//! is skinned by `JOINTS_0`/`WEIGHTS_0` when the node that draws it has a
//! skin. The renderer reads the bytes as admitted (`render-wgpu` `glb.rs`);
//! admission measures the bounds.

use voxel_convert::{MeshSourceBounds, ParsedGltf};

use crate::{ImportCode, ImportDiagnostic};

/// How far past its centre a Gaussian's bounds reach, in standard
/// deviations: its drawn quad's reach, 2·sqrt(2)σ, rounded up.
const BOUNDS_SIGMAS: f64 = 3.0;

/// The attributes a splat primitive carries besides `POSITION`.
const SPLAT_ATTRIBUTES: [&str; 3] = ["ROTATION", "SCALE", "COLOR"];

/// The attributes a card primitive carries besides `POSITION`.
const CARD_ATTRIBUTES: [&str; 3] = ["ROTATION", "SCALE", "TILE_ORIGIN"];

/// What a card primitive's `extras.splatKind` says.
const CARD_KIND: &str = "billboard";

/// Whether `primitive` is a splat figure's matte inset core: `TRIANGLES`
/// marked `extras.splatCore = true`.
pub(crate) fn is_core_primitive(primitive: &gltf::Primitive<'_>) -> bool {
    primitive.mode() == gltf::mesh::Mode::Triangles
        && extras(primitive).is_some_and(|extras| {
            extras.get("splatCore").and_then(|core| core.as_bool()) == Some(true)
        })
}

/// Whether `primitive` is a splat primitive: `POINTS` with `_SCALE`, or
/// `POINTS` marked as cards.
pub(crate) fn is_splat_primitive(primitive: &gltf::Primitive<'_>) -> bool {
    primitive.mode() == gltf::mesh::Mode::Points
        && (primitive
            .get(&gltf::Semantic::Extras("SCALE".to_owned()))
            .is_some()
            || is_card_primitive(primitive))
}

/// Whether `primitive` is a card primitive: `POINTS` with
/// `extras.splatKind = "billboard"`.
pub(crate) fn is_card_primitive(primitive: &gltf::Primitive<'_>) -> bool {
    primitive.mode() == gltf::mesh::Mode::Points
        && extras(primitive).is_some_and(|extras| {
            extras.get("splatKind").and_then(|kind| kind.as_str()) == Some(CARD_KIND)
        })
}

fn extras(primitive: &gltf::Primitive<'_>) -> Option<serde_json::Value> {
    primitive
        .extras()
        .as_ref()
        .and_then(|raw| serde_json::from_str(raw.get()).ok())
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
    let blob = parsed.blob.as_deref();
    let data = |buffer: gltf::Buffer<'_>| match buffer.source() {
        gltf::buffer::Source::Bin => blob,
        gltf::buffer::Source::Uri(_) => None,
    };
    // Cards draw in runs by their atlas, so one GLB's cards share one, and a
    // GLB is cards or Gaussians.
    let mut kinds = (false, false);
    let mut atlas = None;
    for mesh in document.meshes() {
        // A core stands inside its mesh's splats: one, beside them.
        let cores = mesh.primitives().filter(is_core_primitive).count();
        let splats = mesh
            .primitives()
            .any(|primitive| is_splat_primitive(&primitive));
        if cores > 1 || (cores == 1 && !splats) {
            return Err(ImportDiagnostic::error(
                ImportCode::UnsupportedFeature,
                format!("source.meshes[{}]", mesh.index()),
                "a splat mesh has at most one core, beside its splats",
                "export one core primitive in the mesh of the splats it fills",
            ));
        }
        for primitive in mesh.primitives() {
            let path = format!(
                "source.meshes[{}].primitives[{}]",
                mesh.index(),
                primitive.index()
            );
            if is_core_primitive(&primitive) {
                check_core_primitive(document, &mesh, &primitive, &path, data)?;
                continue;
            }
            if !is_splat_primitive(&primitive) {
                return Err(ImportDiagnostic::error(
                    ImportCode::UnsupportedFeature,
                    path,
                    "a GLB with splat primitives has only splat primitives",
                    "export triangle meshes in a GLB of their own",
                ));
            }
            if is_card_primitive(&primitive) {
                kinds.1 = true;
                let texture = check_card_primitive(document, &primitive, &path)?;
                if atlas.is_some_and(|atlas| atlas != texture) {
                    return Err(ImportDiagnostic::error(
                        ImportCode::UnsupportedFeature,
                        format!("{path}.extras.bbsplat.atlasTexture"),
                        "a GLB's card primitives share one atlas",
                        "pack every card primitive's tiles into one atlas texture",
                    ));
                }
                atlas = Some(texture);
            } else {
                kinds.0 = true;
                check_splat_primitive(&primitive, &path)?;
            }
            if kinds.0 && kinds.1 {
                return Err(ImportDiagnostic::error(
                    ImportCode::UnsupportedFeature,
                    path,
                    "a GLB with splat primitives has Gaussians or cards, not both",
                    "export the Gaussians and the cards in GLBs of their own",
                ));
            }
        }
    }
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
        // A core stands inside the splats and does not widen their bounds.
        for primitive in mesh
            .primitives()
            .filter(|primitive| !is_core_primitive(primitive))
        {
            let attribute = |semantic: gltf::Semantic| {
                primitive
                    .get(&semantic)
                    .expect("checked by check_splat_primitive")
            };
            let centres = floats::<3>(attribute(gltf::Semantic::Positions), data)?;
            let scale = attribute(gltf::Semantic::Extras("SCALE".to_owned()));
            // A Gaussian reaches three standard deviations along its longest
            // axis; a card its half-diagonal.
            let reaches: Vec<f64> = if is_card_primitive(&primitive) {
                floats::<2>(scale, data)?
                    .iter()
                    .map(|half| half[0].hypot(half[1]))
                    .collect()
            } else {
                floats::<3>(scale, data)?
                    .iter()
                    .map(|scale| BOUNDS_SIGMAS * scale.iter().fold(0.0f64, |a, b| a.max(b.abs())))
                    .collect()
            };
            for (centre, reach) in centres.iter().zip(&reaches) {
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

/// Check a core primitive: its attributes, one value per vertex, finite
/// positions and normals, its joints within the skin of every node drawing
/// its mesh, and its indices within its vertices (the renderer skins and
/// draws it as they are).
fn check_core_primitive<'a, 's>(
    document: &gltf::Document,
    mesh: &gltf::Mesh<'_>,
    primitive: &'a gltf::Primitive<'a>,
    path: &str,
    data: impl Clone + Fn(gltf::Buffer<'a>) -> Option<&'s [u8]>,
) -> Result<(), ImportDiagnostic> {
    let refused = |locus: String, message: String| {
        ImportDiagnostic::error(
            ImportCode::MalformedSource,
            locus,
            message,
            "write POSITION, NORMAL and COLOR_0 (and _ALBEDO, JOINTS_0 and WEIGHTS_0) for every core vertex",
        )
    };
    let Some(positions) = primitive.get(&gltf::Semantic::Positions) else {
        return Err(refused(
            format!("{path}.attributes.POSITION"),
            "a splat core needs POSITION".to_owned(),
        ));
    };
    let vertices = positions.count();
    for (name, semantic, required) in [
        ("NORMAL", gltf::Semantic::Normals, true),
        ("COLOR_0", gltf::Semantic::Colors(0), true),
        (
            "_ALBEDO",
            gltf::Semantic::Extras("ALBEDO".to_owned()),
            false,
        ),
        ("JOINTS_0", gltf::Semantic::Joints(0), false),
        ("WEIGHTS_0", gltf::Semantic::Weights(0), false),
    ] {
        match primitive.get(&semantic) {
            None if required => {
                return Err(refused(
                    format!("{path}.attributes.{name}"),
                    format!("a splat core needs {name}"),
                ))
            }
            Some(accessor) if accessor.count() != vertices => {
                return Err(ImportDiagnostic::error(
                    ImportCode::AttributeLengthMismatch,
                    format!("{path}.attributes.{name}"),
                    format!(
                        "{name} has {} values for {vertices} core vertices",
                        accessor.count()
                    ),
                    "write one value of each attribute per core vertex",
                ))
            }
            _ => {}
        }
    }
    let reader = primitive.reader(data.clone());
    let finite = |values: &mut dyn Iterator<Item = [f32; 3]>| values.flatten().all(f32::is_finite);
    if !reader
        .read_positions()
        .is_some_and(|mut positions| finite(&mut positions))
        || !reader
            .read_normals()
            .is_some_and(|mut normals| finite(&mut normals))
    {
        return Err(ImportDiagnostic::error(
            ImportCode::NonFiniteValue,
            path.to_owned(),
            "a splat core's positions and normals are finite",
            "write finite core positions and normals",
        ));
    }
    if let Some(joints) = reader.read_joints(0) {
        let highest = joints.into_u16().flatten().max().unwrap_or(0);
        let skins = document
            .nodes()
            .filter(|node| {
                node.mesh()
                    .is_some_and(|drawn| drawn.index() == mesh.index())
            })
            .filter_map(|node| node.skin());
        for skin in skins {
            if usize::from(highest) >= skin.joints().count() {
                return Err(refused(
                    format!("{path}.attributes.JOINTS_0"),
                    format!(
                        "a splat core names joint {highest} of a skin of {} joints",
                        skin.joints().count()
                    ),
                ));
            }
        }
    }
    if let Some(indices) = reader.read_indices() {
        if indices.into_u32().any(|index| index as usize >= vertices) {
            return Err(refused(
                format!("{path}.indices"),
                format!("a splat core's indices reach past its {vertices} vertices"),
            ));
        }
    }
    Ok(())
}

/// Check a card primitive's attributes and atlas; returns the atlas's
/// texture.
fn check_card_primitive(
    document: &gltf::Document,
    primitive: &gltf::Primitive<'_>,
    path: &str,
) -> Result<usize, ImportDiagnostic> {
    let remedy = "write POSITION, _ROTATION, _SCALE and _TILE_ORIGIN for every card";
    let Some(positions) = primitive.get(&gltf::Semantic::Positions) else {
        return Err(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            format!("{path}.attributes.POSITION"),
            "a card primitive needs POSITION, its cards' centres",
            remedy,
        ));
    };
    if positions.count() == 0 {
        return Err(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            format!("{path}.attributes.POSITION"),
            "a card primitive has no cards",
            "remove the empty primitive",
        ));
    }
    for name in CARD_ATTRIBUTES.iter().chain(&["GROUP"]) {
        let Some(accessor) = primitive.get(&gltf::Semantic::Extras((*name).to_owned())) else {
            if *name == "GROUP" {
                continue;
            }
            return Err(ImportDiagnostic::error(
                ImportCode::MalformedSource,
                format!("{path}.attributes._{name}"),
                format!("a card primitive needs _{name}"),
                remedy,
            ));
        };
        if accessor.count() != positions.count() {
            return Err(ImportDiagnostic::error(
                ImportCode::AttributeLengthMismatch,
                format!("{path}.attributes._{name}"),
                format!(
                    "_{name} has {} values for {} cards",
                    accessor.count(),
                    positions.count()
                ),
                "write one value of each attribute per card",
            ));
        }
    }
    check_groups(primitive, path)?;
    // A tile is two whole numbers the renderer reads as they are.
    let tiles = primitive
        .get(&gltf::Semantic::Extras("TILE_ORIGIN".to_owned()))
        .expect("checked above");
    if tiles.dimensions() != gltf::accessor::Dimensions::Vec2
        || tiles.data_type() != gltf::accessor::DataType::U16
        || tiles.normalized()
        || tiles.view().is_none()
    {
        return Err(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            format!("{path}.attributes._TILE_ORIGIN"),
            "_TILE_ORIGIN is two unsigned shorts per card (VEC2, UNSIGNED_SHORT, not normalized)",
            "write each card's tile, in tiles from the atlas's top left",
        ));
    }
    let bbsplat = extras(primitive).and_then(|extras| extras.get("bbsplat").cloned());
    let whole = |name: &str| {
        bbsplat
            .as_ref()
            .and_then(|bbsplat| bbsplat.get(name))
            .and_then(serde_json::Value::as_u64)
    };
    if whole("tilePx").is_none_or(|pixels| pixels == 0) {
        return Err(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            format!("{path}.extras.bbsplat.tilePx"),
            "a card primitive needs its tiles' side in atlas pixels",
            "write extras.bbsplat.tilePx, a whole number above 0",
        ));
    }
    // The GLB's images are embedded (`animated_glb.rs` refuses others).
    let texture = whole("atlasTexture").and_then(|index| document.textures().nth(index as usize));
    match texture.filter(|texture| texture.source().is_some()) {
        Some(texture) => Ok(texture.index()),
        None => Err(ImportDiagnostic::error(
            ImportCode::MalformedSource,
            format!("{path}.extras.bbsplat.atlasTexture"),
            "a card primitive names its atlas, a texture with an image",
            "write extras.bbsplat.atlasTexture, the index of the atlas's texture",
        )),
    }
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
    check_groups(primitive, path)
}

/// A group is a byte the renderer reads as it is.
fn check_groups(primitive: &gltf::Primitive<'_>, path: &str) -> Result<(), ImportDiagnostic> {
    if let Some(groups) = primitive.get(&gltf::Semantic::Extras("GROUP".to_owned())) {
        if groups.dimensions() != gltf::accessor::Dimensions::Scalar
            || groups.data_type() != gltf::accessor::DataType::U8
            || groups.view().is_none()
        {
            return Err(ImportDiagnostic::error(
                ImportCode::MalformedSource,
                format!("{path}.attributes._GROUP"),
                "_GROUP is one unsigned byte per splat (SCALAR, UNSIGNED_BYTE)",
                "write each splat's group, 0 for none, as an unsigned byte",
            ));
        }
    }
    Ok(())
}

/// Attributes a splat primitive may carry: for lighting, `_NORMAL`, each
/// Gaussian's surface normal, and `_ALBEDO`, its colour without baked
/// shading; and `_GROUP`, the group a product may tint it by.
const OPTIONAL_SPLAT_ATTRIBUTES: [&str; 3] = ["NORMAL", "ALBEDO", "GROUP"];

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
        bin.extend(core_bytes());
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

    /// `_GROUP` is optional, one unsigned byte per Gaussian: a float or
    /// vector group, or a short one, is refused.
    #[test]
    fn a_splat_group_stream_is_admitted_and_malformed_ones_refused() {
        // The colours' two bytes 0 and 1 read as groups.
        let group = |accessor: serde_json::Value| {
            import(&glb(
                |attributes| attributes["_GROUP"] = serde_json::json!(4),
                move |document| document["accessors"].as_array_mut().unwrap().push(accessor),
            ))
        };
        let with = group(serde_json::json!(
            {"bufferView": 3, "componentType": 5121, "count": 2, "type": "SCALAR"}
        ));
        assert!(with.assets.is_some(), "{:?}", with.diagnostics);
        for (accessor, code) in [
            (
                serde_json::json!({"bufferView": 2, "componentType": 5126, "count": 2, "type": "SCALAR"}),
                ImportCode::MalformedSource,
            ),
            (
                serde_json::json!({"bufferView": 3, "componentType": 5121, "count": 2, "type": "VEC2"}),
                ImportCode::MalformedSource,
            ),
            (
                serde_json::json!({"bufferView": 3, "componentType": 5121, "count": 1, "type": "SCALAR"}),
                ImportCode::AttributeLengthMismatch,
            ),
        ] {
            let refused = group(accessor.clone());
            assert!(refused.assets.is_none(), "{accessor}");
            assert!(
                refused
                    .diagnostics
                    .iter()
                    .any(|d| d.code == code && d.message.contains("_GROUP")),
                "{accessor}: {:?}",
                refused.diagnostics
            );
        }
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

    /// A core's bytes, appended to a test GLB's BIN chunk: three vertices'
    /// positions, normals and linear colours (floats), joints and weights
    /// (bytes, joint 0 whole) and three u32 indices.
    fn core_bytes() -> Vec<u8> {
        let floats = |values: &[f32]| {
            values
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<u8>>()
        };
        let mut bytes = floats(&[0.0, 1.0, 0.0, 0.1, 1.0, 0.0, 0.0, 1.1, 0.0]);
        bytes.extend(floats(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]));
        bytes.extend(floats(&[0.5; 12]));
        bytes.extend([0u8; 12]);
        bytes.extend([255u8, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0]);
        bytes.extend([0u32, 1, 2].iter().flat_map(|v| v.to_le_bytes()));
        bytes
    }

    /// Add a core to a test GLB's mesh 0, from [`core_bytes`] at `offset`
    /// in its BIN chunk, skinned when `skinned`, its attributes edited by
    /// `edit`.
    fn add_core(
        document: &mut serde_json::Value,
        offset: usize,
        skinned: bool,
        edit: impl FnOnce(&mut serde_json::Value),
    ) {
        let views = document["bufferViews"].as_array_mut().unwrap();
        let first_view = views.len();
        for (start, length) in [(0, 36), (36, 36), (72, 48), (120, 12), (132, 12), (144, 12)] {
            views.push(serde_json::json!(
                {"buffer": 0, "byteOffset": offset + start, "byteLength": length}
            ));
        }
        let accessors = document["accessors"].as_array_mut().unwrap();
        let first = accessors.len();
        for (view, component, kind, normalized) in [
            (0, 5126, "VEC3", false),
            (1, 5126, "VEC3", false),
            (2, 5126, "VEC4", false),
            (3, 5121, "VEC4", false),
            (4, 5121, "VEC4", true),
            (5, 5125, "SCALAR", false),
        ] {
            accessors.push(
                serde_json::json!({"bufferView": first_view + view, "componentType": component,
                "count": 3, "type": kind, "normalized": normalized}),
            );
        }
        accessors[first]["min"] = serde_json::json!([0.0, 1.0, 0.0]);
        accessors[first]["max"] = serde_json::json!([0.1, 1.1, 0.0]);
        let mut attributes = serde_json::json!(
            {"POSITION": first, "NORMAL": first + 1, "COLOR_0": first + 2}
        );
        if skinned {
            attributes["JOINTS_0"] = serde_json::json!(first + 3);
            attributes["WEIGHTS_0"] = serde_json::json!(first + 4);
        }
        edit(&mut attributes);
        document["meshes"][0]["primitives"]
            .as_array_mut()
            .unwrap()
            .push(
                serde_json::json!({"mode": 4, "attributes": attributes, "indices": first + 5,
                "extras": {"splatCore": true}}),
            );
    }

    /// A core beside Gaussians, and beside skinned cards, is admitted, and
    /// does not widen the splats' bounds.
    #[test]
    fn a_core_beside_gaussians_or_cards_is_admitted_inside_their_bounds() {
        let plain = import(&glb(|_| {}, |_| {})).assets.unwrap().animated_mesh;
        let outcome = import(&glb(
            |_| {},
            |document| add_core(document, 88, false, |_| {}),
        ));
        let mesh = outcome
            .assets
            .unwrap_or_else(|| panic!("admitted: {:?}", outcome.diagnostics))
            .animated_mesh;
        assert_eq!(mesh.bounds.min, plain.bounds.min);
        assert_eq!(mesh.bounds.max, plain.bounds.max);
        let outcome = import(&cards_glb(
            true,
            |_| {},
            |document| add_core(document, 160, true, |_| {}),
        ));
        assert!(outcome.assets.is_some(), "{:?}", outcome.diagnostics);
    }

    /// A core without its normals or colours, with attributes of another
    /// length, a joint outside its skin or an index past its vertices, a
    /// second core, or a core in a mesh without splats is refused.
    #[test]
    fn malformed_cores_are_refused() {
        let refused = |bytes: Vec<u8>, code: ImportCode, word: &str| {
            let outcome = import(&bytes);
            assert!(outcome.assets.is_none(), "{word}");
            assert!(
                outcome
                    .diagnostics
                    .iter()
                    .any(|d| d.code == code && d.message.contains(word)),
                "{word}: {:?}",
                outcome.diagnostics
            );
        };
        let cards = |edit: fn(&mut serde_json::Value), extra: fn(&mut serde_json::Value)| {
            cards_glb(
                true,
                |_| {},
                move |document| {
                    add_core(document, 160, true, edit);
                    extra(document);
                },
            )
        };
        refused(
            cards(
                |attributes| {
                    attributes.as_object_mut().unwrap().remove("NORMAL");
                },
                |_| {},
            ),
            ImportCode::MalformedSource,
            "NORMAL",
        );
        refused(
            cards(
                |attributes| {
                    attributes.as_object_mut().unwrap().remove("COLOR_0");
                },
                |_| {},
            ),
            ImportCode::MalformedSource,
            "COLOR_0",
        );
        // The core's colours read as two values.
        refused(
            cards(
                |_| {},
                |document| document["accessors"][9]["count"] = serde_json::json!(2),
            ),
            ImportCode::AttributeLengthMismatch,
            "COLOR_0",
        );
        // Joint 255 of a skin of one: the joints read the weights' bytes.
        refused(
            cards(
                |_| {},
                |document| {
                    document["accessors"][10]["componentType"] = serde_json::json!(5121);
                    document["accessors"][10]["bufferView"] = serde_json::json!(12);
                },
            ),
            ImportCode::MalformedSource,
            "joint 255",
        );
        // Index 3 of three vertices: the indices read the colours' bytes.
        refused(
            cards(
                |_| {},
                |document| document["accessors"][12]["bufferView"] = serde_json::json!(10),
            ),
            ImportCode::MalformedSource,
            "indices",
        );
        refused(
            cards(
                |_| {},
                |document| {
                    let core = document["meshes"][0]["primitives"][1].clone();
                    document["meshes"][0]["primitives"]
                        .as_array_mut()
                        .unwrap()
                        .push(core);
                },
            ),
            ImportCode::UnsupportedFeature,
            "at most one core",
        );
        refused(
            cards(
                |_| {},
                |document| {
                    // The core in a mesh of its own.
                    let core = document["meshes"][0]["primitives"]
                        .as_array_mut()
                        .unwrap()
                        .remove(1);
                    document["meshes"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!({"primitives": [core]}));
                },
            ),
            ImportCode::UnsupportedFeature,
            "beside its splats",
        );
    }

    /// A GLB of two cards on one node, skinned to one joint when `skinned`,
    /// its primitive's attributes edited by `edit` and the document by
    /// `extra`. Its atlas's bytes are not an image: admission does not
    /// decode it.
    fn cards_glb(
        skinned: bool,
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
        bin.extend(floats(&[0.03, 0.04, 0.03, 0.04]));
        bin.extend([0u16, 0, 1, 0].iter().flat_map(|v| v.to_le_bytes()));
        bin.extend([0u8; 8]);
        bin.extend([255u8, 0, 0, 0, 255, 0, 0, 0]);
        bin.extend(floats(&[
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]));
        bin.extend(core_bytes());
        let view = |offset: usize, length: usize| serde_json::json!({"buffer": 0, "byteOffset": offset, "byteLength": length});
        let mut attributes = serde_json::json!(
            {"POSITION": 0, "_ROTATION": 1, "_SCALE": 2, "_TILE_ORIGIN": 3}
        );
        if skinned {
            attributes["JOINTS_0"] = serde_json::json!(4);
            attributes["WEIGHTS_0"] = serde_json::json!(5);
        }
        edit(&mut attributes);
        let mut document = serde_json::json!({
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0, 1]}],
            "nodes": [
                {"name": "body", "mesh": 0, "translation": [0.0, 0.0, 2.0]},
                {"name": "hips"},
            ],
            "meshes": [{"primitives": [{
                "mode": 0,
                "attributes": attributes,
                "extras": {"splatKind": "billboard", "bbsplat": {"tilePx": 16, "atlasW": 32, "atlasH": 16, "atlasTexture": 0}},
            }]}],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 2, "type": "VEC3",
                 "min": [0.0, 1.0, 0.0], "max": [0.2, 1.1, 0.0]},
                {"bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC4"},
                {"bufferView": 2, "componentType": 5126, "count": 2, "type": "VEC2"},
                {"bufferView": 3, "componentType": 5123, "count": 2, "type": "VEC2"},
                {"bufferView": 4, "componentType": 5121, "count": 2, "type": "VEC4"},
                {"bufferView": 5, "componentType": 5121, "normalized": true, "count": 2, "type": "VEC4"},
                {"bufferView": 6, "componentType": 5126, "count": 1, "type": "MAT4"},
            ],
            "bufferViews": [view(0, 24), view(24, 32), view(56, 16), view(72, 8), view(80, 8),
                            view(88, 8), view(96, 64), view(0, 8)],
            "buffers": [{"byteLength": bin.len()}],
            "images": [{"bufferView": 7, "mimeType": "image/png"}],
            "textures": [{"source": 0}],
        });
        if skinned {
            document["nodes"][0]["skin"] = serde_json::json!(0);
            document["skins"] = serde_json::json!([{"joints": [1], "inverseBindMatrices": 6}]);
        }
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

    /// A card GLB is admitted with its cards' half-diagonals as their
    /// reach, unskinned in model space and skinned in bind space with its
    /// rig named.
    #[test]
    fn a_card_glb_is_admitted_with_its_cards_bounds() {
        let outcome = import(&cards_glb(false, |_| {}, |_| {}));
        let mesh = outcome
            .assets
            .unwrap_or_else(|| panic!("admitted: {:?}", outcome.diagnostics))
            .animated_mesh;
        // Centres reaching hypot(0.03, 0.04) = 0.05, moved by the node.
        assert_eq!(
            mesh.bounds.min.map(|v| (v * 1000.0).round()),
            [-50.0, 950.0, 1950.0]
        );
        assert_eq!(
            mesh.bounds.max.map(|v| (v * 1000.0).round()),
            [250.0, 1150.0, 2050.0]
        );
        let skinned = import(&cards_glb(true, |_| {}, |_| {}));
        let mesh = skinned
            .assets
            .unwrap_or_else(|| panic!("admitted: {:?}", skinned.diagnostics))
            .animated_mesh;
        assert_eq!(
            mesh.bounds.min.map(|v| (v * 1000.0).round()),
            [-50.0, 950.0, -50.0]
        );
        let rig = mesh.rig.expect("a skinned card GLB names its rig");
        assert_eq!(rig.joints[0].id, "hips");
    }

    /// A card primitive without its tiles, with tiles of the wrong type or
    /// count, without a tile size or naming no atlas texture is refused,
    /// and so are cards beside Gaussians.
    #[test]
    fn malformed_card_glbs_are_refused() {
        let refused = |bytes: Vec<u8>, code: ImportCode, word: &str| {
            let outcome = import(&bytes);
            assert!(outcome.assets.is_none(), "{word}");
            assert!(
                outcome
                    .diagnostics
                    .iter()
                    .any(|d| d.code == code && d.message.contains(word)),
                "{word}: {:?}",
                outcome.diagnostics
            );
        };
        refused(
            cards_glb(
                false,
                |attributes| {
                    attributes.as_object_mut().unwrap().remove("_TILE_ORIGIN");
                },
                |_| {},
            ),
            ImportCode::MalformedSource,
            "_TILE_ORIGIN",
        );
        refused(
            cards_glb(
                false,
                |_| {},
                |document| {
                    document["accessors"][3]["componentType"] = serde_json::json!(5126);
                },
            ),
            ImportCode::MalformedSource,
            "_TILE_ORIGIN",
        );
        refused(
            cards_glb(
                false,
                |_| {},
                |document| document["accessors"][3]["count"] = serde_json::json!(1),
            ),
            ImportCode::AttributeLengthMismatch,
            "_TILE_ORIGIN",
        );
        refused(
            cards_glb(
                false,
                |_| {},
                |document| {
                    document["meshes"][0]["primitives"][0]["extras"]["bbsplat"]
                        .as_object_mut()
                        .unwrap()
                        .remove("tilePx");
                },
            ),
            ImportCode::MalformedSource,
            "tiles' side",
        );
        refused(
            cards_glb(
                false,
                |_| {},
                |document| {
                    document["meshes"][0]["primitives"][0]["extras"]["bbsplat"]["atlasTexture"] =
                        serde_json::json!(3);
                },
            ),
            ImportCode::MalformedSource,
            "atlas",
        );
        refused(
            cards_glb(
                false,
                |_| {},
                |document| {
                    // A Gaussian primitive beside the cards, reading the
                    // cards' streams.
                    document["accessors"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!(
                            {"bufferView": 1, "componentType": 5126, "count": 2, "type": "VEC3"}
                        ));
                    document["meshes"][0]["primitives"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!({"mode": 0, "attributes":
                            {"POSITION": 0, "_ROTATION": 1, "_SCALE": 7, "_COLOR": 5}}));
                },
            ),
            ImportCode::UnsupportedFeature,
            "not both",
        );
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
