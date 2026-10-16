//! Vegetation tiles (#10016): grass, plants and placed things over ground a
//! product samples, with no voxel or collision residency behind them.
//!
//! A field divides the world's ground (x and z in world metres) into square
//! tiles. It asks for the tiles within its layers' reach of the focus,
//! nearest first, each with a revision; the product samples a tile as it
//! samples a heightfield tile (`svc_mesh::heightfield::TileSamples`: heights
//! with a ring about them, and in `materials` a ground class per vertex),
//! with an optional cover weight per vertex and explicit placement records.
//! Samples worked out for an older revision (the tile was invalidated or left
//! and came back) are ignored; an invalidated tile keeps drawing what it had
//! until its new samples land.
//!
//! Each layer grows one static mesh. Its copies stand on the triangles a
//! heightfield of the same lattice draws (`TileSurface::surface`), at the
//! spots of the absolute jittered grid voxel scatter uses
//! (`svc_mesh::scatter`), drawn from the same keys: the same ground grows the
//! same copies whichever tile or revision placed them, and a voxel scatter
//! of the same seed and density fading out where a layer's near band fades
//! in hands its copies over one for one. A layer's copies on a tile are one
//! scatter patch, under a group node for the tile, under the field's root,
//! which stands at the world's origin in its scene; tiles nearest the focus
//! take a layer's instance budget first.
//!
//! Placing is the costly step: it runs on worker threads, at most
//! `placements_per_settle` tile layers a settle, and what waits is counted.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use render_model::{
    Geometry, MeshMaterialSlot, RenderDiff, RenderHandle, RenderNode, ScatterFade, ScatterInstance,
    ScatterLod, ScatterPatchDescriptor, ScatterThinning, ShadowCasting, Transform,
    MAX_SCATTER_PATCH_INSTANCES,
};
use svc_mesh::heightfield::{HeightfieldLayout, HeightfieldRingSpec, TileSamples, TileSurface};
use svc_mesh::heightfield_tiles::in_parallel;
use svc_mesh::scatter::{hash, unit};

use crate::voxel_scatter::{about_y, copy_at, lean, multiply};
use crate::{HandleAllocationError, RenderHandleNamespace, StableHandleRegistry};

/// A tile is kept, and a layer's patch on it, until it is this many times
/// the reach away, so a focus on the boundary does not rebuild it.
pub const VEGETATION_HYSTERESIS: f64 = 1.1;

/// Ground classes are 0 to 63: a layer's `grounds` holds a bit for each.
pub const VEGETATION_GROUND_CLASSES: u32 = 64;

/// The most copies per square metre a layer may ask for.
pub const MAX_VEGETATION_DENSITY: f32 = 64.0;

/// Mixed into a layer's seed for the draw that thins a spot by its cover.
const COVER_SALT: u64 = 0xc07e_5a17;
/// The keys a spot's copy is drawn from, as voxel scatter keys them.
const KEY_SALT: u64 = 0x5ca7_7e7a;

/// How a field is tiled: square tiles of `tile_size` metres, sampled every
/// `tile_size / tile_cells` metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VegetationLayout {
    pub tile_size: f64,
    pub tile_cells: u32,
}

/// One kind of thing that grows. Distances are metres from the focus.
#[derive(Debug, Clone, PartialEq)]
pub struct VegetationLayer {
    /// The static mesh each copy draws, defined by the caller.
    pub mesh: String,
    pub material_overrides: Vec<MeshMaterialSlot>,
    pub shadow_casting: ShadowCasting,
    /// Copies per square metre of ground; 0 grows only placement records.
    pub density: f32,
    /// The ground classes it grows on, a bit each.
    pub grounds: u64,
    /// Ground steeper than this many degrees grows nothing.
    pub slope_limit_degrees: f32,
    /// 0 stands copies upright; 1 stands them along the ground's normal.
    pub align: f32,
    pub scale: [f32; 2],
    pub tints: [[f32; 3]; 2],
    /// Keeps two layers of the same mesh from growing in the same spots.
    pub seed: u64,
    /// Copies stand on tiles within this distance, and shrink away over
    /// the last `fade` metres of it.
    pub reach: f32,
    pub fade: f32,
    /// Copies nearer than this are not drawn; they grow back over the next
    /// `near_fade` metres. 0 draws them however near.
    pub near: f32,
    pub near_fade: f32,
    /// At most this many copies: the nearest tiles' first.
    pub maximum_instances: u32,
    /// Farther levels of detail and the band they crossfade over
    /// (`ScatterLod`), and coverage-preserving thinning by distance.
    pub lods: Vec<ScatterLod>,
    pub lod_band: f32,
    pub thinning: ScatterThinning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VegetationLayerError {
    Mesh,
    Density,
    Distances,
    Scale,
    Tint,
    Slope,
    Align,
    Detail,
}

impl VegetationLayer {
    pub fn validate(&self) -> Result<(), VegetationLayerError> {
        render_model::validate_asset_id(&self.mesh, render_model::RenderAssetKind::StaticMesh)
            .map_err(|_| VegetationLayerError::Mesh)?;
        if !(0.0..=MAX_VEGETATION_DENSITY).contains(&self.density) {
            return Err(VegetationLayerError::Density);
        }
        let distances = [self.reach, self.fade, self.near, self.near_fade];
        if !distances
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
            || self.reach <= 0.0
            || self.fade >= self.reach
            || self.near + self.near_fade > self.reach - self.fade
        {
            return Err(VegetationLayerError::Distances);
        }
        if !(self
            .scale
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
            && self.scale[0] <= self.scale[1])
        {
            return Err(VegetationLayerError::Scale);
        }
        if !self
            .tints
            .iter()
            .flatten()
            .all(|value| value.is_finite() && *value >= 0.0)
        {
            return Err(VegetationLayerError::Tint);
        }
        if !(0.0..=90.0).contains(&self.slope_limit_degrees) {
            return Err(VegetationLayerError::Slope);
        }
        if !(0.0..=1.0).contains(&self.align) {
            return Err(VegetationLayerError::Align);
        }
        render_model::validate_detail(&self.lods, self.lod_band, &self.thinning)
            .map_err(|_| VegetationLayerError::Detail)?;
        Ok(())
    }

