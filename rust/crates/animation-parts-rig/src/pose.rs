//! Playing a rig, and the pose one step produces.

use std::ops::Range;

use crate::mixer::{Mixer, Working};
use crate::rig::{BlendMode, Geometry, Rig, TimelineKind, VertexData};
use crate::timeline::{key_index_at, order_of, sequence_frame_at};
use crate::world::{world_transforms, BoneWorld};

/// One drawn slot, in draw order.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub slot: usize,
    /// The attachment the slot shows.
    pub attachment: usize,
    /// What it draws with: [`Rig::drawable`] gives its page, UVs and
    /// triangles, which never change.
    pub drawable: usize,
    /// The page it draws from.
    pub page: usize,
    /// Its posed vertices in [`Pose::vertices`], one per UV.
    pub vertices: Range<usize>,
    pub blend: BlendMode,
    /// The slot's colour times the attachment's, RGBA from 0 to 1.
    pub color: [f32; 4],
    /// The slot's two-colour tint dark colour, when it has one.
    pub dark: Option<[f32; 3]>,
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
    pub events: Vec<FiredEvent>,
    /// The playing clip's time, in seconds.
    pub animation_time: f64,
}

/// How a clip starts playing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Play {
    pub looping: bool,
    /// Seconds over which the clip fades in over whatever played before.
    pub crossfade: f64,
}

/// One instance of a rig playing one clip at a time.
///
/// Each [`Player::advance`] moves the track by a step and poses the rig
/// once from its setup pose.
pub struct Player {
    mixer: Mixer,
    working: Working,
    skin: Option<usize>,
    order: Vec<usize>,
    parents: Vec<Option<usize>>,
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
        };
        player.set_skin(rig, None);
        player
    }

    /// Shows a named skin, falling back to the default skin for slots it
    /// does not fill; `None` shows the default skin alone. A skin-required
    /// bone is active only under a skin that names it or a bone below it.
    pub fn set_skin(&mut self, rig: &Rig, skin: Option<usize>) {
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

    /// Plays a clip from its start, fading in over what played before.
    pub fn play(&mut self, animation: usize, play: Play) {
        self.mixer
            .set(Some(animation), play.looping, play.crossfade);
    }

    /// Fades back to the setup pose.
    pub fn stop(&mut self, crossfade: f64) {
        self.mixer.set(None, false, crossfade);
    }

    /// The playing clip, if any.
    pub fn playing(&self) -> Option<usize> {
        self.mixer.chain.first().and_then(|e| e.animation)
    }

    /// Moves the track by `dt` seconds and poses the rig into `pose`. The
    /// first step of a clip is usually `dt = 0`, which poses its start.
    pub fn advance(&mut self, rig: &Rig, dt: f64, pose: &mut Pose) {
        self.mixer.update(dt);
        self.working.reset(rig);
        let animation_time = self.mixer.apply(rig, &mut self.working);
        let w = &self.working;

        world_transforms(&w.locals, &self.parents, &w.active, &mut pose.bones);

        // Draw order and sequence frames follow the playing clip.
        let playing = self
            .mixer
            .chain
            .first()
            .and_then(|e| e.animation)
            .map(|a| &rig.animations[a]);
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
        pose.animation_time = animation_time;
        for &slot in &self.order {
            let data = &rig.slots[slot];
            if !w.active[data.bone] {
                continue;
            }
            let state = &w.slots[slot];
            let Some(attachment) = w.resolve(rig, slot, state.placeholder) else {
                continue;
            };
            let a = &rig.attachments[attachment];
            if !a.drawn() {
                continue;
            }
            let mut frame = a.setup_frame;
            if let Some(animation) = playing {
                let identity = a.deform_identity;
                let count = rig.attachments[identity].frames.max(1);
                for (target, keys) in &animation.sequences {
                    if *target == identity {
                        if let Some(f) = sequence_frame_at(keys, count, animation_time) {
                            frame = f;
                        }
                    }
                }
            }
            let frame = frame.min(a.frames - 1);
            let bone = &pose.bones[data.bone];
            let start = pose.vertices.len();
            match &a.geometry {
                Geometry::Region(corners) => {
                    for c in corners[frame].as_chunks::<2>().0 {
                        let (x, y) = (
                            bone.a * c[0] + bone.b * c[1] + bone.x,
                            bone.c * c[0] + bone.d * c[1] + bone.y,
                        );
                        pose.vertices.push([x as f32, y as f32]);
                    }
                }
                Geometry::Mesh(_) | Geometry::Linked { .. } => {
                    let source = match a.geometry {
                        Geometry::Linked { source } => source,
                        _ => attachment,
                    };
                    let Geometry::Mesh(mesh) = &rig.attachments[source].geometry else {
                        continue;
                    };
                    mesh_vertices(
                        &mesh.vertices,
                        state.deform.as_deref(),
                        bone,
                        &pose.bones,
                        &mut pose.vertices,
                    );
                }
                Geometry::Inert => continue,
            }
            let drawable = a.drawables + frame;
            let l = state.light;
            let c = a.color;
            pose.pieces.push(Piece {
                slot,
                attachment,
                drawable,
                page: rig.drawables[drawable].page,
                vertices: start..pose.vertices.len(),
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

/// rig-c's `worldVertices` / `deformedWorldVertices`.
fn mesh_vertices(
    vertices: &VertexData,
    deform: Option<&[f64]>,
    bone: &BoneWorld,
    world: &[BoneWorld],
    out: &mut Vec<[f32; 2]>,
) {
    match vertices {
        VertexData::Unweighted(xy) => {
            let xy = deform.unwrap_or(xy);
            for p in xy.as_chunks::<2>().0 {
                let (x, y) = (p[0], p[1]);
                out.push([
                    (x * bone.a + y * bone.b + bone.x) as f32,
                    (x * bone.c + y * bone.d + bone.y) as f32,
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
                out.push([wx as f32, wy as f32]);
            }
        }
    }
}
