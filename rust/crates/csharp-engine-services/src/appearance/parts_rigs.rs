//! Parts rigs the Engine poses (`animation-parts-rig`) and realizers draw:
//! the `PartsRigs` service.
//!
//! A rig loaded from product content (`RuntimeAppearanceBridge::load_parts_rig`)
//! is kept by its drawing's asset id, with its pages admitted as textures;
//! its `DefinePartsRig` goes out through the projector's catalog. Each
//! published object showing a rig appearance gets one player per view, kept
//! by the object's id so a recreated renderer node keeps playing; every
//! call that plays, stops or weighs a track does so in every view. As each
//! call settles, every player advances to the Engine's presentation time by
//! the object's speed, every view of an object by the same step, and each
//! view's pose is published as `SetPartsRigPose`, once per change: a pose
//! equal to the one last published sends nothing.
//!
//! A rig node's motion in the world moves its physics: what the node moved
//! since the last settle along its own x and y axes, in rig units, is every
//! view's `translate_physics` before its step (mirrored for a mirrored
//! view). A pixel-sized rig has no world unit, and takes none.

use std::collections::BTreeMap;
use std::sync::Arc;

use ::animation_parts_rig::{BlendMode, BoneChannel, Layer, Play, Player, Pose, Rig};
use render_model::{
    PartsRigAsset, PartsRigBlend, PartsRigCamera, PartsRigClipped, PartsRigDrawable,
    PartsRigInstanceDescriptor, PartsRigPage, PartsRigPiece, PartsRigPose, RenderDiff,
    RenderFrameDiff, RenderHandle, SpriteSizeMode,
};

use super::*;

/// The parts rigs the Engine poses, and their players.
#[derive(Clone, Default)]
pub(crate) struct PartsRigScene {
    rigs: BTreeMap<String, Arc<Rig>>,
    /// Rigs loaded through the service, by handle.
    loaded: BTreeMap<u64, LoadedRig>,
    next_rig: u64,
    /// By product object.
    instances: BTreeMap<u64, RigInstance>,
    /// Each instance's renderer node.
    objects: BTreeMap<RenderHandle, u64>,
    /// The latest update's events.
    events: Vec<NativePartsRigEvent>,
    /// Fired since the latest update; reported by the next one.
    pending_events: Vec<NativePartsRigEvent>,
}

#[derive(Clone)]
struct LoadedRig {
    asset: String,
    /// The catalog textures of its pages.
    textures: Vec<String>,
    /// The appearances showing it, with how many of their views do.
    appearances: BTreeMap<u64, u32>,
}

#[derive(Clone)]
struct RigInstance {
    /// None between a destroy and the create that replaces it.
    handle: Option<RenderHandle>,
    views: Vec<ViewPlayer>,
    /// The node's rig: its views, scale and how it faces the camera.
    rig: PartsRigInstanceDescriptor,
    /// The node's world matrix (columns) at the last settle.
    world: Option<[[f32; 4]; 4]>,
    speed: f64,
    /// The presentation time it was last posed at; None before its first
    /// pose.
    posed_at: Option<f64>,
    /// Played, stopped, driven or reskinned since it was last posed.
    dirty: bool,
    /// The node's world origin at the last settle.
    origin: Option<[f32; 3]>,
}

#[derive(Clone)]
struct ViewPlayer {
    asset: String,
    mirror: bool,
    /// None until its rig is defined.
    player: Option<Player>,
    pose: Pose,
    published: Option<PartsRigPose>,
}

impl ViewPlayer {
    fn new(asset: &str, mirror: bool, rigs: &BTreeMap<String, Arc<Rig>>) -> ViewPlayer {
        ViewPlayer {
            asset: asset.to_owned(),
            mirror,
            player: rigs.get(asset).map(|rig| Player::new(rig)),
            pose: Pose::default(),
            published: None,
        }
    }
}

fn not_published(object_id: u64) -> CsharpEngineServicesError {
    parts_rig_error(
        "CSHARP_PARTS_RIG_OBJECT",
        format!("object {object_id} is not a published parts rig"),
    )
}

impl PartsRigScene {
    /// Keep `rig` as asset `id`'s, drawing from `pages` (one per rig page,
    /// in its order); the drawing realizers draw its poses with. Views
    /// waiting for it start from the setup pose.
    pub(crate) fn define(
        &mut self,
        id: String,
        rig: Arc<Rig>,
        pages: Vec<String>,
    ) -> PartsRigAsset {
        let asset = PartsRigAsset {
            id: id.clone(),
            pages: rig
                .pages()
                .iter()
                .zip(pages)
                .map(|(page, texture)| PartsRigPage {
                    texture,
                    premultiplied: page.pma,
                    detail: None,
                })
                .collect(),
            drawables: rig
                .drawables()
                .iter()
                .map(|drawable| PartsRigDrawable {
                    page: drawable.page as u32,
                    uvs: drawable.uvs.clone(),
                    triangles: drawable.triangles.clone(),
                })
                .collect(),
        };
        for instance in self.instances.values_mut() {
            for view in &mut instance.views {
                if view.asset == id {
                    view.player = Some(Player::new(&rig));
                    instance.posed_at = None;
                    instance.dirty = true;
                }
            }
        }
        self.rigs.insert(id, rig);
        asset
    }