    fn fade_band(&self) -> ScatterFade {
        let (start, end) = if self.fade > 0.0 {
            (self.reach - self.fade, self.reach)
        } else {
            (0.0, 0.0)
        };
        let (near_start, near_end) = if self.near > 0.0 {
            (self.near, self.near + self.near_fade)
        } else {
            (0.0, 0.0)
        };
        ScatterFade {
            start,
            end,
            near_start,
            near_end,
        }
    }
}

/// One thing the product places itself on a tile, in the layer keyed
/// `layer`: x and z from the tile's origin, y the world height. It turns
/// `yaw` radians about the vertical and leans onto the ground as its layer
/// does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VegetationRecord {
    pub layer: u32,
    pub position: [f32; 3],
    pub yaw: f32,
    pub scale: f32,
}

/// What a product gives for one tile: its ground, a cover weight (0 to 1)
/// at each vertex that thins every layer's spots there (none: 1), and its
/// placement records.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VegetationTileContent {
    pub samples: TileSamples,
    pub cover: Option<Vec<f32>>,
    pub records: Vec<VegetationRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VegetationError {
    /// Tile size not finite and above 0, or cells outside 1 to 256.
    Layout,
    /// The field does not live.
    Field,
    /// Heights not (cells + 3)², or cover or classes not (cells + 1)².
    SampleCount {
        expected: usize,
        actual: usize,
    },
    /// A tile carried colours or layer weights, which vegetation reads not.
    Streams,
    /// A height infinite, a cover outside 0 to 1, a ground class over 63
    /// or a record not finite or not of positive scale.
    Value,
    Handle(HandleAllocationError),
}

impl From<HandleAllocationError> for VegetationError {
    fn from(error: HandleAllocationError) -> Self {
        Self::Handle(error)
    }
}

/// A tile to sample: its column and row, the revision to commit it with,
/// and where its first vertex stands (world x, z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VegetationRequest {
    pub x: i64,
    pub z: i64,
    pub revision: u32,
    pub origin: [f64; 2],
}

/// A field's state and the work of its last settle.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct VegetationReadout {
    /// Tiles holding samples, and tiles in reach waiting for current ones.
    pub tiles: u32,
    pub pending_tiles: u32,
    /// Patches drawn and the copies they hold.
    pub patches: u32,
    pub instances: u64,
    /// Tile layers in reach a layer's instance budget left bare.
    pub over_budget: u32,
    /// Tile layers in reach with samples, waiting for a placement.
    pub deferred: u32,
    /// The last settle's placements (tile layers) and the copies they made,
    /// and its time placing (on workers, end to end), choosing what is
    /// drawn, and turning that into renderer work.
    pub placed: u32,
    pub placed_instances: u64,
    pub placement_microseconds: u64,
    pub selection_microseconds: u64,
    pub publication_microseconds: u64,
    /// Since the field was made: tiles invalidated, and patches placed again
    /// on a tile's new samples.
    pub invalidated_tiles: u64,
    pub replaced_patches: u64,
    /// The samples and records held, in bytes.
    pub sample_bytes: u64,
}

/// A layer's key and a tile.
type Slot = (u32, (i64, i64));

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NodeKey {
    Root(u64),
    Tile(u64, i64, i64),
    Patch(u64, i64, i64, u32),
}

