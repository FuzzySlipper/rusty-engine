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
//! A hidden field draws nothing, shadows included, and holds still: its
//! tiles and patches stay for when it is shown again.
//!
//! Placing is the costly step: it runs on the fields' own worker threads,
//! at most `placements_per_settle` tile layers of a field at once, and a
//! later settle takes each finished placement in whole. Until then a tile
//! layer draws what it had (nothing, or its copies placed before), and what
//! waits is counted.
//!
//! Choosing is the update's own cost: tiles × layers candidates for a far
//! field. A field keeps its last choice, and a settle that only takes in
//! placements patches it: the landed counts, the layers whose budgets they
//! touch, and their patches, drawing what choosing from scratch would.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use render_model::{
    Geometry, MeshMaterialSlot, RenderDiff, RenderHandle, RenderLayer, RenderNode, ScatterFade,
    ScatterInstance, ScatterLod, ScatterPatchDescriptor, ScatterThinning, ShadowCasting, Transform,
    MAX_SCATTER_PATCH_INSTANCES,
};
use svc_mesh::heightfield::{HeightfieldLayout, HeightfieldRingSpec, TileSamples, TileSurface};
use svc_mesh::scatter::{hash, unit};

use crate::voxel_scatter::{about_y, copy_at, lean, multiply};
use crate::{HandleAllocationError, RenderHandleNamespace, StableHandleRegistry};

/// A tile is kept, and a layer's patch on it, until it is this many times
/// the reach (widened by a selection step) away, so a focus on the
/// boundary does not rebuild it.
pub const VEGETATION_HYSTERESIS: f64 = 1.1;

/// A settle chooses again only once the focus has moved this share of a
/// tile from where it last chose (or the field changed); choosing, it
/// widens every reach by that much, so the tiles it keeps cover any focus
/// within it.
pub const VEGETATION_SELECTION_STEP: f64 = 1.0 / 16.0;

/// Ground classes are 0 to 63: a layer's `grounds` holds a bit for each.
pub const VEGETATION_GROUND_CLASSES: u32 = 64;

/// The most copies per square metre a layer may ask for.
pub const MAX_VEGETATION_DENSITY: f32 = 64.0;

/// Mixed into a layer's seed for the draw that thins a spot by its cover.
const COVER_SALT: u64 = 0xc07e_5a17;
/// The keys a spot's copy is drawn from, as voxel scatter keys them.
const KEY_SALT: u64 = 0x5ca7_7e7a;

