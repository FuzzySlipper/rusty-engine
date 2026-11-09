//! Playing a rig, and the pose one step produces.

use std::ops::Range;

use crate::clip::{self, Cut, Plan};
use crate::constraints::{self, ConstraintKind};
use crate::mixer::{Mixer, Working};
use crate::physics::Physics;
use crate::rig::{AnimationData, BlendMode, Geometry, Rig, TimelineKind, VertexData};
use crate::timeline::{key_index_at, order_of, sequence_frame_at};
use crate::world::{world_transforms, BoneWorld};

/// One drawn slot, in draw order.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub slot: usize,
    /// The attachment the slot shows.
    pub attachment: usize,
    /// What it draws with: [`Rig::drawable`] gives its page, UVs and
    /// triangles, which never change, unless a clip cut it.
    pub drawable: usize,
    /// The page it draws from.
    pub page: usize,
    /// Its posed vertices in [`Pose::vertices`], one per UV.
    pub vertices: Range<usize>,
    /// Where a clipping attachment cut it, its own UVs and triangles in
    /// place of its drawable's: [`Pose::uvs`] and [`Pose::triangles`] over
    /// its vertices, which are what the cut left. None when no clip covers
    /// it, or the clip over it cut nothing.
    pub clipped: Option<Clipped>,
    pub blend: BlendMode,
    /// The slot's colour times the attachment's, RGBA from 0 to 1.
    pub color: [f32; 4],
    /// The slot's two-colour tint dark colour, when it has one.
    pub dark: Option<[f32; 3]>,
}

/// A piece's geometry as a clip cut it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clipped {
    /// Page UVs in [`Pose::uvs`], one per vertex of the piece.
    pub uvs: Range<usize>,
    /// Indices into the piece's own vertices, three per triangle, in
    /// [`Pose::triangles`]. Empty when the clip cut it all away.
    pub triangles: Range<usize>,
}

/// An event a step's clip time passed.
#[derive(Debug, Clone, PartialEq)]
pub struct FiredEvent {
    pub event: usize,
    /// The key's time in its clip, in seconds.
    pub time: f64,
    pub int: i64,
    pub float: f64,
    pub string: String,
    pub volume: f64,
    pub balance: f64,
}

/// A rig's pose after one step: every bone's world transform, and the
/// drawn slots in draw order with their vertices.
///
/// Positions are rig units, y up, in the rig's own space.
#[derive(Debug, Default, Clone)]
pub struct Pose {
    pub bones: Vec<BoneWorld>,
    pub pieces: Vec<Piece>,
    pub vertices: Vec<[f32; 2]>,
    /// The page UVs of clipped pieces ([`Piece::clipped`]).
    pub uvs: Vec<[f32; 2]>,
    /// The triangles of clipped pieces ([`Piece::clipped`]).
    pub triangles: Vec<u32>,
    pub events: Vec<FiredEvent>,
    /// Track 0's playing clip's time, in seconds.
    pub animation_time: f64,
}

/// How a clip starts playing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Play {
    pub looping: bool,
    /// Seconds over which the clip fades in over whatever played before.
    pub crossfade: f64,
}

/// How a clip layers over the tracks below its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layer {
    /// From 0 to 1, multiplied into every one of the clip's timelines: how
    /// far it moves the pose from what the tracks below posed toward its
    /// own. Track 0 below 1 moves the setup pose part way.
    pub alpha: f64,
    /// Adds the clip's keyed bone values, deforms, transform mixes and
    /// physics wind and gravity (times `alpha`) to what the tracks below
    /// posed, rather than replacing them: a breathing layer or a flinch
    /// over a walk. Its colours, attachments and other values replace, as
    /// a replacing clip's do.
    pub additive: bool,
}

impl Default for Layer {
    fn default() -> Layer {
        Layer {
            alpha: 1.0,
            additive: false,
        }
    }
}