/// A tile's samples, as placing reads them.
#[derive(Debug)]
struct Content {
    surface: TileSurface,
    cover: Option<Vec<f32>>,
    records: Vec<VegetationRecord>,
    /// Lowest and highest world height sampled.
    heights: [f32; 2],
    generation: u64,
    bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct Patch {
    handle: RenderHandle,
    instances: u32,
    /// The content and layer generations it was placed for.
    generations: [u64; 2],
}

#[derive(Debug, Clone)]
struct Tile {
    revision: u32,
    /// The samples, current at `revision` or kept from before it until the
    /// new ones land.
    content: Option<Arc<Content>>,
    current: bool,
    patches: BTreeMap<u32, Patch>,
    /// Layers whose copies here the budget left out: their count, for the
    /// generations it was made for.
    bare: BTreeMap<u32, ([u64; 2], u32)>,
    node: Option<RenderHandle>,
}

#[derive(Debug, Clone)]
struct Field {
    layout: VegetationLayout,
    heightfield: HeightfieldLayout,
    layers: BTreeMap<u32, (VegetationLayer, u64)>,
    placements_per_settle: u32,
    tiles: BTreeMap<(i64, i64), Tile>,
    focus: Option<[f64; 3]>,
    placement: Option<[f32; 3]>,
    root: Option<RenderHandle>,
    next_revision: u32,
    next_generation: u64,
    readout: VegetationReadout,
}

impl Field {
    fn generation(&mut self) -> u64 {
        self.next_generation += 1;
        self.next_generation
    }

    fn revision(&mut self) -> u32 {
        self.next_revision = self.next_revision.wrapping_add(1).max(1);
        self.next_revision
    }

    fn reach(&self) -> f64 {
        self.layers
            .values()
            .map(|(layer, _)| f64::from(layer.reach))
            .fold(0.0, f64::max)
    }

    fn origin(&self, (x, z): (i64, i64)) -> [f64; 2] {
        [
            x as f64 * self.layout.tile_size,
            z as f64 * self.layout.tile_size,
        ]
    }

    /// The horizontal distance from the focus to a tile.
    fn flat_distance(&self, focus: [f64; 3], tile: (i64, i64)) -> f64 {
        let [x0, z0] = self.origin(tile);
        let size = self.layout.tile_size;
        let gap = |low: f64, at: f64| (low - at).max(at - (low + size)).max(0.0);
        gap(x0, focus[0]).hypot(gap(z0, focus[2]))
    }

    /// The distance from the focus to the nearest and farthest points of a
    /// tile's ground, or its horizontal ones before it has samples.
    fn distances(&self, focus: [f64; 3], tile: (i64, i64), content: Option<&Content>) -> [f64; 2] {
        let [x0, z0] = self.origin(tile);
        let size = self.layout.tile_size;
        let [low, high] = content.map_or([focus[1]; 2], |content| content.heights.map(f64::from));
        let span = [(x0, x0 + size), (low, high), (z0, z0 + size)];
        let mut near = 0.0;
        let mut far = 0.0;
        for (axis, (min, max)) in span.into_iter().enumerate() {
            let at = focus[axis];
            let gap = (min - at).max(at - max).max(0.0);
            let reach = (at - min).abs().max((max - at).abs());
            near += gap * gap;
            far += reach * reach;
        }
        [near.sqrt(), far.sqrt()]
    }
}

/// The vegetation fields of a runtime and the renderer nodes they draw.
#[derive(Debug, Clone)]
pub struct VegetationFields {
    registry: StableHandleRegistry<NodeKey>,
    fields: BTreeMap<u64, Field>,
}

impl Default for VegetationFields {
    fn default() -> Self {
        Self {
            registry: StableHandleRegistry::new(RenderHandleNamespace::VEGETATION),
            fields: BTreeMap::new(),
        }
    }
}

impl VegetationFields {
    pub fn create(
        &mut self,
        field: u64,
        layout: VegetationLayout,
        placements_per_settle: u32,
    ) -> Result<(), VegetationError> {
        if !(layout.tile_size.is_finite() && layout.tile_size > 0.0) {
            return Err(VegetationError::Layout);
        }
        let heightfield = HeightfieldLayout::new(
            layout.tile_cells,
            &[HeightfieldRingSpec {
                cell_size: layout.tile_size / f64::from(layout.tile_cells.max(1)),
                reach: layout.tile_size,
            }],
        )
        .map_err(|_| VegetationError::Layout)?;
        self.fields.insert(
            field,
            Field {
                layout,
                heightfield,
                layers: BTreeMap::new(),
                placements_per_settle: placements_per_settle.max(1),
                tiles: BTreeMap::new(),
                focus: None,
                placement: None,
                root: None,
                next_revision: 0,
                next_generation: 0,
                readout: VegetationReadout::default(),
            },
        );
        Ok(())
    }

    /// Remove a field and everything it draws.
    pub fn destroy(&mut self, field: u64) -> Vec<RenderDiff> {
        let Some(removed) = self.fields.remove(&field) else {
            return Vec::new();
        };
        for (&(x, z), tile) in &removed.tiles {
            self.forget_tile(field, (x, z), tile);
        }
        self.registry.remove(&NodeKey::Root(field));
        removed
            .root
            .map(|handle| RenderDiff::Destroy { handle })
            .into_iter()
            .collect()
    }