/// How a field is tiled and placed: square tiles of `tile_size` field units,
/// sampled every `tile_size / tile_cells`, drawn in `layer` with one field
/// unit spanning `scale` layer units. Every distance a field is given or
/// reports (tiles, focus, reach, fade, levels and thinning) is in field
/// units; in the scene at scale 1 they are metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VegetationLayout {
    pub tile_size: f64,
    pub tile_cells: u32,
    pub layer: RenderLayer,
    pub scale: f64,
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

    /// The fade bands in layer units, `units` to a field unit.
    fn fade_band(&self, units: f32) -> ScatterFade {
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
            start: start * units,
            end: end * units,
            near_start: near_start * units,
            near_end: near_end * units,
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
    /// Whether the field is drawn.
    pub visible: bool,
    /// Tiles holding samples, and tiles in reach waiting for current ones.
    pub tiles: u32,
    pub pending_tiles: u32,
    /// Patches drawn and the copies they hold.
    pub patches: u32,
    pub instances: u64,
    /// Tile layers in reach a layer's instance budget left bare.
    pub over_budget: u32,
    /// Tile layers in reach with samples waiting for a placement: being
    /// placed on workers, or waiting for one.
    pub deferred: u32,
    /// The placements (tile layers) the last settle took in from workers
    /// and the copies they made, the workers' time placing them, and the
    /// settle's own time choosing what is drawn and turning that into
    /// renderer work.
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
    /// Since the field was made: settles that had nothing to choose again
    /// (no change, the focus within a step of the last choice), and those
    /// that chose again.
    pub settles_skipped: u64,
    pub settles_reselected: u64,
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
    /// The layers its records name, in order: a layer of records alone
    /// (no density) grows nothing on a tile without its own.
    recorded: Vec<u32>,
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

/// A placement a worker finished and a settle has not taken in yet.
#[derive(Debug)]
struct Landed {
    generations: [u64; 2],
    instances: Vec<ScatterInstance>,
    microseconds: u64,
}

/// A layer's candidate on a tile the field last chose from: the tile's
/// rank among them nearest first, the generations it is placed for, its
/// nearest distance from where the field chose, and its count when known;
/// its tile's patch and bare entry for the layer, kept as they change; and
/// what its last walk made of it: the copies it holds of its layer's budget
/// and the generations chosen.
#[derive(Debug)]
struct Candidate {
    tile: (i64, i64),
    rank: usize,
    generations: [u64; 2],
    near: f64,
    known: Option<u32>,
    drawn: Option<([u64; 2], u32)>,
    bare: Option<([u64; 2], u32)>,
    contribution: u32,
    chosen: Option<[u64; 2]>,
    /// Its copies landed and are not yet published.
    fresh: bool,
}

impl Candidate {
    /// Its count as a choice from scratch finds it: from its patch or its
    /// bare entry, made for its generations.
    fn counted(&self) -> Option<u32> {
        match (self.drawn, self.bare) {
            (Some((made, count)), _) if made == self.generations => Some(count),
            (_, Some((made, count))) if made == self.generations => Some(count),
            _ => None,
        }
    }
}

/// No candidate of a layer at a tile rank.
const NO_CANDIDATE: u32 = u32::MAX;

/// A layer's candidates nearest first, each tile rank's candidate, the
/// copies they hold of its budget, and the candidates its last walk left
/// out for the budget.
#[derive(Debug, Default)]
struct LayerChoice {
    candidates: Vec<Candidate>,
    by_rank: Vec<u32>,
    total: u64,
    over_budget: u32,
}

/// What a field last chose from, kept while only placements land.
#[derive(Debug, Default)]
struct Selection {
    /// The tiles with samples nearest first, with their samples; and each
    /// tile's rank, made when a patched choice first looks one up.
    tiles: Vec<(i64, i64)>,
    contents: Vec<Arc<Content>>,
    ranks: Option<HashMap<(i64, i64), usize>>,
    layers: BTreeMap<u32, LayerChoice>,
    /// Candidates of unknown count not being placed, by tile rank then layer:
    /// they start as the field's placements at once leave room.
    waiting: BTreeSet<(usize, u32)>,
}

impl Selection {
    /// A layer's candidate at a tile rank.
    fn at(&self, key: u32, rank: usize) -> Option<usize> {
        let index = *self.layers.get(&key)?.by_rank.get(rank)?;
        (index != NO_CANDIDATE).then_some(index as usize)
    }

    /// A slot's candidate.
    fn find(&mut self, (key, tile): Slot) -> Option<usize> {
        let tiles = &self.tiles;
        let ranks = self.ranks.get_or_insert_with(|| {
            tiles
                .iter()
                .enumerate()
                .map(|(rank, tile)| (*tile, rank))
                .collect()
        });
        let rank = *ranks.get(&tile)?;
        self.at(key, rank)
    }
}

/// What a choice walks: the layers chosen again within their budgets, the
/// landed candidates of the others, and whether it is a choice from scratch.
struct Choice {
    walk: Vec<u32>,
    landed: Vec<(u32, usize)>,
    whole: bool,
}

/// Whether a candidate at `near` is within a layer's reach, wider while the
/// tile holds its patch.
fn within(layer: &VegetationLayer, near: f64, held: bool, step: f64) -> bool {
    near <= f64::from(layer.reach) * if held { VEGETATION_HYSTERESIS } else { 1.0 } + step
}

#[derive(Debug)]
struct Field {
    layout: VegetationLayout,
    heightfield: HeightfieldLayout,
    layers: BTreeMap<u32, (Arc<VegetationLayer>, u64)>,
    placements_per_settle: u32,
    /// Tile layers being placed on workers, for the generations asked for,
    /// and those finished, which the next settle takes in.
    placing: BTreeMap<Slot, [u64; 2]>,
    landed: BTreeMap<Slot, Landed>,
    /// The last choice, patched while only placements land.
    selection: Option<Selection>,
    tiles: BTreeMap<(i64, i64), Tile>,
    focus: Option<[f64; 3]>,
    placement: Option<[f32; 3]>,
    root: Option<RenderHandle>,
    /// Whether its root, and so everything it grows, is drawn, and whether
    /// the root was last drawn so (the settle brings it to `visible`).
    visible: bool,
    shown: bool,
    next_revision: u32,
    next_generation: u64,
    readout: VegetationReadout,
    /// Its layers or tiles changed since the last choice.
    changed: bool,
    /// The focus the last choice was made for.
    chosen_at: Option<[f64; 3]>,
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
#[derive(Debug)]
pub struct VegetationFields {
    registry: StableHandleRegistry<NodeKey>,
    fields: BTreeMap<u64, Field>,
    /// The workers placing for every field, started on first use.
    placer: Option<Placer>,
    /// Tests: every choice from scratch, landings or not.
    full_choices: bool,
    /// Tests: the choices patched after landings.
    #[cfg(test)]
    patched_choices: u64,
}

impl Default for VegetationFields {
    fn default() -> Self {
        Self {
            registry: StableHandleRegistry::new(RenderHandleNamespace::VEGETATION),
            fields: BTreeMap::new(),
            placer: None,
            full_choices: false,
            #[cfg(test)]
            patched_choices: 0,
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
        if !(layout.tile_size.is_finite()
            && layout.tile_size > 0.0
            && layout.scale.is_finite()
            && layout.scale > 0.0)
        {
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
                placing: BTreeMap::new(),
                landed: BTreeMap::new(),
                selection: None,
                tiles: BTreeMap::new(),
                focus: None,
                placement: None,
                root: None,
                visible: true,
                shown: true,
                next_revision: 0,
                next_generation: 0,
                readout: VegetationReadout::default(),
                changed: true,
                chosen_at: None,
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

    /// Draw the field or not, from its root down, shadows included, from
    /// the next settle (so a field removed first changes nothing). Its tiles
    /// and patches stay as they are.
    pub fn set_visible(&mut self, field: u64, visible: bool) -> Result<(), VegetationError> {
        self.field(field)?.visible = visible;
        Ok(())
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
        if retained.layers.get(&key).map(|(current, _)| &**current) == Some(&layer) {
            return Ok(());
        }
        let generation = retained.generation();
        retained.layers.insert(key, (Arc::new(layer), generation));
        retained.changed = true;
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
        retained.changed = true;
        retained.placing.retain(|(layer, _), _| *layer != key);
        retained.landed.retain(|(layer, _), _| *layer != key);
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
        let mut recorded: Vec<u32> = records.iter().map(|record| record.layer).collect();
        recorded.sort_unstable();
        recorded.dedup();
        tile.content = Some(Arc::new(Content {
            surface: TileSurface::new(&retained.heightfield, 0, samples),
            cover,
            records,
            recorded,
            heights: if heights[0] <= heights[1] {
                heights
            } else {
                [0.0; 2]
            },
            generation,
            bytes,
        }));
        tile.current = true;
        retained.changed = true;
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
            retained.changed = true;
        }
        Ok(())
    }

    pub fn readout(&self, field: u64) -> Option<VegetationReadout> {
        self.fields.get(&field).map(|field| VegetationReadout {
            visible: field.visible,
            ..field.readout
        })
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
        self.take_in(false);
        let mut retained = self.fields.remove(&field).ok_or(VegetationError::Field)?;
        let result = self.settle_field(field, &mut retained, placement, focus);
        self.fields.insert(field, retained);
        result
    }

    /// Wait until every field's placements on workers have finished; each
    /// field's next settle takes them in.
    pub fn wait_for_placement(&mut self) {
        self.take_in(true);
    }

    /// Hold what workers have finished for the settles to take in; with
    /// `wait`, until no field has a placement on workers.
    fn take_in(&mut self, wait: bool) {
        let Some(placer) = &self.placer else {
            return;
        };
        loop {
            let placing = wait && self.fields.values().any(|field| !field.placing.is_empty());
            let finished = if placing {
                placer.finished.recv().ok()
            } else {
                placer.finished.try_recv().ok()
            };
            let Some(placed) = finished else {
                return;
            };
            // A placement asked for again since, or for a tile, layer or
            // field gone since, is dropped.
            let Some(retained) = self.fields.get_mut(&placed.field) else {
                continue;
            };
            if retained.placing.get(&placed.slot) != Some(&placed.generations) {
                continue;
            }
            retained.placing.remove(&placed.slot);
            retained.landed.insert(
                placed.slot,
                Landed {
                    generations: placed.generations,
                    instances: placed.instances,
                    microseconds: placed.microseconds,
                },
            );
        }
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
            scale: [retained.layout.scale as f32; 3],
            ..Transform::IDENTITY
        };
        match retained.root {
            None => {
                let handle = self.registry.allocate(NodeKey::Root(field))?;
                let mut node = RenderNode::new(Geometry::Group);
                node.transform = transform;
                node.layer = retained.layout.layer;
                node.visible = retained.visible;
                operations.push(RenderDiff::Create {
                    handle,
                    parent: None,
                    node,
                });
                retained.root = Some(handle);
            }
            Some(handle)
                if retained.placement != Some(placement) || retained.shown != retained.visible =>
            {
                operations.push(RenderDiff::Update {
                    handle,
                    transform: (retained.placement != Some(placement)).then_some(transform),
                    material: None,
                    visible: (retained.shown != retained.visible).then_some(retained.visible),
                    metadata: None,
                });
            }
            Some(_) => {}
        }
        retained.shown = retained.visible;
        retained.placement = Some(placement);
        // A hidden field holds still, so its tiles are there when it shows.
        if focus.is_some() && retained.visible {
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
        // Nothing to choose again: no change (a placement landing is one),
        // the focus within a step of the last choice, whose widened reaches
        // cover it. Placements left waiting while a field's are all on
        // workers start as those land.
        let step = retained.layout.tile_size * VEGETATION_SELECTION_STEP;
        let near_last = retained.chosen_at.is_some_and(|at| {
            let [dx, dy, dz] = [0, 1, 2].map(|axis| at[axis] - focus[axis]);
            (dx * dx + dy * dy + dz * dz).sqrt() <= step
        });
        // Only placements landed since the last choice, the focus within a
        // step of it: that choice is patched (or, for tests, made again from
        // scratch at the same focus).
        let landing_only = !retained.changed && near_last;
        if landing_only && retained.landed.is_empty() {
            readout.settles_skipped += 1;
            readout.selection_microseconds = started.elapsed().as_micros() as u64;
            readout.publication_microseconds = 0;
            return Ok(operations);
        }
        readout.settles_reselected += 1;
        let at = if landing_only {
            retained
                .chosen_at
                .expect("a landing-only choice follows a choice")
        } else {
            focus
        };
        if landing_only && !self.full_choices {
            if let Some(selection) = retained.selection.take() {
                #[cfg(test)]
                {
                    self.patched_choices += 1;
                }
                return self.patch_choice(field, retained, selection, step, started, operations);
            }
        }
        retained.changed = false;
        retained.chosen_at = Some(at);

        // Residency: the tiles within the farthest reach widened by a step,
        // kept a little past it.
        let reach = retained.reach();
        let leaving: Vec<(i64, i64)> = retained
            .tiles
            .keys()
            .copied()
            .filter(|&key| retained.flat_distance(at, key) > (reach + step) * VEGETATION_HYSTERESIS)
            .collect();
        for key in leaving {
            let tile = retained.tiles.remove(&key).expect("listed above");
            retained.placing.retain(|(_, tile), _| *tile != key);
            self.forget_tile(field, key, &tile);
            if let Some(handle) = tile.node {
                operations.push(RenderDiff::Destroy { handle });
            }
        }
        if reach > 0.0 {
            let size = retained.layout.tile_size;
            let low = [at[0] - reach - step, at[2] - reach - step]
                .map(|value| (value / size).floor() as i64);
            let high = [at[0] + reach + step, at[2] + reach + step]
                .map(|value| (value / size).floor() as i64);
            for x in low[0]..=high[0] {
                for z in low[1]..=high[1] {
                    if retained.tiles.contains_key(&(x, z))
                        || retained.flat_distance(at, (x, z)) > reach + step
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

        // Each tile's distances once, the tiles nearest first; each layer's
        // candidates are those of them within its reach, in order, with what
        // is known of their counts.
        let mut nearest: Vec<((i64, i64), [f64; 2], &Tile)> = retained
            .tiles
            .iter()
            .filter_map(|(&key, tile)| {
                let content = tile.content.as_ref()?;
                Some((key, retained.distances(at, key, Some(content)), tile))
            })
            .collect();
        nearest.sort_by(|a, b| a.1[0].total_cmp(&b.1[0]).then(a.0.cmp(&b.0)));
        let mut landed = std::mem::take(&mut retained.landed);
        let mut fresh: BTreeMap<Slot, Vec<ScatterInstance>> = BTreeMap::new();
        let mut selection = Selection {
            tiles: nearest.iter().map(|(key, _, _)| *key).collect(),
            contents: nearest
                .iter()
                .map(|(_, _, tile)| tile.content.clone().expect("a tile with samples"))
                .collect(),
            ..Selection::default()
        };
        for (&key, (layer, layer_generation)) in &retained.layers {
            let mut choice = LayerChoice {
                by_rank: vec![NO_CANDIDATE; nearest.len()],
                ..LayerChoice::default()
            };
            for (rank, &(tile_key, [near, far], tile)) in nearest.iter().enumerate() {
                let content = tile.content.as_ref().expect("a tile with samples");
                // A layer of records alone has nothing to place on a tile
                // without its records, and nothing of it is drawn there.
                if layer.density <= 0.0 && content.recorded.binary_search(&key).is_err() {
                    continue;
                }
                let drawn = tile
                    .patches
                    .get(&key)
                    .map(|patch| (patch.generations, patch.instances));
                if !within(layer, near, drawn.is_some(), step)
                    || (layer.near > 0.0 && far < f64::from(layer.near) - step)
                {
                    continue;
                }
                let mut candidate = Candidate {
                    tile: tile_key,
                    rank,
                    generations: [content.generation, *layer_generation],
                    near,
                    known: None,
                    drawn,
                    bare: tile.bare.get(&key).copied(),
                    contribution: 0,
                    chosen: None,
                    fresh: false,
                };
                candidate.known = candidate.counted().or_else(|| {
                    let landing = landed.remove(&(key, tile_key))?;
                    if landing.generations != candidate.generations {
                        return None;
                    }
                    let count = landing.instances.len() as u32;
                    let readout = &mut retained.readout;
                    readout.placed += 1;
                    readout.placed_instances += u64::from(count);
                    readout.placement_microseconds += landing.microseconds;
                    fresh.insert((key, tile_key), landing.instances);
                    Some(count)
                });
                candidate.fresh = fresh.contains_key(&(key, tile_key));
                candidate.contribution = candidate
                    .known
                    .or(candidate.drawn.map(|(_, count)| count))
                    .unwrap_or(0);
                choice.total += u64::from(candidate.contribution);
                if candidate.known.is_none()
                    && retained.placing.get(&(key, tile_key)) != Some(&candidate.generations)
                {
                    selection.waiting.insert((rank, key));
                }
                choice.by_rank[rank] = choice.candidates.len() as u32;
                choice.candidates.push(candidate);
            }
            selection.layers.insert(key, choice);
        }
        // What lands for a tile layer no longer a candidate is dropped, and
        // the patches of tile layers no longer candidates go.
        drop(landed);
        let mut leaving: Vec<Slot> = Vec::new();
        for (rank, &(tile_key, _, tile)) in nearest.iter().enumerate() {
            for &key in tile.patches.keys() {
                if selection.at(key, rank).is_none() {
                    leaving.push((key, tile_key));
                }
            }
        }
        drop(nearest);
        for (key, tile_key) in leaving {
            let tile = retained
                .tiles
                .get_mut(&tile_key)
                .expect("a tile with samples");
            let patch = tile.patches.remove(&key).expect("listed above");
            self.registry
                .remove(&NodeKey::Patch(field, tile_key.0, tile_key.1, key));
            operations.push(RenderDiff::Destroy {
                handle: patch.handle,
            });
        }
        let walk: Vec<u32> = selection.layers.keys().copied().collect();
        self.choose(
            field,
            retained,
            &mut selection,
            fresh,
            Choice {
                walk,
                landed: Vec::new(),
                whole: true,
            },
            step,
            started,
            &mut operations,
        )?;
        retained.selection = Some(selection);
        Ok(operations)
    }

    /// Only placements landed since the last choice: they become known
    /// counts in it, and only what they change is chosen and published again.
    fn patch_choice(
        &mut self,
        field: u64,
        retained: &mut Field,
        mut selection: Selection,
        step: f64,
        started: Instant,
        mut operations: Vec<RenderDiff>,
    ) -> Result<Vec<RenderDiff>, VegetationError> {
        let mut fresh: BTreeMap<Slot, Vec<ScatterInstance>> = BTreeMap::new();
        // Each layer's landed candidates and the copies they held before.
        let mut landed: BTreeMap<u32, Vec<(usize, u32)>> = BTreeMap::new();
        for (slot, landing) in std::mem::take(&mut retained.landed) {
            let Some(index) = selection.find(slot) else {
                continue;
            };
            let candidate = &mut selection
                .layers
                .get_mut(&slot.0)
                .expect("a found layer")
                .candidates[index];
            if candidate.generations != landing.generations || candidate.known.is_some() {
                continue;
            }
            let count = landing.instances.len() as u32;
            let readout = &mut retained.readout;
            readout.placed += 1;
            readout.placed_instances += u64::from(count);
            readout.placement_microseconds += landing.microseconds;
            candidate.known = Some(count);
            candidate.fresh = true;
            landed
                .entry(slot.0)
                .or_default()
                .push((index, candidate.contribution));
            fresh.insert(slot, landing.instances);
        }
        // A layer whose budget left nothing out and still holds every copy
        // after the landings chooses its other candidates as before: only
        // the landed ones change. Any other with landings, and any whose
        // budget leaves some out, is walked again.
        let mut walk = Vec::new();
        let mut fast = Vec::new();
        for (&key, choice) in &selection.layers {
            let layer = &retained.layers[&key].0;
            let Some(indices) = landed.get(&key) else {
                if choice.over_budget > 0 {
                    walk.push(key);
                }
                continue;
            };
            let total = indices
                .iter()
                .fold(choice.total, |total, &(index, before)| {
                    total - u64::from(before)
                        + u64::from(choice.candidates[index].known.unwrap_or(0))
                });
            let held = indices.iter().all(|&(index, _)| {
                let candidate = &choice.candidates[index];
                within(layer, candidate.near, candidate.drawn.is_some(), step)
            });
            if choice.over_budget == 0 && total <= u64::from(layer.maximum_instances) && held {
                fast.extend(indices.iter().map(|&(index, _)| (key, index)));
            } else {
                walk.push(key);
            }
        }
        self.choose(
            field,
            retained,
            &mut selection,
            fresh,
            Choice {
                walk,
                landed: fast,
                whole: false,
            },
            step,
            started,
            &mut operations,
        )?;
        retained.selection = Some(selection);
        Ok(operations)
    }

    /// Choose from `selection`: start the waiting placements there is room
    /// for, nearest first across layers; walk the layers named within their
    /// budgets, nearest first, and settle the landed candidates of the others
    /// alone; start the chosen tile layers the budget had left out; publish
    /// what changed; and bring the selection to what a choice from scratch
    /// would now find.
    #[allow(clippy::too_many_arguments)]
    fn choose(
        &mut self,
        field: u64,
        retained: &mut Field,
        selection: &mut Selection,
        mut fresh: BTreeMap<Slot, Vec<ScatterInstance>>,
        choice: Choice,
        step: f64,
        started: Instant,
        operations: &mut Vec<RenderDiff>,
    ) -> Result<(), VegetationError> {
        let placements = retained.placements_per_settle as usize;
        let layout = retained.layout;
        let room = placements.saturating_sub(retained.placing.len());
        let starting: Vec<(usize, u32)> = selection.waiting.iter().take(room).copied().collect();
        for (rank, key) in starting {
            selection.waiting.remove(&(rank, key));
            let index = selection.at(key, rank).expect("a waiting candidate");
            let candidate = &selection.layers[&key].candidates[index];
            let slot = (key, candidate.tile);
            retained.placing.insert(slot, candidate.generations);
            self.placer
                .get_or_insert_with(Placer::new)
                .start(PlacementJob {
                    field,
                    slot,
                    generations: candidate.generations,
                    layout,
                    content: selection.contents[rank].clone(),
                    layer: retained.layers[&key].0.clone(),
                });
        }

        // Choosing: within each layer's budget, nearest first. A tile layer
        // waiting to be placed again counts and draws what it has meanwhile.
        // A tile's bare entry changes only where its count's fate does.
        // Chosen tile layers last counted while the budget left them out
        // need their copies again: they take the room left, nearest first in
        // each layer. Each candidate is brought to what a choice from
        // scratch would find of it (its count, the copies it holds of its
        // layer's budget, whether it waits to be placed) as it goes, and
        // again where publication changes its patch.
        let Selection {
            layers,
            waiting,
            contents,
            ..
        } = &mut *selection;
        let mut late: Vec<(Slot, [u64; 2], usize)> = Vec::new();
        let mut publish: Vec<(u32, usize)> = Vec::new();
        for &key in &choice.walk {
            let layer = &retained.layers[&key].0;
            let mut budget = u64::from(layer.maximum_instances);
            let walked = layers.get_mut(&key).expect("a walked layer");
            walked.over_budget = 0;
            for (index, candidate) in walked.candidates.iter_mut().enumerate() {
                candidate.chosen = None;
                let slot = (key, candidate.tile);
                if within(layer, candidate.near, candidate.drawn.is_some(), step) {
                    let counted = match (candidate.known, candidate.drawn) {
                        (Some(count), _) => Some((count, candidate.generations)),
                        (None, Some((generations, count))) => Some((count, generations)),
                        (None, None) => None,
                    };
                    if let Some((count, generations)) = counted {
                        // Bare ground draws nothing until it changes; past
                        // the budget, it is counted.
                        let bare = if count == 0 {
                            Some((generations, 0))
                        } else if u64::from(count) > budget {
                            walked.over_budget += 1;
                            Some((generations, count))
                        } else {
                            budget -= u64::from(count);
                            None
                        };
                        if bare != candidate.bare {
                            set_bare(&mut retained.tiles, candidate, key, bare);
                            let before = candidate.contribution;
                            let after =
                                derive(layer, key, candidate, step, &retained.placing, waiting);
                            walked.total = walked.total - u64::from(before) + u64::from(after);
                        }
                        if bare.is_none() {
                            candidate.chosen = Some(generations);
                            if candidate.drawn.map(|(drawn, _)| drawn) != Some(generations)
                                && !candidate.fresh
                                && retained.placing.get(&slot) != Some(&generations)
                            {
                                late.push((slot, generations, candidate.rank));
                            }
                        }
                    }
                }
                if (candidate.drawn.is_some() && candidate.chosen.is_none()) || candidate.fresh {
                    publish.push((key, index));
                }
            }
        }
        for &(key, index) in &choice.landed {
            let layer = &retained.layers[&key].0;
            let landed = layers.get_mut(&key).expect("a landed layer");
            let candidate = &mut landed.candidates[index];
            let count = candidate.known.expect("a landed count");
            let bare = (count == 0).then_some((candidate.generations, 0));
            if bare != candidate.bare {
                set_bare(&mut retained.tiles, candidate, key, bare);
            }
            candidate.chosen = bare.is_none().then_some(candidate.generations);
            publish.push((key, index));
            let before = candidate.contribution;
            let after = derive(layer, key, candidate, step, &retained.placing, waiting);
            landed.total = landed.total - u64::from(before) + u64::from(after);
        }
        let room = placements.saturating_sub(retained.placing.len());
        late.truncate(room);
        for (slot, generations, rank) in late {
            retained.placing.insert(slot, generations);
            waiting.remove(&(rank, slot.0));
            self.placer
                .get_or_insert_with(Placer::new)
                .start(PlacementJob {
                    field,
                    slot,
                    generations,
                    layout,
                    content: contents[rank].clone(),
                    layer: retained.layers[&slot.0].0.clone(),
                });
        }
        let selection_time = started.elapsed();

        // Publication: drop the patches no longer chosen, replace those
        // whose new copies are ready, and make the new ones.
        let publishing = Instant::now();
        let mut publication = Publication {
            field,
            layout,
            root: retained.root,
            replaced: 0,
            patches: 0,
            instances: 0,
        };
        let Selection {
            layers, waiting, ..
        } = &mut *selection;
        for (key, index) in publish {
            let layer = &retained.layers[&key].0;
            let published = layers.get_mut(&key).expect("a published layer");
            let candidate = &mut published.candidates[index];
            publication.publish(
                &mut self.registry,
                &mut retained.tiles,
                layer,
                key,
                candidate,
                &mut fresh,
                operations,
            )?;
            candidate.fresh = false;
            let before = candidate.contribution;
            let after = derive(layer, key, candidate, step, &retained.placing, waiting);
            published.total = published.total - u64::from(before) + u64::from(after);
        }
        let publication_time = publishing.elapsed();

        let readout = &mut retained.readout;
        readout.replaced_patches += publication.replaced;
        readout.selection_microseconds = selection_time.as_micros() as u64;
        readout.publication_microseconds = publication_time.as_micros() as u64;
        readout.over_budget = selection
            .layers
            .values()
            .map(|layer| layer.over_budget)
            .sum();
        readout.deferred = (selection.waiting.len() + retained.placing.len()) as u32;
        if choice.whole {
            readout.tiles = retained
                .tiles
                .values()
                .filter(|tile| tile.content.is_some())
                .count() as u32;
            readout.pending_tiles =
                retained.tiles.values().filter(|tile| !tile.current).count() as u32;
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
        } else {
            readout.patches = (i64::from(readout.patches) + publication.patches) as u32;
            readout.instances = (readout.instances as i64 + publication.instances) as u64;
        }
        Ok(())
    }
}

/// Bring a candidate to what a choice from scratch would find of it: its
/// count from its patch or bare entry, whether it waits to be placed, and
/// (returned) the copies it holds of its layer's budget.
fn derive(
    layer: &VegetationLayer,
    key: u32,
    candidate: &mut Candidate,
    step: f64,
    placing: &BTreeMap<Slot, [u64; 2]>,
    waiting: &mut BTreeSet<(usize, u32)>,
) -> u32 {
    let member = within(layer, candidate.near, candidate.drawn.is_some(), step);
    candidate.known = candidate.counted();
    candidate.contribution = if member {
        candidate
            .known
            .or(candidate.drawn.map(|(_, count)| count))
            .unwrap_or(0)
    } else {
        0
    };
    if member
        && candidate.known.is_none()
        && placing.get(&(key, candidate.tile)) != Some(&candidate.generations)
    {
        waiting.insert((candidate.rank, key));
    } else {
        waiting.remove(&(candidate.rank, key));
    }
    candidate.contribution
}

/// Set a candidate's bare entry, on its tile too.
fn set_bare(
    tiles: &mut BTreeMap<(i64, i64), Tile>,
    candidate: &mut Candidate,
    key: u32,
    bare: Option<([u64; 2], u32)>,
) {
    let tile = tiles.get_mut(&candidate.tile).expect("a candidate's tile");
    match bare {
        Some(entry) => tile.bare.insert(key, entry),
        None => tile.bare.remove(&key),
    };
    candidate.bare = bare;
}

/// A choice's publication: where its patches stand, and what it changed.
struct Publication {
    field: u64,
    layout: VegetationLayout,
    root: Option<RenderHandle>,
    replaced: u64,
    patches: i64,
    instances: i64,
}

impl Publication {
    /// Drop a candidate's patch if it is no longer chosen or its new copies
    /// are ready, and make its new one.
    #[allow(clippy::too_many_arguments)]
    fn publish(
        &mut self,
        registry: &mut StableHandleRegistry<NodeKey>,
        tiles: &mut BTreeMap<(i64, i64), Tile>,
        layer: &VegetationLayer,
        key: u32,
        candidate: &mut Candidate,
        fresh: &mut BTreeMap<Slot, Vec<ScatterInstance>>,
        operations: &mut Vec<RenderDiff>,
    ) -> Result<(), VegetationError> {
        let slot = (key, candidate.tile);
        let (x, z) = candidate.tile;
        let field = self.field;
        if let Some((drawn, count)) = candidate.drawn {
            let leaving = match candidate.chosen {
                None => Some(false),
                Some(generations) if generations != drawn && fresh.contains_key(&slot) => {
                    Some(generations[0] != drawn[0])
                }
                Some(_) => None,
            };
            if let Some(resampled) = leaving {
                let tile = tiles.get_mut(&candidate.tile).expect("a candidate's tile");
                let patch = tile.patches.remove(&key).expect("a drawn patch");
                self.replaced += u64::from(resampled);
                registry.remove(&NodeKey::Patch(field, x, z, key));
                operations.push(RenderDiff::Destroy {
                    handle: patch.handle,
                });
                self.patches -= 1;
                self.instances -= i64::from(count);
                candidate.drawn = None;
            }
        }
        let Some(generations) = candidate.chosen else {
            return Ok(());
        };
        if candidate.drawn.is_some() {
            return Ok(());
        }
        let Some(copies) = fresh.remove(&slot) else {
            return Ok(());
        };
        let tile = tiles.get_mut(&candidate.tile).expect("a candidate's tile");
        let parent = match tile.node {
            Some(handle) => handle,
            None => {
                let handle = registry.allocate(NodeKey::Tile(field, x, z))?;
                let size = self.layout.tile_size;
                let mut node = RenderNode::new(Geometry::Group);
                node.transform = Transform {
                    translation: [(x as f64 * size) as f32, 0.0, (z as f64 * size) as f32],
                    ..Transform::IDENTITY
                };
                operations.push(RenderDiff::Create {
                    handle,
                    parent: self.root,
                    node,
                });
                tile.node = Some(handle);
                handle
            }
        };
        let units = self.layout.scale as f32;
        let handle = registry.allocate(NodeKey::Patch(field, x, z, key))?;
        let count = copies.len() as u32;
        tile.patches.insert(
            key,
            Patch {
                handle,
                instances: count,
                generations,
            },
        );
        candidate.drawn = Some((generations, count));
        self.patches += 1;
        self.instances += i64::from(count);
        operations.push(RenderDiff::CreateScatterPatch {
            handle,
            parent: Some(parent),
            patch: ScatterPatchDescriptor {
                asset: layer.mesh.clone(),
                material_overrides: layer.material_overrides.clone(),
                instances: copies,
                // The renderer measures from the camera in layer units.
                fade: layer.fade_band(units),
                shadow_casting: layer.shadow_casting,
                lods: layer
                    .lods
                    .iter()
                    .map(|lod| ScatterLod {
                        start: lod.start * units,
                        ..lod.clone()
                    })
                    .collect(),
                lod_band: layer.lod_band * units,
                thinning: ScatterThinning {
                    points: layer
                        .thinning
                        .points
                        .iter()
                        .map(|&[distance, share]| [distance * units, share])
                        .collect(),
                    ..layer.thinning.clone()
                },
            },
        });
        Ok(())
    }
}

/// Workers at most: placing is the cost, and the product's thread, the draw
/// and the shared worker pool keep theirs.
const MAX_PLACING_WORKERS: usize = 4;

/// A field's layer on a tile to place, for the generations it was asked for.
struct PlacementJob {
    field: u64,
    slot: Slot,
    generations: [u64; 2],
    layout: VegetationLayout,
    content: Arc<Content>,
    layer: Arc<VegetationLayer>,
}

/// What a worker placed, and its time placing it.
struct Placed {
    field: u64,
    slot: Slot,
    generations: [u64; 2],
    instances: Vec<ScatterInstance>,
    microseconds: u64,
}

/// The threads that place tile layers, and what they finish.
struct Placer {
    jobs: Option<mpsc::Sender<PlacementJob>>,
    finished_sender: mpsc::Sender<Placed>,
    finished: mpsc::Receiver<Placed>,
}

impl std::fmt::Debug for Placer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Placer").finish_non_exhaustive()
    }
}

impl Placer {
    fn new() -> Self {
        let (finished_sender, finished) = mpsc::channel();
        let (jobs, queue) = mpsc::channel::<PlacementJob>();
        let queue = Arc::new(Mutex::new(queue));
        let count = std::thread::available_parallelism()
            .map_or(1, |threads| threads.get() / 2)
            .clamp(1, MAX_PLACING_WORKERS);
        let mut started = 0;
        for _ in 0..count {
            let queue = Arc::clone(&queue);
            let finished = finished_sender.clone();
            let spawned = std::thread::Builder::new()
                .name("rusty-vegetation-place".to_owned())
                .spawn(move || loop {
                    let job = queue
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .recv();
                    match job {
                        Ok(job) => run(job, &finished),
                        // The fields are gone.
                        Err(_) => return,
                    }
                });
            started += usize::from(spawned.is_ok());
        }
        Self {
            jobs: (started > 0).then_some(jobs),
            finished_sender,
            finished,
        }
    }

    fn start(&self, job: PlacementJob) {
        match &self.jobs {
            Some(jobs) => {
                if let Err(mpsc::SendError(job)) = jobs.send(job) {
                    run(job, &self.finished_sender);
                }
            }
            // No thread to spare: placed here, taken in as one that was.
            None => run(job, &self.finished_sender),
        }
    }
}

fn run(job: PlacementJob, finished: &mpsc::Sender<Placed>) {
    let started = Instant::now();
    let (key, tile) = job.slot;
    let instances = place(&job.layout, tile, &job.content, key, &job.layer);
    // Fields dropped since have no use for it.
    let _ = finished.send(Placed {
        field: job.field,
        slot: job.slot,
        generations: job.generations,
        instances,
        microseconds: started.elapsed().as_micros() as u64,
    });
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