    /// Forget asset `id`'s rig; views showing it hold their last pose.
    pub(crate) fn release(&mut self, id: &str) {
        self.rigs.remove(id);
        for instance in self.instances.values_mut() {
            for view in &mut instance.views {
                if view.asset == id {
                    view.player = None;
                }
            }
        }
    }

    /// Take the rig nodes a pushed frame creates, keeping the players of an
    /// object whose node is created again over the same views.
    pub(crate) fn observe_frame(&mut self, frame: &RenderFrameDiff) {
        for op in &frame.ops {
            match op {
                RenderDiff::CreatePartsRig { handle, rig, .. } => {
                    let Some(object) = rig.metadata.source_entity else {
                        continue;
                    };
                    let same = self.instances.get(&object).is_some_and(|instance| {
                        instance.views.len() == rig.views.len()
                            && instance
                                .views
                                .iter()
                                .zip(&rig.views)
                                .all(|(player, view)| player.asset == view.asset)
                    });
                    if same {
                        let instance = self.instances.get_mut(&object).expect("checked");
                        if let Some(previous) = instance.handle.replace(*handle) {
                            self.objects.remove(&previous);
                        }
                        for (player, view) in instance.views.iter_mut().zip(&rig.views) {
                            player.mirror = view.mirror;
                            player.published = None;
                        }
                        instance.rig = rig.clone();
                        instance.dirty = true;
                    } else {
                        let instance = RigInstance {
                            handle: Some(*handle),
                            views: rig
                                .views
                                .iter()
                                .map(|view| ViewPlayer::new(&view.asset, view.mirror, &self.rigs))
                                .collect(),
                            rig: rig.clone(),
                            world: None,
                            speed: 1.0,
                            posed_at: None,
                            dirty: true,
                            origin: None,
                        };
                        if let Some(previous) = self.instances.insert(object, instance) {
                            if let Some(handle) = previous.handle {
                                self.objects.remove(&handle);
                            }
                        }
                    }
                    self.objects.insert(*handle, object);
                }
                RenderDiff::Destroy { handle } => {
                    if let Some(object) = self.objects.remove(handle) {
                        if let Some(instance) = self.instances.get_mut(&object) {
                            instance.handle = None;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn live(&self, object_id: u64) -> Result<&RigInstance, CsharpEngineServicesError> {
        self.instances
            .get(&object_id)
            .filter(|instance| instance.handle.is_some())
            .ok_or_else(|| not_published(object_id))
    }

    /// Each view's player and rig of object `object_id`, which is marked to
    /// pose again at the next settle, and its speed; refused while a view's
    /// rig is disposed.
    #[allow(
        clippy::type_complexity,
        reason = "the players and the speed they share"
    )]
    fn players(
        &mut self,
        object_id: u64,
    ) -> Result<(Vec<(&mut Player, &Rig)>, &mut f64), CsharpEngineServicesError> {
        let rigs = &self.rigs;
        let instance = self
            .instances
            .get_mut(&object_id)
            .filter(|instance| instance.handle.is_some())
            .ok_or_else(|| not_published(object_id))?;
        instance.dirty = true;
        let players = instance
            .views
            .iter_mut()
            .map(|view| match (view.player.as_mut(), rigs.get(&view.asset)) {
                (Some(player), Some(rig)) => Ok((player, &**rig)),
                _ => Err(parts_rig_error(
                    "CSHARP_PARTS_RIG_RELEASED",
                    format!("object {object_id} shows a parts rig that was disposed"),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((players, &mut instance.speed))
    }

    /// Play clip `clip` on `track` of every view of `object_id` from its
    /// start, layered by `layer` over the tracks below, fading in over what
    /// the track played before; a play on track 0 sets the object's
    /// `speed`.
    pub(crate) fn play(
        &mut self,
        object_id: u64,
        track: usize,
        clip: &str,
        play: Play,
        layer: Layer,
        speed: f64,
    ) -> Result<(), CsharpEngineServicesError> {
        let (players, instance_speed) = self.players(object_id)?;
        let clips = players
            .iter()
            .map(|(_, rig)| {
                rig.find_animation(clip).ok_or_else(|| {
                    parts_rig_error(
                        "CSHARP_PARTS_RIG_CLIP",
                        format!("a view of object {object_id} has no clip `{clip}`"),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        for ((player, _), clip) in players.into_iter().zip(clips) {
            player.play_on(track, clip, play, layer);
        }
        if track == 0 {
            *instance_speed = speed;
        }
        Ok(())
    }

    /// Fade `track` of every view of `object_id` out.
    pub(crate) fn stop(
        &mut self,
        object_id: u64,
        track: usize,
        crossfade: f64,
    ) -> Result<(), CsharpEngineServicesError> {
        for (player, _) in self.players(object_id)?.0 {
            player.stop_on(track, crossfade);
        }
        Ok(())
    }

    /// Set the alpha of the clip `track` of every view of `object_id`
    /// plays; refused when it plays nothing.
    pub(crate) fn set_track_alpha(
        &mut self,
        object_id: u64,
        track: usize,
        alpha: f64,
    ) -> Result<(), CsharpEngineServicesError> {
        // The views play the same tracks: each plays something on it or
        // none does, so a refusal comes before any is set.
        for (player, _) in self.players(object_id)?.0 {
            if !player.set_alpha(track, alpha) {
                return Err(parts_rig_error(
                    "CSHARP_PARTS_RIG_TRACK",
                    format!("track {track} of object {object_id} plays nothing"),
                ));
            }
        }
        Ok(())
    }

    /// Show skin `skin` (empty: the default skin alone) on every view of
    /// `object_id`.
    pub(crate) fn set_skin(
        &mut self,
        object_id: u64,
        skin: &str,
    ) -> Result<(), CsharpEngineServicesError> {
        let players = self.players(object_id)?.0;
        let skins = players
            .iter()
            .map(|(_, rig)| match skin {
                "" => Ok(None),
                name => rig.find_skin(name).map(Some).ok_or_else(|| {
                    parts_rig_error(
                        "CSHARP_PARTS_RIG_SKIN",
                        format!("a view of object {object_id} has no skin `{name}`"),
                    )
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        for ((player, rig), skin) in players.into_iter().zip(skins) {
            player.set_skin(rig, skin);
        }
        Ok(())
    }

    /// Hold (or with `None` release) a local value of bone `bone` in every
    /// view of `object_id` whose rig has it; refused when none has.
    pub(crate) fn drive(
        &mut self,
        object_id: u64,
        bone: &str,
        channel: BoneChannel,
        value: Option<f64>,
    ) -> Result<(), CsharpEngineServicesError> {
        let mut found = false;
        for (player, rig) in self.players(object_id)?.0 {
            if let Some(index) = rig.find_bone(bone) {
                player.drive(index, channel, value);
                found = true;
            }
        }
        if !found {
            return Err(parts_rig_error(
                "CSHARP_PARTS_RIG_BONE",
                format!("no view of object {object_id} has a bone `{bone}`"),
            ));
        }
        Ok(())
    }

    /// What `track` of `object_id` plays.
    pub(crate) fn read(
        &self,
        object_id: u64,
        track: usize,
    ) -> Result<NativePartsRigReadout, CsharpEngineServicesError> {
        let instance = self.live(object_id)?;
        let first = &instance.views[0];
        let playing = self.rigs.get(&first.asset).and_then(|rig| {
            first
                .player
                .as_ref()
                .and_then(|player| player.track(rig, track))
                .map(|state| (state, rig))
        });
        Ok(NativePartsRigReadout {
            views: instance.views.len() as u32,
            playing: playing.is_some(),
            clip: playing.map_or_else(Default::default, |(state, rig)| {
                feedback_text(rig.animation_name(state.animation))
            }),
            clip_seconds: playing.map_or(0.0, |(state, _)| state.animation_time),
            looping: playing.is_some_and(|(state, _)| state.looping),
            alpha: playing.map_or(0.0, |(state, _)| state.alpha as f32),
            additive: playing.is_some_and(|(state, _)| state.additive),
            speed: instance.speed as f32,
            posed: instance.posed_at.is_some(),
        })
    }

    /// Bone `bone` of `object_id`, as last posed, in the view `camera`'s
    /// reading draws and where that reading draws it.
    pub(crate) fn read_bone(
        &self,
        object_id: u64,
        bone: &str,
        camera: &PartsRigCamera,
    ) -> Result<NativePartsRigBoneReadout, CsharpEngineServicesError> {
        let instance = self.live(object_id)?;
        let world = instance.world.ok_or_else(|| {
            parts_rig_error(
                "CSHARP_PARTS_RIG_NOT_POSED",
                format!("object {object_id} has not been posed yet"),
            )
        })?;
        let rig = &instance.rig;
        if rig.size_mode == SpriteSizeMode::Pixel {
            return Err(parts_rig_error(
                "CSHARP_PARTS_RIG_PIXEL_SIZED",
                format!(
                    "object {object_id} is sized in pixels: where it draws follows each reading's viewport"
                ),
            ));
        }
        let view = rig.view_seen(&world, camera);
        let missing = || {
            parts_rig_error(
                "CSHARP_PARTS_RIG_BONE",
                format!("view {view} of object {object_id} has no posed bone `{bone}`"),
            )
        };
        let player = &instance.views[view];
        let index = self
            .rigs
            .get(&player.asset)
            .and_then(|rig| rig.find_bone(bone))
            .ok_or_else(missing)?;
        let bone = player.pose.bones.get(index).ok_or_else(missing)?;
        let (x, y) = (bone.x as f32, bone.y as f32);
        let placed = rig
            .place_point(&world, camera, [x, y])
            .expect("a world-sized rig places its points");
        let [wx, wy, wz] = placed.world;
        Ok(NativePartsRigBoneReadout {
            view: view as u32,
            position: NativeVec2 { x, y },
            rotation_degrees: bone.c.atan2(bone.a).to_degrees() as f32,
            scale: NativeVec2 {
                x: bone.a.hypot(bone.c) as f32,
                y: bone.b.hypot(bone.d) as f32,
            },
            world: NativeVec3 {
                x: wx,
                y: wy,
                z: wz,
            },
        })
    }

    /// An update begins: the events fired since the last are its.
    pub(crate) fn begin_update(&mut self) {
        self.events = std::mem::take(&mut self.pending_events);
    }

    fn has_events(&self) -> bool {
        !self.events.is_empty() || !self.pending_events.is_empty()
    }

    /// Advance every player to presentation time `now` and pose it once;
    /// the view poses that changed since they were last published, for
    /// realizers. Objects `world` no longer holds as parts rigs are dropped.
    pub(crate) fn settle(
        &mut self,
        now: f64,
        world: &render_presentation::PresentationWorld,
    ) -> Vec<RenderDiff> {
        let objects = &mut self.objects;
        self.instances.retain(|_, instance| {
            let live = instance
                .handle
                .is_some_and(|handle| world.parts_rig(handle).is_some());
            if !live {
                if let Some(handle) = instance.handle {
                    objects.remove(&handle);
                }
            }
            live
        });
        let mut ops = Vec::new();
        for (object, instance) in &mut self.instances {
            let handle = instance.handle.expect("live instances have a node");
            if let (Some(node), Some(m)) =
                (world.parts_rig(handle), world.scene_world_matrix(handle))
            {
                let origin = [m[3][0], m[3][1], m[3][2]];
                if let Some(from) = instance
                    .origin
                    .filter(|_| node.size_mode == SpriteSizeMode::World)
                {
                    let moved = [0, 1, 2].map(|k| f64::from(origin[k]) - f64::from(from[k]));
                    // Along an axis of world length `l` per node unit, one
                    // rig unit is `scale * l` long.
                    let along = |axis: [f32; 4]| {
                        let axis = [0, 1, 2].map(|k| f64::from(axis[k]));
                        let squared: f64 = axis.iter().map(|a| a * a).sum();
                        let dot: f64 = (0..3).map(|k| moved[k] * axis[k]).sum();
                        if squared > 0.0 {
                            dot / (f64::from(node.scale) * squared)
                        } else {
                            0.0
                        }
                    };
                    let (x, y) = (along(m[0]), along(m[1]));
                    if x != 0.0 || y != 0.0 {
                        for view in &mut instance.views {
                            let x = if view.mirror { -x } else { x };
                            if let Some(player) = &mut view.player {
                                player.translate_physics(x, y);
                            }
                        }
                    }
                }
                instance.origin = Some(origin);
                instance.world = Some(m);
            }
            let elapsed = instance.posed_at.map_or(0.0, |at| (now - at).max(0.0));
            if elapsed == 0.0 && !instance.dirty {
                continue;
            }
            let step = elapsed * instance.speed;
            instance.posed_at = Some(now);
            instance.dirty = false;
            for (index, view) in instance.views.iter_mut().enumerate() {
                let (Some(player), Some(rig)) = (&mut view.player, self.rigs.get(&view.asset))
                else {
                    continue;
                };
                player.advance(rig, step, &mut view.pose);
                if index == 0 {
                    self.pending_events
                        .extend(view.pose.events.iter().map(|event| NativePartsRigEvent {
                            object_id: *object,
                            name: feedback_text(rig.event_name(event.event)),
                            clip_seconds: event.time,
                            int_value: event.int,
                            float_value: event.float as f32,
                            volume: event.volume as f32,
                            balance: event.balance as f32,
                        }));
                }
                let pieces: Arc<[PartsRigPiece]> = view
                    .pose
                    .pieces
                    .iter()
                    .map(|piece| PartsRigPiece {
                        drawable: piece.drawable as u32,
                        clipped: piece.clipped.as_ref().map(|c| PartsRigClipped {
                            vertices: c.uvs.len() as u32,
                            triangles: c.triangles.len() as u32,
                        }),
                        blend: match piece.blend {
                            BlendMode::Normal => PartsRigBlend::Normal,
                            BlendMode::Additive => PartsRigBlend::Additive,
                            BlendMode::Multiply => PartsRigBlend::Multiply,
                            BlendMode::Screen => PartsRigBlend::Screen,
                        },
                        color: piece.color,
                        dark: piece.dark.unwrap_or_default(),
                    })
                    .collect();
                let posed = &view.pose;
                let unchanged = view.published.as_ref().is_some_and(|published| {
                    *published.pieces == *pieces
                        && *published.vertices == *posed.vertices
                        && *published.uvs == *posed.uvs
                        && *published.triangles == *posed.triangles
                });
                if unchanged {
                    continue;
                }
                let pose = PartsRigPose {
                    seconds: now,
                    pieces,
                    vertices: posed.vertices.as_slice().into(),
                    uvs: posed.uvs.as_slice().into(),
                    triangles: posed.triangles.as_slice().into(),
                };
                view.published = Some(pose.clone());
                ops.push(RenderDiff::SetPartsRigPose {
                    handle,
                    view: index as u32,
                    pose,
                });
            }
        }
        ops
    }
}

fn parts_rig_error(code: &'static str, message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new(code, message)
}

/// `value` as inline feedback text, cut at a character boundary to its 96
/// bytes.
fn feedback_text(value: &str) -> NativeAnimationFeedbackText {
    let mut length = value.len().min(96);
    while !value.is_char_boundary(length) {
        length -= 1;
    }
    let mut out = NativeAnimationFeedbackText {
        len: length as u32,
        ..Default::default()
    };
    out.bytes[..length].copy_from_slice(&value.as_bytes()[..length]);
    out
}

fn bone_channel(channel: NativePartsRigBoneChannel) -> BoneChannel {
    match channel {
        NativePartsRigBoneChannel::Rotation => BoneChannel::Rotation,
        NativePartsRigBoneChannel::X => BoneChannel::X,
        NativePartsRigBoneChannel::Y => BoneChannel::Y,
        NativePartsRigBoneChannel::ScaleX => BoneChannel::ScaleX,
        NativePartsRigBoneChannel::ScaleY => BoneChannel::ScaleY,
        NativePartsRigBoneChannel::ShearX => BoneChannel::ShearX,
        NativePartsRigBoneChannel::ShearY => BoneChannel::ShearY,
    }
}

/// A track's alpha: from 0 to 1, as rig-c's mixer defines a weight.
fn alpha(value: f32) -> Result<f64, CsharpEngineServicesError> {
    if (0.0..=1.0).contains(&value) {
        Ok(f64::from(value))
    } else {
        Err(parts_rig_error(
            "CSHARP_PARTS_RIG_VALUE",
            "a track's alpha must be from 0 to 1",
        ))
    }
}

/// A finite, non-negative number: seconds, or a speed.
fn not_negative(value: f32, what: &str) -> Result<f64, CsharpEngineServicesError> {
    if value.is_finite() && value >= 0.0 {
        Ok(f64::from(value))
    } else {
        Err(parts_rig_error(
            "CSHARP_PARTS_RIG_VALUE",
            format!("{what} must be finite and not negative"),
        ))
    }
}

/// Page `name` of the model document at `path`: a file beside it.
fn page_path(path: &str, name: &str) -> String {
    match path.rsplit_once('/') {
        Some((directory, _)) => format!("{directory}/{name}"),
        None => name.to_owned(),
    }
}

impl RuntimeAppearanceBridge {
    /// Read a rig's model document and its pages from product content.
    fn load_parts_rig(
        &mut self,
        request: NativePartsRigLoadRequest,
    ) -> Result<NativePartsRigInfo, CsharpEngineServicesError> {
        // SAFETY: the borrowed path is copied before the direct call returns.
        let path =
            unsafe { borrowed_utf8(request.path.bytes, request.path.len, "parts rig path")? }
                .to_owned();
        let content = self.content_path(&path)?;
        let text = std::str::from_utf8(&content.bytes).map_err(|_| {
            parts_rig_error(
                "CSHARP_PARTS_RIG_MODEL",
                format!("`{path}` is not UTF-8 text"),
            )
        })?;
        let rig = Arc::new(Rig::from_json(text).map_err(|error| {
            parts_rig_error(
                "CSHARP_PARTS_RIG_MODEL",
                format!("`{path}` is not a parts rig the Engine plays: {error}"),
            )
        })?);
        let mut pages = Vec::with_capacity(rig.pages().len());
        for page in rig.pages() {
            let content = self.content_path(&page_path(&path, &page.name))?;
            pages.push(self.imports.file(
                content,
                request.filter,
                NativeTextureWrap::Clamp,
                NativeTextureColorSpace::Srgb,
                &[],
            )?);
        }
        let staged = self.staged_mut()?;
        let state = &mut staged.state;
        state.parts_rigs.next_rig += 1;
        let handle = state.parts_rigs.next_rig;
        let asset = format!("parts-rig/native-{handle}");
        let mut held = BTreeSet::new();
        let mut textures = Vec::with_capacity(pages.len());
        for (index, resource) in pages.into_iter().enumerate() {
            let texture = sprite_texture_descriptor(
                &resource,
                format!("texture/parts-rig-{handle}-{index}"),
            )?;
            held.insert(state.render_resources.stage(resource)?);
            textures.push(texture);
        }
        let texture_ids: Vec<String> = textures.iter().map(|texture| texture.id.clone()).collect();
        let info = NativePartsRigInfo {
            rig: NativePartsRigHandle { value: handle },
            bones: rig.bone_count() as u32,
            slots: rig.slot_count() as u32,
            animations: rig.animation_count() as u32,
            skins: rig.skin_count() as u32,
            pages: textures.len() as u32,
        };
        let drawing = state
            .parts_rigs
            .define(asset.clone(), rig, texture_ids.clone());
        let resources = state.projector.resources_mut();
        resources.textures.extend(textures);
        resources.parts_rigs.push(Arc::new(drawing));
        state.parts_rigs.loaded.insert(
            handle,
            LoadedRig {
                asset,
                textures: texture_ids,
                appearances: BTreeMap::new(),
            },
        );
        state.set_held(Holder::PartsRig, handle, held);
        Ok(info)
    }

    fn destroy_parts_rig(
        &mut self,
        rig: NativePartsRigHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let state = &mut staged.state;
        let Some(loaded) = state.parts_rigs.loaded.get(&rig.value) else {
            return Err(parts_rig_error(
                "CSHARP_PARTS_RIG_HANDLE",
                "parts rig is not live",
            ));
        };
        if !loaded.appearances.is_empty() {
            return Err(parts_rig_error(
                "CSHARP_PARTS_RIG_IN_USE",
                "dispose the appearances showing this parts rig before disposing it",
            ));
        }
        let loaded = state.parts_rigs.loaded.remove(&rig.value).expect("checked");
        state.parts_rigs.release(&loaded.asset);
        let resources = state.projector.resources_mut();
        resources.parts_rigs.remove(&loaded.asset);
        for texture in &loaded.textures {
            resources.textures.remove(texture);
        }
        state.release_held(Holder::PartsRig, rig.value);
        release_unowned_internal_resources(state);
        staged.resource_releases_pending = true;
        Ok(())
    }

    unsafe fn create_parts_rig_appearance(
        &mut self,
        request: &NativePartsRigAppearanceRequest,
    ) -> Result<NativeAppearanceHandle, CsharpEngineServicesError> {
        let native_views =
            unsafe { borrowed_slice(request.views, request.views_len, "parts rig views") }?;
        let material = self.sprite_material_descriptor(request.material)?;
        let staged = self.staged_ref()?;
        let views = native_views
            .iter()
            .map(|view| {
                let loaded = staged
                    .state
                    .parts_rigs
                    .loaded
                    .get(&view.rig.value)
                    .ok_or_else(|| {
                        parts_rig_error("CSHARP_PARTS_RIG_HANDLE", "a view's parts rig is not live")
                    })?;
                Ok(render_model::PartsRigView {
                    asset: loaded.asset.clone(),
                    heading: view.heading,
                    mirror: view.mirror,
                })
            })
            .collect::<Result<Vec<_>, CsharpEngineServicesError>>()?;
        // The fields a rig shares with a sprite resolve as a sprite's do.
        let sprite = sprite_instance_descriptor(
            String::new(),
            0,
            NativeVec2 { x: 0.0, y: 0.0 },
            NativeVec2 { x: 1.0, y: 1.0 },
            request.billboard,
            request.size_mode,
            request.render_order,
            request.depth,
            request.tint,
            material,
        );
        let rig = render_model::PartsRigInstanceDescriptor {
            views,
            scale: request.scale,
            size_mode: sprite.size_mode,
            billboard: sprite.billboard,
            tint: sprite.tint,
            render_order: sprite.render_order,
            depth: sprite.depth,
            layer: sprite.layer,
            material: sprite.material,
            texel_snap: request.texel_snap,
            visible: true,
            transform: Transform::IDENTITY,
            metadata: RenderMetadata::default(),
        };
        rig.validate().map_err(|error| {
            parts_rig_error("CSHARP_PARTS_RIG_APPEARANCE", format!("{error:?}"))
        })?;
        let appearance = self.allocate_appearance(Appearance::PartsRig { rig })?;
        let loaded = &mut self.staged_mut()?.state.parts_rigs.loaded;
        for view in native_views {
            if let Some(rig) = loaded.get_mut(&view.rig.value) {
                *rig.appearances.entry(appearance.value).or_default() += 1;
            }
        }
        Ok(appearance)
    }

    fn play_parts_rig(
        &mut self,
        request: NativePartsRigPlayRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        // SAFETY: the borrowed name is copied before the direct call returns.
        let clip =
            unsafe { borrowed_utf8(request.clip.bytes, request.clip.len, "parts rig clip")? }
                .to_owned();
        let crossfade = not_negative(request.crossfade_seconds, "crossfade")?;
        let speed = not_negative(request.speed, "speed")?;
        let layer = Layer {
            alpha: alpha(request.alpha)?,
            additive: request.additive,
        };
        self.staged_mut()?.state.parts_rigs.play(
            request.object_id,
            request.track as usize,
            &clip,
            Play {
                looping: request.looping,
                crossfade,
            },
            layer,
            speed,
        )
    }

    fn stop_parts_rig(
        &mut self,
        request: NativePartsRigStopRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let crossfade = not_negative(request.crossfade_seconds, "crossfade")?;
        self.staged_mut()?.state.parts_rigs.stop(
            request.object_id,
            request.track as usize,
            crossfade,
        )
    }

    fn set_parts_rig_track_alpha(
        &mut self,
        request: NativePartsRigTrackAlphaRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let alpha = alpha(request.alpha)?;
        self.staged_mut()?.state.parts_rigs.set_track_alpha(
            request.object_id,
            request.track as usize,
            alpha,
        )
    }

    fn set_parts_rig_skin(
        &mut self,
        request: NativePartsRigSkinRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        // SAFETY: the borrowed name is copied before the direct call returns.
        let skin =
            unsafe { borrowed_utf8(request.skin.bytes, request.skin.len, "parts rig skin")? }
                .to_owned();
        self.staged_mut()?
            .state
            .parts_rigs
            .set_skin(request.object_id, &skin)
    }

    fn drive_parts_rig_bone(
        &mut self,
        request: NativePartsRigDriveRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        // SAFETY: the borrowed name is copied before the direct call returns.
        let bone =
            unsafe { borrowed_utf8(request.bone.bytes, request.bone.len, "parts rig bone")? }
                .to_owned();
        if request.held && !request.value.is_finite() {
            return Err(parts_rig_error(
                "CSHARP_PARTS_RIG_VALUE",
                "a driven value must be finite",
            ));
        }
        self.staged_mut()?.state.parts_rigs.drive(
            request.object_id,
            &bone,
            bone_channel(request.channel),
            request.held.then_some(f64::from(request.value)),
        )
    }

    /// A live camera as parts-rig placement reads it.
    fn parts_rig_camera(&self, camera: u64) -> Result<PartsRigCamera, CsharpEngineServicesError> {
        let camera = self
            .camera_view
            .and_then(|views| unsafe { &*views }.renderer_camera(camera))
            .ok_or_else(|| parts_rig_error("CSHARP_PARTS_RIG_CAMERA", "camera is not live"))?;
        let [right, up, forward] = camera.axes().map(|axis| axis.map(|c| c as f32));
        Ok(PartsRigCamera {
            eye: camera.pose.position.map(|c| c as f32),
            right,
            up,
            forward,
            orthographic: matches!(
                camera.projection,
                render_host_contracts::RendererCameraProjection::Orthographic { .. }
            ),
        })
    }

    fn read_parts_rig_events(
        &mut self,
    ) -> Result<NativePartsRigEventResult, CsharpEngineServicesError> {
        let events: Box<[NativePartsRigEvent]> =
            self.staged_ref()?.state.parts_rigs.events.clone().into();
        let result = NativePartsRigEventResult {
            events: events.as_ptr(),
            events_len: events.len(),
        };
        self.borrowed.hold(events);
        Ok(result)
    }
}

/// An update begins: the parts rigs' events fired since the last are its.
pub(crate) fn begin_update(state: &mut RuntimeAppearanceState) {
    // Read first: a call with no events does not write the shared state.
    if state.parts_rigs.has_events() {
        state.parts_rigs.begin_update();
    }
}

impl RuntimeAppearanceData {
    /// A disposed appearance no longer shows the rigs it showed.
    pub(super) fn forget_parts_rig_appearance(&mut self, appearance: u64) {
        for rig in self.parts_rigs.loaded.values_mut() {
            rig.appearances.remove(&appearance);
        }
    }
}

fn respond<T>(
    context: *mut c_void,
    result: *mut T,
    call: impl FnOnce(&mut RuntimeAppearanceBridge) -> Result<T, CsharpEngineServicesError>,
) -> i32 {
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAppearanceBridge>() };
    match call(bridge) {
        Ok(value) => {
            unsafe { *result = value };
            ABI_OK
        }
        Err(error) => {
            bridge.record_operation_error(error);
            0
        }
    }
}

unsafe extern "C" fn load(
    context: *mut c_void,
    request: *const NativePartsRigLoadRequest,
    result: *mut NativePartsRigInfo,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        // SAFETY: the request is read before the direct call returns.
        let request = unsafe { *request };
        respond(context, result, |bridge| bridge.load_parts_rig(request))
    })
}

unsafe extern "C" fn destroy(
    context: *mut c_void,
    rig: NativePartsRigHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.destroy_parts_rig(rig))
    })
}

unsafe extern "C" fn destroy_appearance(
    context: *mut c_void,
    appearance: NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.destroy_appearance(appearance))
    })
}

unsafe extern "C" fn create_appearance(
    context: *mut c_void,
    request: *const NativePartsRigAppearanceRequest,
    result: *mut NativeAppearanceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        respond(context, result, |bridge| unsafe {
            bridge.create_parts_rig_appearance(&*request)
        })
    })
}

unsafe extern "C" fn play(
    context: *mut c_void,
    request: *const NativePartsRigPlayRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        // SAFETY: the request is read before the direct call returns.
        let request = unsafe { *request };
        appearance_void(context, |bridge| bridge.play_parts_rig(request))
    })
}

unsafe extern "C" fn stop(
    context: *mut c_void,
    request: NativePartsRigStopRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.stop_parts_rig(request))
    })
}

unsafe extern "C" fn set_track_alpha(
    context: *mut c_void,
    request: NativePartsRigTrackAlphaRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        appearance_void(context, |bridge| bridge.set_parts_rig_track_alpha(request))
    })
}