    fn field(&mut self, field: u64) -> Result<&mut Field, VegetationError> {
        self.fields.get_mut(&field).ok_or(VegetationError::Field)
    }

    pub fn layout(&self, field: u64) -> Option<VegetationLayout> {
        self.fields.get(&field).map(|field| field.layout)
    }

    /// Set (or change) a layer; its copies are placed again as the settle's
    /// budget allows, drawing as they were until then.
    pub fn set_layer(
        &mut self,
        field: u64,
        key: u32,
        layer: VegetationLayer,
    ) -> Result<(), VegetationError> {
        let retained = self.field(field)?;
        if retained.layers.get(&key).map(|(current, _)| current) == Some(&layer) {
            return Ok(());
        }
        let generation = retained.generation();
        retained.layers.insert(key, (layer, generation));
        Ok(())
    }

    /// Remove a layer and its patches.
    pub fn remove_layer(
        &mut self,
        field: u64,
        key: u32,
    ) -> Result<Vec<RenderDiff>, VegetationError> {
        let retained = self.fields.get_mut(&field).ok_or(VegetationError::Field)?;
        if retained.layers.remove(&key).is_none() {
            return Ok(Vec::new());
        }
        let mut operations = Vec::new();
        for (&(x, z), tile) in &mut retained.tiles {
            tile.bare.remove(&key);
            if let Some(patch) = tile.patches.remove(&key) {
                self.registry.remove(&NodeKey::Patch(field, x, z, key));
                operations.push(RenderDiff::Destroy {
                    handle: patch.handle,
                });
            }
        }
        Ok(operations)
    }

    /// The tiles waiting for samples, nearest the focus first; at most
    /// `max` (0: all).
    pub fn requests(&self, field: u64, max: usize) -> Vec<VegetationRequest> {
        let Some(retained) = self.fields.get(&field) else {
            return Vec::new();
        };
        let Some(focus) = retained.focus else {
            return Vec::new();
        };
        let mut waiting: Vec<(f64, (i64, i64), u32)> = retained
            .tiles
            .iter()
            .filter(|(_, tile)| !tile.current)
            .map(|(&key, tile)| (retained.flat_distance(focus, key), key, tile.revision))
            .collect();
        waiting.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        if max > 0 {
            waiting.truncate(max);
        }
        waiting
            .into_iter()
            .map(|(_, (x, z), revision)| VegetationRequest {
                x,
                z,
                revision,
                origin: retained.origin((x, z)),
            })
            .collect()
    }

    /// Take a tile's samples; false when `revision` is not the tile's now.
    pub fn commit(
        &mut self,
        field: u64,
        (x, z): (i64, i64),
        revision: u32,
        content: VegetationTileContent,
    ) -> Result<bool, VegetationError> {
        let retained = self.field(field)?;
        validate(&retained.heightfield, &content)?;
        let generation = retained.generation();
        let Some(tile) = retained.tiles.get_mut(&(x, z)) else {
            return Ok(false);
        };
        if tile.revision != revision || tile.current {
            return Ok(false);
        }
        let VegetationTileContent {
            samples,
            cover,
            records,
        } = content;
        let finite = samples.heights.iter().filter(|height| height.is_finite());
        let heights = finite.fold([f32::INFINITY, f32::NEG_INFINITY], |[low, high], &h| {
            [low.min(h), high.max(h)]
        });
        let bytes = (samples.heights.len() * 4
            + samples
                .materials
                .as_ref()
                .map_or(0, |classes| classes.len() * 4)
            + cover.as_ref().map_or(0, |cover| cover.len() * 4)
            + records.len() * std::mem::size_of::<VegetationRecord>()) as u64;
        tile.content = Some(Arc::new(Content {
            surface: TileSurface::new(&retained.heightfield, 0, samples),
            cover,
            records,
            heights: if heights[0] <= heights[1] {
                heights
            } else {
                [0.0; 2]
            },
            generation,
            bytes,
        }));
        tile.current = true;
        Ok(true)
    }

    /// Every tile overlapping the rectangle (world x, z) is sampled again;
    /// each keeps drawing what it has until its new samples land.
    pub fn invalidate(
        &mut self,
        field: u64,
        min: [f64; 2],
        max: [f64; 2],
    ) -> Result<(), VegetationError> {
        let retained = self.field(field)?;
        let size = retained.layout.tile_size;
        let first = min.map(|value| (value / size).floor() as i64);
        let last = max.map(|value| (value / size).floor() as i64);
        let keys: Vec<(i64, i64)> = retained
            .tiles
            .range((first[0], i64::MIN)..=(last[0], i64::MAX))
            .map(|(key, _)| *key)
            .filter(|&(_, z)| (first[1]..=last[1]).contains(&z))
            .collect();
        for key in keys {
            let revision = retained.revision();
            let tile = retained.tiles.get_mut(&key).expect("listed above");
            tile.revision = revision;
            tile.current = false;
            retained.readout.invalidated_tiles += 1;
        }
        Ok(())
    }