/// What a track plays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackState {
    pub animation: usize,
    pub looping: bool,
    /// Seconds since the clip started playing.
    pub track_time: f64,
    /// The clip time it poses at: the track time wrapped by the clip's
    /// duration when looping, else held at it.
    pub animation_time: f64,
    pub alpha: f64,
    pub additive: bool,
}

/// One of a bone's local values, as a product drives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoneChannel {
    /// Degrees counter-clockwise.
    Rotation,
    X,
    Y,
    ScaleX,
    ScaleY,
    ShearX,
    ShearY,
}

/// One instance of a rig playing clips on layered tracks.
///
/// Each track plays one clip at a time, crossfading from the clips before
/// it, over the tracks below it: track 0 is the base, a higher track
/// replaces or adds to what they posed by its clip's [`Layer`]. Each
/// [`Player::advance`] moves every track by a step and poses the rig once
/// from its setup pose: the tracks lowest first, then the driven values,
/// then the bones' world transforms and the constraints. Events fire from
/// every track's playing clip; draw order and sequence frames follow track
/// 0's.
///
/// Physics constraints carry their state from step to step: a pose depends
/// on every step taken since the player was made, so a player advanced by
/// the same steps poses the same. The player's first step resets them.
#[derive(Clone)]
pub struct Player {
    mixer: Mixer,
    working: Working,
    skin: Option<usize>,
    order: Vec<usize>,
    parents: Vec<Option<usize>>,
    /// Which constraints the skin applies.
    applied: Vec<bool>,
    driven: Vec<(usize, BoneChannel, f64)>,
    scratch: constraints::Scratch,
    physics: Physics,
    /// A slot's world vertices as doubles, before they are cut or stored.
    world: Vec<[f64; 2]>,
    clipping: clip::Scratch,
}

impl Player {
    /// A player showing the setup pose, under the default skin.
    pub fn new(rig: &Rig) -> Player {
        let mut player = Player {
            mixer: Mixer::default(),
            working: Working::new(),
            skin: None,
            order: Vec::new(),
            parents: rig.bones.iter().map(|b| b.parent).collect(),
            applied: Vec::new(),
            driven: Vec::new(),
            scratch: constraints::Scratch::default(),
            physics: Physics::new(rig.constraints.len()),
            world: Vec::new(),
            clipping: clip::Scratch::default(),
        };
        player.set_skin(rig, None);
        player
    }

    /// Shows a named skin, falling back to the default skin for slots it
    /// does not fill; `None` shows the default skin alone. A skin-required
    /// bone is active only under a skin that names it or a bone below it,
    /// and a skin-required constraint applies only under a skin that lists
    /// it.
    pub fn set_skin(&mut self, rig: &Rig, skin: Option<usize>) {
        self.applied.clear();
        self.applied
            .extend(rig.constraints.iter().enumerate().map(|(i, c)| {
                !c.skin_required || skin.is_some_and(|s| rig.skins[s].constraints.contains(&i))
            }));
        self.skin = skin;
        let w = &mut self.working;
        w.lookup.clear();
        w.lookup.extend(skin);
        if let Some(default) = rig.default_skin.filter(|d| Some(*d) != skin) {
            w.lookup.push(default);
        }
        let mut reached = vec![false; rig.bones.len()];
        for &named in skin.iter().flat_map(|s| rig.skins[*s].bones.iter()) {
            let mut at = Some(named);
            while let Some(b) = at.filter(|b| !reached[*b]) {
                reached[b] = true;
                at = rig.bones[b].parent;
            }
        }
        w.active.clear();
        w.active.extend(
            rig.bones
                .iter()
                .enumerate()
                .map(|(i, b)| !b.skin_required || reached[i]),
        );
    }

    pub fn skin(&self) -> Option<usize> {
        self.skin
    }

    /// Plays a clip on track 0 from its start, fully, fading in over what
    /// played before.
    pub fn play(&mut self, animation: usize, play: Play) {
        self.play_on(0, animation, play, Layer::default());
    }