unsafe extern "C" fn set_skin(
    context: *mut c_void,
    request: *const NativePartsRigSkinRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        // SAFETY: the request is read before the direct call returns.
        let request = unsafe { *request };
        appearance_void(context, |bridge| bridge.set_parts_rig_skin(request))
    })
}

unsafe extern "C" fn drive_bone(
    context: *mut c_void,
    request: *const NativePartsRigDriveRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        // SAFETY: the request is read before the direct call returns.
        let request = unsafe { *request };
        appearance_void(context, |bridge| bridge.drive_parts_rig_bone(request))
    })
}

unsafe extern "C" fn read(
    context: *mut c_void,
    request: NativePartsRigTrackRequest,
    readout: *mut NativePartsRigReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        respond(context, readout, |bridge| {
            bridge
                .staged_ref()?
                .state
                .parts_rigs
                .read(request.object_id, request.track as usize)
        })
    })
}

unsafe extern "C" fn read_events(
    context: *mut c_void,
    result: *mut NativePartsRigEventResult,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        respond(context, result, |bridge| bridge.read_parts_rig_events())
    })
}

unsafe extern "C" fn read_bone(
    context: *mut c_void,
    request: *const NativePartsRigBoneRequest,
    readout: *mut NativePartsRigBoneReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    appearance_operation(context, operation_error, || {
        if request.is_null() {
            return 0;
        }
        // SAFETY: the request is read before the direct call returns.
        let request = unsafe { *request };
        respond(context, readout, |bridge| {
            // SAFETY: the borrowed name is read before the direct call returns.
            let bone =
                unsafe { borrowed_utf8(request.bone.bytes, request.bone.len, "parts rig bone")? };
            let camera = bridge.parts_rig_camera(request.camera.value)?;
            bridge
                .staged_ref()?
                .state
                .parts_rigs
                .read_bone(request.object_id, bone, &camera)
        })
    })
}

pub(crate) fn api(bridge: &mut RuntimeAppearanceBridge) -> NativePartsRigsApi {
    NativePartsRigsApi {
        context: (bridge as *mut RuntimeAppearanceBridge).cast(),
        load,
        destroy,
        create_appearance,
        destroy_appearance,
        play,
        stop,
        set_track_alpha,
        set_skin,
        drive_bone,
        read,
        read_events,
        read_bone,
    }
}

#[cfg(test)]
mod tests;