    pub fn readout(&self, field: u64) -> Option<VegetationReadout> {
        self.fields.get(&field).map(|field| field.readout)
    }

    /// The handle of a tile's patch for a layer, for tests and inspection.
    pub fn patch_handle(&self, field: u64, tile: (i64, i64), layer: u32) -> Option<RenderHandle> {
        self.registry
            .handle_of(&NodeKey::Patch(field, tile.0, tile.1, layer))
    }

    fn forget_tile(&mut self, field: u64, (x, z): (i64, i64), tile: &Tile) {
        for layer in tile.patches.keys() {
            self.registry.remove(&NodeKey::Patch(field, x, z, *layer));
        }
        self.registry.remove(&NodeKey::Tile(field, x, z));
    }

    /// Bring a field to its focus: `placement` is where the world's origin
    /// stands in the scene, `focus` the viewer in world metres (None: hold
    /// still). Tiles leave and are asked for, layers' copies are placed as
    /// the budget allows and chosen nearest first within each layer's
    /// instance budget, and the patches change to match.
    pub fn settle(
        &mut self,
        field: u64,
        placement: [f64; 3],
        focus: Option<[f64; 3]>,
    ) -> Result<Vec<RenderDiff>, VegetationError> {
        let mut retained = self.fields.remove(&field).ok_or(VegetationError::Field)?;
        let result = self.settle_field(field, &mut retained, placement, focus);
        self.fields.insert(field, retained);
        result
    }

    fn settle_field(
        &mut self,
        field: u64,
        retained: &mut Field,
        placement: [f64; 3],
        focus: Option<[f64; 3]>,
    ) -> Result<Vec<RenderDiff>, VegetationError> {
        let started = Instant::now();
        let mut operations = Vec::new();
        let placement = placement.map(|value| value as f32);
        let transform = Transform {
            translation: placement,
            ..Transform::IDENTITY
        };
        match retained.root {
            None => {
                let handle = self.registry.allocate(NodeKey::Root(field))?;
                let mut node = RenderNode::new(Geometry::Group);
                node.transform = transform;
                operations.push(RenderDiff::Create {
                    handle,
                    parent: None,
                    node,
                });
                retained.root = Some(handle);
            }
            Some(handle) if retained.placement != Some(placement) => {
                operations.push(RenderDiff::Update {
                    handle,
                    transform: Some(transform),
                    material: None,
                    visible: None,
                    metadata: None,
                });
            }
            Some(_) => {}
        }
        retained.placement = Some(placement);
        if focus.is_some() {
            retained.focus = focus;
        }
        let readout = &mut retained.readout;
        readout.placed = 0;
        readout.placed_instances = 0;
        readout.placement_microseconds = 0;
        let Some(focus) = retained.focus else {
            readout.selection_microseconds = started.elapsed().as_micros() as u64;
            readout.publication_microseconds = 0;
            return Ok(operations);
        };

        // Residency: the tiles within the farthest reach, kept a little past it.
        let reach = retained.reach();
        let leaving: Vec<(i64, i64)> = retained
            .tiles
            .keys()
            .copied()
            .filter(|&key| retained.flat_distance(focus, key) > reach * VEGETATION_HYSTERESIS)
            .collect();
        for key in leaving {
            let tile = retained.tiles.remove(&key).expect("listed above");
            self.forget_tile(field, key, &tile);
            if let Some(handle) = tile.node {
                operations.push(RenderDiff::Destroy { handle });
            }
        }
        if reach > 0.0 {
            let size = retained.layout.tile_size;
            let low =
                [focus[0] - reach, focus[2] - reach].map(|value| (value / size).floor() as i64);
            let high =
                [focus[0] + reach, focus[2] + reach].map(|value| (value / size).floor() as i64);
            for x in low[0]..=high[0] {
                for z in low[1]..=high[1] {
                    if retained.tiles.contains_key(&(x, z))
                        || retained.flat_distance(focus, (x, z)) > reach
                    {
                        continue;
                    }
                    let revision = retained.revision();
                    retained.tiles.insert(
                        (x, z),
                        Tile {
                            revision,
                            content: None,
                            current: false,
                            patches: BTreeMap::new(),
                            bare: BTreeMap::new(),
                            node: None,
                        },
                    );
                }
            }
        }

        // Each layer's candidates, nearest first, with what is known of
        // their counts; those unknown are placed as the budget allows.
        struct Candidate {
            tile: (i64, i64),
            content: Arc<Content>,
            generations: [u64; 2],
            known: Option<u32>,
        }
        let mut candidates: BTreeMap<u32, Vec<Candidate>> = BTreeMap::new();
        for (&key, (layer, layer_generation)) in &retained.layers {
            let mut list: Vec<(f64, Candidate)> = Vec::new();
            for (&tile_key, tile) in &retained.tiles {
                let Some(content) = &tile.content else {
                    continue;
                };
                let [near, far] = retained.distances(focus, tile_key, Some(content));
                let held = tile.patches.contains_key(&key);
                let limit = f64::from(layer.reach) * if held { VEGETATION_HYSTERESIS } else { 1.0 };
                if near > limit || (layer.near > 0.0 && far < f64::from(layer.near)) {
                    continue;
                }
                let generations = [content.generation, *layer_generation];
                let known = match (tile.patches.get(&key), tile.bare.get(&key)) {
                    (Some(patch), _) if patch.generations == generations => Some(patch.instances),
                    (_, Some((made, count))) if *made == generations => Some(*count),
                    _ => None,
                };
                list.push((
                    near,
                    Candidate {
                        tile: tile_key,
                        content: content.clone(),
                        generations,
                        known,
                    },
                ));
            }
            list.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.tile.cmp(&b.1.tile)));
            candidates.insert(
                key,
                list.into_iter().map(|(_, candidate)| candidate).collect(),
            );
        }
        // Placing: the candidates whose counts are unknown, nearest first
        // across layers (the budget goes where it shows most), at most the
        // field's placements a settle.
        let placements = retained.placements_per_settle as usize;
        let mut jobs: Vec<(usize, u32)> = candidates
            .iter()
            .flat_map(|(key, list)| {
                list.iter()
                    .enumerate()
                    .filter(|(_, candidate)| candidate.known.is_none())
                    .map(move |(index, _)| (index, *key))
            })
            .collect();
        jobs.sort();
        let mut deferred = jobs.len().saturating_sub(placements) as u32;
        jobs.truncate(placements);
        let selection_before_placing = started.elapsed();
        let placing = Instant::now();
        let layout = retained.layout;
        let layers = &retained.layers;
        let placed: Vec<Vec<ScatterInstance>> = in_parallel(&jobs, |&(index, key)| {
            let candidate = &candidates[&key][index];
            place(
                &layout,
                candidate.tile,
                &candidate.content,
                key,
                &layers[&key].0,
            )
        });
        let mut placement_time = placing.elapsed();
        let mut fresh: BTreeMap<Slot, Vec<ScatterInstance>> = BTreeMap::new();
        for (&(index, key), instances) in jobs.iter().zip(placed) {
            let candidate = &mut candidates.get_mut(&key).expect("a job's layer")[index];
            candidate.known = Some(instances.len() as u32);
            retained.readout.placed += 1;
            retained.readout.placed_instances += instances.len() as u64;
            fresh.insert((key, candidate.tile), instances);
        }