    /// Plays a clip on `track` from its start, layered over the tracks
    /// below it, fading in over what the track played before.
    pub fn play_on(&mut self, track: usize, animation: usize, play: Play, layer: Layer) {
        self.mixer.set(
            track,
            Some(animation),
            play.looping,
            play.crossfade,
            layer.alpha,
            layer.additive,
        );
    }

    /// Fades track 0 back to the setup pose.
    pub fn stop(&mut self, crossfade: f64) {
        self.stop_on(0, crossfade);
    }

    /// Fades `track` out, leaving what the tracks below it pose.
    pub fn stop_on(&mut self, track: usize, crossfade: f64) {
        self.mixer.set(track, None, false, crossfade, 1.0, false);
    }

    /// Sets the alpha of the clip `track` plays from the next step on; a
    /// clip it fades from keeps its own. False, and nothing set, when the
    /// track plays nothing.
    pub fn set_alpha(&mut self, track: usize, alpha: f64) -> bool {
        self.mixer.set_alpha(track, alpha)
    }

    /// What `track` plays, if anything (a track fading out to nothing plays
    /// nothing).
    pub fn track(&self, rig: &Rig, track: usize) -> Option<TrackState> {
        let entry = self.mixer.current(track)?;
        Some(TrackState {
            animation: entry.animation?,
            looping: entry.looping,
            track_time: entry.track_time,
            animation_time: entry.animation_time(rig),
            alpha: entry.alpha,
            additive: entry.additive,
        })
    }

    /// Holds one of a bone's local values at `value` on every step, written
    /// over what the clips posed and before the world transforms and the
    /// constraints read it, as rig-c's `adjust` does; `None` lets the clips
    /// pose it again. A product's driving value, such as a `turn` bone that
    /// transform constraints read.
    pub fn drive(&mut self, bone: usize, channel: BoneChannel, value: Option<f64>) {
        self.driven.retain(|d| (d.0, d.1) != (bone, channel));
        if let Some(value) = value {
            self.driven.push((bone, channel, value));
        }
    }

    /// Sets the directions every physics constraint's own `wind` and
    /// `gravity` act along, in the rig's space (y up), from the next step
    /// on. Until set, wind blows toward +x and gravity pulls toward −y:
    /// `[1, 0]` and `[0, -1]`, rig-c's own. A gust is the product varying
    /// `wind` from step to step; a constraint whose `wind` is 0 does not
    /// feel it.
    pub fn set_physics_environment(&mut self, wind: [f64; 2], gravity: [f64; 2]) {
        // rig-c's environment is y down; `0 - y` keeps a 0 positive.
        self.physics.wind = [wind[0], 0.0 - wind[1]];
        self.physics.gravity = [gravity[0], 0.0 - gravity[1]];
    }

    /// The rig moved by `(x, y)` rig units (y up) since the last step: every
    /// physics constraint's next step sees its bone move that much, so what
    /// hangs from a figure walking across the world trails it (rig-c's
    /// `physicsTranslate`).
    pub fn translate_physics(&mut self, x: f64, y: f64) {
        self.physics.translate(x, y);
    }

    /// The clip track 0 plays, if any.
    pub fn playing(&self) -> Option<usize> {
        self.mixer.current(0).and_then(|e| e.animation)
    }