        // Choosing: within each layer's budget, nearest first. A tile layer
        // waiting to be placed again counts and draws what it has meanwhile.
        let selecting = Instant::now();
        let mut chosen: BTreeMap<Slot, [u64; 2]> = BTreeMap::new();
        let mut over_budget = 0u32;
        for (&key, list) in &candidates {
            let mut budget = u64::from(retained.layers[&key].0.maximum_instances);
            for candidate in list {
                let tile = retained
                    .tiles
                    .get_mut(&candidate.tile)
                    .expect("a candidate's tile");
                let (count, generations) = match (candidate.known, tile.patches.get(&key)) {
                    (Some(count), _) => (count, candidate.generations),
                    (None, Some(patch)) => (patch.instances, patch.generations),
                    (None, None) => continue,
                };
                if count == 0 {
                    // Bare ground: nothing to draw until it changes.
                    tile.bare.insert(key, (generations, 0));
                    continue;
                }
                if u64::from(count) > budget {
                    tile.bare.insert(key, (generations, count));
                    over_budget += 1;
                    continue;
                }
                budget -= u64::from(count);
                tile.bare.remove(&key);
                chosen.insert((key, candidate.tile), generations);
            }
        }
        // Chosen tile layers last counted while the budget left them out
        // need their copies again: they take the placements left.
        let mut late: Vec<(Slot, Arc<Content>)> = Vec::new();
        for (&(key, tile_key), generations) in &chosen {
            let tile = &retained.tiles[&tile_key];
            let drawn = tile
                .patches
                .get(&key)
                .is_some_and(|patch| patch.generations == *generations);
            if !drawn && !fresh.contains_key(&(key, tile_key)) {
                let content = tile.content.clone().expect("chosen tiles have samples");
                late.push(((key, tile_key), content));
            }
        }
        let left = placements.saturating_sub(jobs.len());
        if late.len() > left {
            deferred += (late.len() - left) as u32;
            late.truncate(left);
        }
        let selection_time = selection_before_placing + selecting.elapsed();
        let placing = Instant::now();
        let placed_late = in_parallel(&late, |((key, tile_key), content)| {
            place(&layout, *tile_key, content, *key, &retained.layers[key].0)
        });
        placement_time += placing.elapsed();
        for ((slot, _), instances) in late.iter().zip(placed_late) {
            retained.readout.placed += 1;
            retained.readout.placed_instances += instances.len() as u64;
            fresh.insert(*slot, instances);
        }