    /// Moves every track by `dt` seconds and poses the rig into `pose`. The
    /// first step of a clip is usually `dt = 0`, which poses its start.
    pub fn advance(&mut self, rig: &Rig, dt: f64, pose: &mut Pose) {
        let before = self.physics.time;
        self.physics.time += dt;
        self.mixer.update(dt);
        self.working.reset(rig);
        let animation_time = self.mixer.apply(rig, &mut self.working);
        let w = &mut self.working;
        for &(bone, channel, value) in &self.driven {
            let local = &mut w.locals[bone];
            *match channel {
                BoneChannel::Rotation => &mut local.rotation,
                BoneChannel::X => &mut local.x,
                BoneChannel::Y => &mut local.y,
                BoneChannel::ScaleX => &mut local.scale_x,
                BoneChannel::ScaleY => &mut local.scale_y,
                BoneChannel::ShearX => &mut local.shear_x,
                BoneChannel::ShearY => &mut local.shear_y,
            } = value;
        }

        // A crossed reset key resets each active constraint it names at the
        // clock before this step.
        for reset in &w.resets {
            for (i, c) in rig.constraints.iter().enumerate() {
                if let ConstraintKind::Physics(p) = &c.kind {
                    if reset.is_none_or(|r| r == i) && self.applied[i] && w.active[p.bone] {
                        self.physics.states[i].reset(before);
                    }
                }
            }
        }

        world_transforms(&w.locals, &self.parents, &w.active, &mut pose.bones);
        if !rig.constraints.is_empty() {
            constraints::apply(
                rig,
                &w.constraints,
                &self.applied,
                &mut w.locals,
                &mut pose.bones,
                &w.active,
                &mut self.scratch,
                &mut self.physics,
            );
        }
        self.physics.resetting = false;
        let w = &self.working;

        // Draw order and sequence frames follow track 0's playing clip.
        let playing = self.playing().map(|a| &rig.animations[a]);
        let keyed_order = playing.and_then(|a| {
            key_index_at(a.draw_order.iter().map(|k| k.time), animation_time)
                .map(|i| &a.draw_order[i].moves)
        });
        match keyed_order {
            Some(moves) => order_of(rig.slots.len(), moves, &mut self.order),
            None => {
                self.order.clear();
                self.order.extend(0..rig.slots.len());
            }
        }

        pose.pieces.clear();
        pose.vertices.clear();
        pose.uvs.clear();
        pose.triangles.clear();
        pose.animation_time = animation_time;
        // The clip over the walk: rig-c's draw walk (`poseClipped`). A clip
        // starts at its slot when its bone is active and no clip is, and ends
        // after its end slot is walked, whatever that slot shows.
        let mut active: Option<(Option<usize>, Plan)> = None;
        for &slot in &self.order {
            let data = &rig.slots[slot];
            let state = &w.slots[slot];
            let shown = w.resolve(rig, slot, state.placeholder);
            if let Some(clip) = shown.and_then(|a| rig.attachments[a].clip.as_ref()) {
                if active.as_ref().is_some_and(|(end, _)| *end == Some(slot)) {
                    active = None;
                }
                let geometry = shown.map(|a| &rig.attachments[a].geometry);
                if let (true, None, Some(Geometry::Mesh(mesh))) =
                    (w.active[data.bone], &active, geometry)
                {
                    self.world.clear();
                    mesh_vertices(
                        &mesh.vertices,
                        state.deform.as_deref(),
                        &pose.bones[data.bone],
                        &pose.bones,
                        &mut self.world,
                    );
                    let polygon: Vec<f64> = self.world.iter().flatten().copied().collect();
                    active = Some((clip.end, clip::plan(&polygon, clip.inverse, clip.convex)));
                }
                continue;
            }
            if let Some(attachment) = shown.filter(|_| w.active[data.bone]) {
                let shape = Shape {
                    playing,
                    animation_time,
                    clip: active.as_ref().map(|a| &a.1),
                };
                draw(
                    rig,
                    w,
                    slot,
                    attachment,
                    &shape,
                    (&mut self.world, &mut self.clipping),
                    pose,
                );
            }
            if active.as_ref().is_some_and(|(end, _)| *end == Some(slot)) {
                active = None;
            }
        }

        pose.events.clear();
        for &(key, animation) in &w.events {
            let Some(TimelineKind::Events(keys)) = rig.animations[animation]
                .timelines
                .iter()
                .rev()
                .map(|t| &t.kind)
                .find(|k| matches!(k, TimelineKind::Events(_)))
            else {
                continue;
            };
            let k = &keys[key];
            pose.events.push(FiredEvent {
                event: k.event,
                time: k.time,
                int: k.int,
                float: k.float,
                string: k.string.clone(),
                volume: k.volume,
                balance: k.balance,
            });
        }
    }
}

/// What a step draws a slot by: the playing clip and its time, and the clip
/// over the slot.
struct Shape<'a> {
    playing: Option<&'a AnimationData>,
    animation_time: f64,
    clip: Option<&'a Plan>,
}

/// One slot's piece, appended to `pose`, cut by the clip over it.
fn draw(
    rig: &Rig,
    w: &Working,
    slot: usize,
    attachment: usize,
    shape: &Shape<'_>,
    (world, scratch): (&mut Vec<[f64; 2]>, &mut clip::Scratch),
    pose: &mut Pose,
) {
    let data = &rig.slots[slot];
    let state = &w.slots[slot];
    let a = &rig.attachments[attachment];
    if !a.drawn() {
        return;
    }
    let mut frame = a.setup_frame;
    if let Some(animation) = shape.playing {
        let identity = a.deform_identity;
        let count = rig.attachments[identity].frames.max(1);
        for (target, keys) in &animation.sequences {
            if *target == identity {
                if let Some(f) = sequence_frame_at(keys, count, shape.animation_time) {
                    frame = f;
                }
            }
        }
    }
    let frame = frame.min(a.frames - 1);
    let bone = &pose.bones[data.bone];
    world.clear();
    match &a.geometry {
        Geometry::Region(corners) => {
            for c in corners[frame].as_chunks::<2>().0 {
                world.push([
                    bone.a * c[0] + bone.b * c[1] + bone.x,
                    bone.c * c[0] + bone.d * c[1] + bone.y,
                ]);
            }
        }
        Geometry::Mesh(_) | Geometry::Linked { .. } => {
            let source = match a.geometry {
                Geometry::Linked { source } => source,
                _ => attachment,
            };
            let Geometry::Mesh(mesh) = &rig.attachments[source].geometry else {
                return;
            };
            mesh_vertices(
                &mesh.vertices,
                state.deform.as_deref(),
                bone,
                &pose.bones,
                world,
            );
        }
        Geometry::Inert => return,
    }
    let drawable = a.drawables + frame;
    let start = pose.vertices.len();
    let (uvs, triangles) = (pose.uvs.len(), pose.triangles.len());
    let cut = shape.clip.is_some_and(|plan| {
        let d = &rig.drawables[drawable];
        clip::cut(
            plan,
            world,
            &d.uvs,
            &d.triangles,
            scratch,
            Cut {
                vertices: &mut pose.vertices,
                uvs: &mut pose.uvs,
                triangles: &mut pose.triangles,
            },
        )
    });
    if !cut {
        pose.vertices
            .extend(world.iter().map(|[x, y]| [*x as f32, *y as f32]));
    }
    let l = state.light;
    let c = a.color;
    pose.pieces.push(Piece {
        slot,
        attachment,
        drawable,
        page: rig.drawables[drawable].page,
        vertices: start..pose.vertices.len(),
        clipped: cut.then_some(Clipped {
            uvs: uvs..pose.uvs.len(),
            triangles: triangles..pose.triangles.len(),
        }),
        blend: data.blend,
        color: [
            (l[0] * c[0]) as f32,
            (l[1] * c[1]) as f32,
            (l[2] * c[2]) as f32,
            (l[3] * c[3]) as f32,
        ],
        dark: state.dark.map(|d| [d[0] as f32, d[1] as f32, d[2] as f32]),
    });
}