        // Publication: drop the patches no longer chosen, replace those
        // whose new copies are ready, and make the new ones.
        let publishing = Instant::now();
        let root = retained.root;
        for (&tile_key, tile) in &mut retained.tiles {
            let (x, z) = tile_key;
            let mut leaving: Vec<(u32, bool)> = Vec::new();
            for (&key, patch) in &tile.patches {
                match chosen.get(&(key, tile_key)) {
                    None => leaving.push((key, false)),
                    Some(generations)
                        if *generations != patch.generations
                            && fresh.contains_key(&(key, tile_key)) =>
                    {
                        leaving.push((key, generations[0] != patch.generations[0]));
                    }
                    Some(_) => {}
                }
            }
            for (key, resampled) in leaving {
                let patch = tile.patches.remove(&key).expect("listed above");
                retained.readout.replaced_patches += u64::from(resampled);
                self.registry.remove(&NodeKey::Patch(field, x, z, key));
                operations.push(RenderDiff::Destroy {
                    handle: patch.handle,
                });
            }
        }
        for ((key, tile_key), instances) in fresh {
            let Some(generations) = chosen.get(&(key, tile_key)).copied() else {
                continue;
            };
            let tile = retained.tiles.get_mut(&tile_key).expect("a chosen tile");
            if tile.patches.contains_key(&key) {
                continue;
            }
            let (x, z) = tile_key;
            let parent = match tile.node {
                Some(handle) => handle,
                None => {
                    let handle = self.registry.allocate(NodeKey::Tile(field, x, z))?;
                    let origin = [x as f64 * layout.tile_size, z as f64 * layout.tile_size];
                    let mut node = RenderNode::new(Geometry::Group);
                    node.transform = Transform {
                        translation: [origin[0] as f32, 0.0, origin[1] as f32],
                        ..Transform::IDENTITY
                    };
                    operations.push(RenderDiff::Create {
                        handle,
                        parent: root,
                        node,
                    });
                    tile.node = Some(handle);
                    handle
                }
            };
            let layer = &retained.layers[&key].0;
            let handle = self.registry.allocate(NodeKey::Patch(field, x, z, key))?;
            tile.patches.insert(
                key,
                Patch {
                    handle,
                    instances: instances.len() as u32,
                    generations,
                },
            );
            operations.push(RenderDiff::CreateScatterPatch {
                handle,
                parent: Some(parent),
                patch: ScatterPatchDescriptor {
                    asset: layer.mesh.clone(),
                    material_overrides: layer.material_overrides.clone(),
                    instances,
                    fade: layer.fade_band(),
                    shadow_casting: layer.shadow_casting,
                    lods: layer.lods.clone(),
                    lod_band: layer.lod_band,
                    thinning: layer.thinning.clone(),
                },
            });
        }
        let publication_time = publishing.elapsed();

        let readout = &mut retained.readout;
        readout.placement_microseconds = placement_time.as_micros() as u64;
        readout.selection_microseconds = selection_time.as_micros() as u64;
        readout.publication_microseconds = publication_time.as_micros() as u64;
        readout.over_budget = over_budget;
        readout.deferred = deferred;
        readout.tiles = retained
            .tiles
            .values()
            .filter(|tile| tile.content.is_some())
            .count() as u32;
        readout.pending_tiles = retained.tiles.values().filter(|tile| !tile.current).count() as u32;
        readout.patches = retained
            .tiles
            .values()
            .map(|tile| tile.patches.len() as u32)
            .sum();
        readout.instances = retained
            .tiles
            .values()
            .flat_map(|tile| tile.patches.values())
            .map(|patch| u64::from(patch.instances))
            .sum();
        readout.sample_bytes = retained
            .tiles
            .values()
            .filter_map(|tile| tile.content.as_ref())
            .map(|content| content.bytes)
            .sum();
        Ok(operations)
    }
}

fn validate(
    layout: &HeightfieldLayout,
    content: &VegetationTileContent,
) -> Result<(), VegetationError> {
    let samples = &content.samples;
    if samples.colors.is_some() || samples.layer_weights.is_some() {
        return Err(VegetationError::Streams);
    }
    samples.validate(layout).map_err(|error| match error {
        svc_mesh::heightfield::TileSamplesError::HeightCount { expected, actual }
        | svc_mesh::heightfield::TileSamplesError::StreamCount { expected, actual } => {
            VegetationError::SampleCount { expected, actual }
        }
        _ => VegetationError::Value,
    })?;
    if samples
        .materials
        .iter()
        .flatten()
        .any(|class| *class >= VEGETATION_GROUND_CLASSES)
    {
        return Err(VegetationError::Value);
    }
    if let Some(cover) = &content.cover {
        let expected = layout.side().pow(2);
        if cover.len() != expected {
            return Err(VegetationError::SampleCount {
                expected,
                actual: cover.len(),
            });
        }
        if !cover.iter().all(|weight| (0.0..=1.0).contains(weight)) {
            return Err(VegetationError::Value);
        }
    }
    if !content.records.iter().all(|record| {
        record.position.iter().all(|value| value.is_finite())
            && record.yaw.is_finite()
            && record.scale.is_finite()
            && record.scale > 0.0
    }) {
        return Err(VegetationError::Value);
    }
    Ok(())
}

/// A layer's copies on a tile, x and z from the tile's origin and y the
/// world height: its spots the ground, slope, class and cover allow, then
/// its records.
fn place(
    layout: &VegetationLayout,
    (x, z): (i64, i64),
    content: &Content,
    key: u32,
    layer: &VegetationLayer,
) -> Vec<ScatterInstance> {
    let mut copies = Vec::new();
    let size = layout.tile_size;
    let origin = [x as f64 * size, z as f64 * size];
    let cells = layout.tile_cells as usize;
    let cell = size / cells as f64;
    let side = cells + 1;
    let samples = content.surface.samples();
    // The vertex nearest a tile-local place: its class and cover.
    let vertex = |local: f64| ((local / cell).round().max(0.0) as usize).min(cells);
    if layer.density > 0.0 {
        let spacing = 1.0 / f64::from(layer.density).sqrt();
        let minimum_normal_y = f64::from(layer.slope_limit_degrees).to_radians().cos() as f32;
        let first = origin.map(|value| (value / spacing).floor() as i64);
        let last = origin.map(|value| ((value + size) / spacing).floor() as i64);
        'cells: for cell_z in first[1]..=last[1] {
            for cell_x in first[0]..=last[0] {
                let spot = hash(layer.seed, cell_x, cell_z, 0);
                let world_x = (cell_x as f64 + unit(spot)) * spacing;
                let world_z = (cell_z as f64 + unit(spot >> 32)) * spacing;
                let (local_x, local_z) = (world_x - origin[0], world_z - origin[1]);
                if !((0.0..size).contains(&local_x) && (0.0..size).contains(&local_z)) {
                    continue;
                }
                let Some((height, normal)) = content.surface.surface(local_x, local_z) else {
                    continue;
                };
                let length = normal.iter().map(|value| value * value).sum::<f32>().sqrt();
                if length <= f32::EPSILON {
                    continue;
                }
                let normal = normal.map(|value| value / length);
                if normal[1] < minimum_normal_y || normal[1] <= 0.0 {
                    continue;
                }
                let at = vertex(local_z) * side + vertex(local_x);
                let class = samples.materials.as_ref().map_or(0, |classes| classes[at]);
                if layer.grounds & (1 << class) == 0 {
                    continue;
                }
                if let Some(cover) = &content.cover {
                    let weight = cover[at];
                    if weight < 1.0
                        && unit(hash(layer.seed ^ COVER_SALT, cell_x, cell_z, 0)) as f32 >= weight
                    {
                        continue;
                    }
                }
                let band = f64::from(height).floor() as i64;
                copies.push(copy_at(
                    [local_x as f32, height, local_z as f32],
                    normal,
                    hash(layer.seed ^ KEY_SALT, cell_x, cell_z, band),
                    layer.align,
                    layer.scale,
                    layer.tints,
                ));
                if copies.len() >= MAX_SCATTER_PATCH_INSTANCES {
                    break 'cells;
                }
            }
        }
    }
    for record in content.records.iter().filter(|record| record.layer == key) {
        if copies.len() >= MAX_SCATTER_PATCH_INSTANCES {
            break;
        }
        let [local_x, _, local_z] = record.position.map(f64::from);
        let normal = content
            .surface
            .surface(local_x.clamp(0.0, size), local_z.clamp(0.0, size))
            .map(|(_, normal)| {
                let length = normal.iter().map(|value| value * value).sum::<f32>().sqrt();
                normal.map(|value| value / length.max(f32::EPSILON))
            })
            .unwrap_or([0.0, 1.0, 0.0]);
        let key = hash(
            layer.seed ^ KEY_SALT,
            i64::from(record.position[0].to_bits()),
            i64::from(record.position[2].to_bits()),
            1,
        );
        let shade = unit(key) as f32;
        copies.push(ScatterInstance {
            translation: record.position,
            rotation: multiply(lean(normal, layer.align), about_y(record.yaw)),
            scale: record.scale,
            tint: std::array::from_fn(|channel| {
                layer.tints[0][channel]
                    + (layer.tints[1][channel] - layer.tints[0][channel]) * shade
            }),
            rank: unit(key >> 32) as f32,
        });
    }
    copies
}

#[cfg(test)]
#[path = "vegetation_tests.rs"]
mod tests;