/// rig-c's `worldVertices` / `deformedWorldVertices`, as doubles.
fn mesh_vertices(
    vertices: &VertexData,
    deform: Option<&[f64]>,
    bone: &BoneWorld,
    world: &[BoneWorld],
    out: &mut Vec<[f64; 2]>,
) {
    match vertices {
        VertexData::Unweighted(xy) => {
            let xy = deform.unwrap_or(xy);
            for p in xy.as_chunks::<2>().0 {
                let (x, y) = (p[0], p[1]);
                out.push([
                    x * bone.a + y * bone.b + bone.x,
                    x * bone.c + y * bone.d + bone.y,
                ]);
            }
        }
        VertexData::Weighted { counts, bindings } => {
            let mut b = 0;
            for &count in counts {
                let (mut wx, mut wy) = (0.0, 0.0);
                for &(bone, bx, by, weight) in &bindings[b..b + count as usize] {
                    let t = &world[bone];
                    let (x, y) = match deform {
                        Some(d) => (bx + d[2 * b], by + d[2 * b + 1]),
                        None => (bx, by),
                    };
                    b += 1;
                    wx += (x * t.a + y * t.b + t.x) * weight;
                    wy += (x * t.c + y * t.d + t.y) * weight;
                }
                out.push([wx, wy]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::{GRAVITY, WIND};

    /// rig-c's mixer refuses clips that key constraint timelines, so an
    /// additive layer over them has no oracle. rig-c's `ADDITIVE_MODE` says
    /// what the runtime does: a transform's mixes and a physics
    /// constraint's wind and gravity add, an ik's values and the other
    /// physics values replace. Over the same clip on track 0, a clip added
    /// at alpha one half moves the adding values to one and a half times
    /// track 0's and leaves the replacing ones where track 0 put them.
    #[test]
    fn an_additive_layer_adds_transform_mixes_and_wind_and_replaces_the_rest() {
        for (fixture, clip) in [
            (
                include_str!("../tests/fixtures/constraints.model.json"),
                "reach",
            ),
            (
                include_str!("../tests/fixtures/physics.model.json"),
                "swing",
            ),
        ] {
            let rig = Rig::from_json(fixture).unwrap();
            let clip = rig.find_animation(clip).unwrap();
            let looping = Play {
                looping: true,
                crossfade: 0.0,
            };
            let values = |layered: bool| {
                let mut player = Player::new(&rig);
                player.play(clip, looping);
                if layered {
                    let layer = Layer {
                        alpha: 0.5,
                        additive: true,
                    };
                    player.play_on(1, clip, looping, layer);
                }
                let mut pose = Pose::default();
                player.advance(&rig, 0.0, &mut pose);
                for _ in 0..13 {
                    player.advance(&rig, 1.0 / 30.0, &mut pose);
                }
                player.working.constraints.clone()
            };
            let (base, layered) = (values(false), values(true));
            let mut checked = 0;
            for (i, c) in rig.constraints.iter().enumerate() {
                // The values of constraint `i` the clip keys.
                let keyed: Vec<usize> = rig.animations[clip]
                    .timelines
                    .iter()
                    .flat_map(|tl| match &tl.kind {
                        TimelineKind::Constraint { constraint, .. } if *constraint == i => {
                            (0..7).collect()
                        }
                        TimelineKind::Physics { targets, value, .. } if targets.contains(&i) => {
                            vec![*value]
                        }
                        _ => vec![],
                    })
                    .collect();
                for (k, (b, l)) in base[i].values.iter().zip(layered[i].values).enumerate() {
                    let adds = keyed.contains(&k)
                        && match c.kind {
                            ConstraintKind::Transform(_) => true,
                            ConstraintKind::Physics(_) => k == WIND || k == GRAVITY,
                            ConstraintKind::Ik(_) => false,
                        };
                    let want = if adds { b * 1.5 } else { *b };
                    assert!(
                        (l - want).abs() < 1e-12,
                        "{} value {k}: {l}, want {want}",
                        c.name
                    );
                    checked += usize::from(adds);
                }
                assert_eq!(base[i].flags, layered[i].flags, "{}", c.name);
            }
            assert!(checked > 0, "some value adds");
        }
    }
}
