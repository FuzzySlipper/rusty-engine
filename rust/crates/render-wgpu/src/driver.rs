//! The committed scene in one renderer, as the runtime's product calls leave
//! it, for whichever output draws it: the stream's render thread offscreen,
//! or the desktop shell to its window.
//!
//! The runtime hands over each product call's renderer changes as it commits
//! them, complete: the resource bytes they read and the entity positions
//! they follow come with them, and so does the Engine state the call reached,
//! so no frame shows a call's changes under the previous step. Whoever takes
//! the scene next applies what was handed over, in order and whole, before
//! anything else: an output drawing through [`SceneDriver::draw`], a tool
//! capturing through [`SceneDriver::capture`], or any other reading. The
//! runtime never takes the scene itself, so an update never waits on a frame.
//!
//! What the runtime reads between calls (the renderer's readouts, and the
//! animation and video facts the frames produce) is kept beside the scene,
//! as whoever last held the scene left it.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use render_host_contracts::{RendererCameraPose, RendererViewComposition, RendererViewportAnchors};
use render_model::RenderFrameDiff;
use render_presentation::PresentationFrameDiff;

use crate::{
    AnimationFact, ApplyIssue, DrawnCamera, EntityPositions, GhostPlateReadout, Gpu, GpuReadout,
    OffscreenTarget, Renderer, RendererOptions, ResourceSource, VideoFact,
};

/// Size a capture draws at when neither the caller nor a window states one.
const DEFAULT_CAPTURE_SIZE: (u32, u32) = (1920, 1080);

/// One renderer change a committed product call made, in call order.
#[derive(Debug, Clone)]
pub enum SceneChange {
    Frame(RenderFrameDiff),
    Presentation(PresentationFrameDiff),
    ViewComposition(RendererViewComposition),
}

/// A committed product call's renderer changes, complete when handed over:
/// the scene applies them later, on whichever thread takes it next, and
/// reads nothing back from the runtime.
pub struct CommittedCall {
    /// The changes, in call order.
    pub changes: Vec<SceneChange>,
    /// Every resource the changes may read, held as the call left them.
    pub resources: Box<dyn ResourceSource + Send>,
    /// Where each entity was when the call ended, for the effects and
    /// labels that follow one.
    pub entities: HashMap<u64, [f32; 3]>,
    /// What the call brought the Engine to.
    pub state: SceneState,
}

/// What the runtime handed over for the scene to take up, in order.
enum Edit {
    /// A call's changes, and the presentation clock when it was handed over.
    Call(CommittedCall, f64),
    /// A complete baseline for a new renderer.
    Rebaseline(CommittedCall, f64),
    Simulation(bool, u64),
    CollisionLines(Vec<crate::OverlayLine>),
}

/// The Engine state a call's changes bring the scene to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneState {
    /// Engine presentation time the call reached.
    pub elapsed_seconds: f64,
    /// Retained world revision the changes reach.
    pub world_revision: u64,
    /// Simulation step the scene shows.
    pub step: u64,
    /// Paused, or inspection time held.
    pub held: bool,
    /// How presentation time runs on past `elapsed_seconds` until the next
    /// call, while the world runs; `None` holds it there.
    pub lead: Option<PresentationLead>,
    /// The batch the call's deferred textures and meshes belong to
    /// ([`Renderer::set_preparation_batch`]).
    pub preparation_batch: u64,
}

/// How a running world's presentation time moves between committed calls:
/// frames draw at the committed time plus what host time has owed the world
/// since, never a step or more past it, so what moves with presentation time
/// (clouds, wind sway, product shaders' `frame.time`) moves every frame
/// rather than once per step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresentationLead {
    /// World seconds realtime admission already owed when the call observed
    /// host time: below one step.
    pub owed_seconds: f64,
    /// World seconds per host second (the gameplay rate).
    pub rate: f64,
    /// One fixed step, in seconds.
    pub step_seconds: f64,
    /// The host observation `owed_seconds` was measured at (host monotonic
    /// nanoseconds). A later call that brings the same one, such as a debug
    /// command between observations, leaves the frames' clock running from
    /// where that observation's call anchored it.
    pub observed_ns: u64,
}

/// What one drawn frame showed.
#[derive(Debug, Clone)]
pub struct SceneFrame {
    pub step: u64,
    pub held: bool,
    /// The presentation time it drew at (see [`PresentationLead`]).
    pub presentation_seconds: f64,
    /// Changes with each renderer the driver builds.
    pub renderer_id: u64,
    /// The retained world revision the frame shows.
    pub world_revision: u64,
    pub composition: Option<Arc<RendererViewComposition>>,
    pub composition_revision: u64,
    pub observer: Option<RendererCameraPose>,
    /// How each of `composition`'s cameras drew: its sampled pose, or the
    /// observer's where the observer replaced it in primary views.
    pub cameras: Vec<DrawnCamera>,
    /// The inspection views it drew, with their cameras then.
    pub inspection_views: Vec<crate::InspectionView>,
}

/// The camera of the composition's first primary view, as it is now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrimaryCamera {
    /// Position, yaw and pitch, as the Engine describes cameras.
    pub pose: RendererCameraPose,
    /// Its whole orientation (`[x, y, z, w]`), roll included: what a
    /// viewmodel's parts are placed by.
    pub orientation: [f64; 4],
    pub projection: render_host_contracts::RendererCameraProjection,
}

/// The inspection state of the committed scene, as the scene was last left.
#[derive(Debug, Clone)]
pub struct SceneView {
    pub held: bool,
    pub observer: Option<RendererCameraPose>,
    /// The view composition the scene holds.
    pub composition: Option<Arc<RendererViewComposition>>,
    /// A change was applied or handed over since the last frame was drawn.
    pub changed: bool,
}

/// One frame drawn for a tool ([`SceneDriver::capture`]): its RGBA pixels
/// and what a streamed frame's header says about it.
#[derive(Debug, Clone)]
pub struct Capture {
    /// Counts captures, apart from any output's frames.
    pub sequence: u64,
    pub step: u64,
    pub held: bool,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    /// A playing video clip covered the frame.
    pub video: bool,
    pub composition: Option<Arc<RendererViewComposition>>,
    /// How each of `composition`'s cameras drew.
    pub cameras: Vec<DrawnCamera>,
    /// The overlay sets drawn, with their revisions, when the capture asked
    /// for overlays; `None` when it left them out.
    pub overlays: Option<BTreeMap<String, u64>>,
    /// The inspection view drawn, for a capture of one.
    pub view: Option<crate::InspectionView>,
    /// The Engine presentation time the frame shows.
    pub presentation_seconds: f64,
}

/// The committed scene in a renderer on one device.
pub struct SceneDriver {
    gpu: Gpu,
    options: RendererOptions,
    epoch: Instant,
    scene: Mutex<Scene>,
    /// What is read and set without the scene; `wake` waits on it.
    beside: Mutex<Beside>,
    wake: Condvar,
}

/// What the runtime hands over and reads without holding the scene. Every
/// holder of the scene takes up the edits handed over here when it takes
/// the scene, and leaves the renderer's readings here when it lets it go.
/// The scene is always taken first.
struct Beside {
    /// Something changed since the last frame was drawn.
    changed: bool,
    /// Handed over and not yet taken up.
    edits: Vec<Edit>,
    /// Whether the simulation is held, and the step the scene shows, as
    /// last handed over.
    simulation: (bool, u64),
    readings: Readings,
}

/// The renderer's readouts as the scene was last left, and the facts its
/// frames produced since the runtime last took them.
struct Readings {
    settings: crate::RendererSettingsReadout,
    preparation: crate::PreparationReadout,
    gpu: GpuReadout,
    scatter: crate::ScatterUploadReadout,
    skipped_ops: BTreeMap<&'static str, u64>,
    last_skip: Option<String>,
    landmarks: (u64, Vec<crate::LandmarkSetReadout>),
    ghost_plates: Vec<GhostPlateReadout>,
    shadows: crate::ShadowReport,
    collision: Option<crate::CollisionShown>,
    held: bool,
    observer: Option<RendererCameraPose>,
    composition: Option<Arc<RendererViewComposition>>,
    primary_camera: Option<PrimaryCamera>,
    animation_facts: Vec<AnimationFact>,
    video_facts: Vec<VideoFact>,
}

struct Scene {
    renderer: Renderer,
    /// Something changed since the scene was last left.
    changed: bool,
    held: bool,
    step: u64,
    /// Engine presentation time the last applied call reached.
    elapsed_seconds: f64,
    /// Engine presentation time the renderer last advanced to, drawn or not.
    realized_seconds: f64,
    /// How presentation time runs on past `elapsed_seconds`, and the clock
    /// reading the last call applied at.
    lead: Option<(PresentationLead, f64)>,
    /// The presentation time the last frame drew at: frames between calls
    /// never draw behind it.
    drawn_seconds: f64,
    /// Counts captures; a capture's sequence.
    captures: u64,
    animation_facts: Vec<AnimationFact>,
    video_facts: Vec<VideoFact>,
    skipped_ops: BTreeMap<&'static str, u64>,
    last_skip: Option<String>,
    /// Counts renderers built; a rebaseline replaces the renderer.
    renderer_id: u64,
    /// The retained world revision the last applied changes reached.
    world_revision: u64,
    composition: Option<Arc<RendererViewComposition>>,
    /// Counts installed compositions on the current renderer.
    composition_revision: u64,
    observer: Option<RendererCameraPose>,
    viewport_anchors: RendererViewportAnchors,
    /// The player's choices of renderer settings, kept across rebaselines.
    player_settings: render_model::RendererSettingsOverrides,
    /// What inspection shows, kept across rebaselines.
    overlay_shown: crate::OverlayShown,
    /// The inspection views, kept across rebaselines.
    inspection_views: Vec<crate::InspectionView>,
    /// The inspection view pointer input goes to.
    selected_view: Option<String>,
}

impl SceneDriver {
    /// A renderer for the committed scene on `gpu`.
    pub fn new(gpu: Gpu, options: RendererOptions) -> Arc<Self> {
        let renderer = Renderer::new(&gpu, options);
        let readings = Readings {
            settings: renderer.settings_readout(),
            preparation: renderer.preparation_readout(),
            gpu: renderer.gpu_readout(),
            scatter: renderer.scatter_upload_readout(),
            skipped_ops: BTreeMap::new(),
            last_skip: None,
            landmarks: (0, renderer.landmark_sets()),
            ghost_plates: renderer.ghost_plate_readouts(),
            shadows: renderer.shadow_report(),
            collision: None,
            held: true,
            observer: None,
            composition: None,
            primary_camera: None,
            animation_facts: Vec::new(),
            video_facts: Vec::new(),
        };
        Arc::new(Self {
            beside: Mutex::new(Beside {
                changed: true,
                edits: Vec::new(),
                simulation: (true, 0),
                readings,
            }),
            scene: Mutex::new(Scene {
                renderer,
                changed: true,
                held: true,
                step: 0,
                elapsed_seconds: 0.0,
                realized_seconds: 0.0,
                lead: None,
                drawn_seconds: 0.0,
                captures: 0,
                animation_facts: Vec::new(),
                video_facts: Vec::new(),
                skipped_ops: BTreeMap::new(),
                last_skip: None,
                renderer_id: 1,
                world_revision: 0,
                composition: None,
                composition_revision: 0,
                observer: None,
                overlay_shown: crate::OverlayShown::default(),
                inspection_views: Vec::new(),
                selected_view: None,
                viewport_anchors: RendererViewportAnchors::new(),
                player_settings: render_model::RendererSettingsOverrides::default(),
            }),
            gpu,
            options,
            epoch: Instant::now(),
            wake: Condvar::new(),
        })
    }

    /// Hands over one committed call's renderer changes. The scene applies
    /// them in order, and moves effects and animation to the Engine
    /// presentation time the call reached, before it is next drawn or read;
    /// `state` lands with the changes, so no frame shows them under the
    /// previous step. Never waits on a frame.
    pub fn apply(&self, call: CommittedCall) {
        let now = self.now();
        self.hand_over(Edit::Call(call, now));
    }

    /// Hands over a complete baseline that replaces the renderer, after a
    /// product call's renderer work was lost or the world was replaced. No
    /// draw sees the empty renderer in between.
    pub fn rebaseline(&self, baseline: CommittedCall) {
        let now = self.now();
        self.hand_over(Edit::Rebaseline(baseline, now));
    }

    fn hand_over(&self, edit: Edit) {
        let mut beside = self.beside();
        // The latest handed over, which the next set_simulation compares with.
        match &edit {
            Edit::Call(call, _) | Edit::Rebaseline(call, _) => {
                beside.simulation = (call.state.held, call.state.step);
            }
            Edit::Simulation(held, step) => beside.simulation = (*held, *step),
            Edit::CollisionLines(_) => {}
        }
        beside.edits.push(edit);
        beside.changed = true;
        drop(beside);
        self.wake.notify_all();
    }

    /// Takes up one edit the runtime handed over.
    fn take_up(&self, scene: &mut Scene, edit: Edit) {
        match edit {
            Edit::Call(call, now) => {
                let entities = &|entity| call.entities.get(&entity).copied();
                scene.apply(
                    &call.changes,
                    call.resources.as_ref(),
                    entities,
                    call.state,
                    now,
                );
            }
            Edit::Rebaseline(baseline, now) => {
                scene.renderer = Renderer::new(&self.gpu, self.options);
                let observer = scene.observer;
                scene.renderer.set_observer(observer);
                let anchors = scene.viewport_anchors.clone();
                scene.renderer.set_viewport_anchors(anchors);
                let player = scene.player_settings;
                scene.renderer.set_player_settings(player);
                scene.renderer_id += 1;
                scene.composition = None;
                scene.composition_revision = 0;
                let state = baseline.state;
                scene.elapsed_seconds = state.elapsed_seconds;
                scene.drawn_seconds = state.elapsed_seconds;
                scene.renderer.set_animation_time(state.elapsed_seconds);
                scene.animation_facts.clear();
                scene.video_facts.clear();
                {
                    let mut beside = self.beside();
                    beside.readings.animation_facts.clear();
                    beside.readings.video_facts.clear();
                }
                let entities = &|entity| baseline.entities.get(&entity).copied();
                scene.apply(
                    &baseline.changes,
                    baseline.resources.as_ref(),
                    entities,
                    state,
                    now,
                );
                let shown = scene.overlay_shown.clone();
                scene.renderer.set_overlay_shown(shown);
                let views = scene.inspection_views.clone();
                scene.renderer.set_inspection_views(views);
                scene.changed = true;
            }
            Edit::Simulation(held, step) => {
                if (scene.held, scene.step) != (held, step) {
                    (scene.held, scene.step) = (held, step);
                    if held {
                        scene.lead = None;
                    }
                    scene.changed = true;
                }
            }
            Edit::CollisionLines(lines) => {
                scene.renderer.set_collision_lines(&lines);
                scene.changed |= scene.overlay_shown.collision.is_some();
            }
        }
    }

    /// Whether the simulation is held, and the step the scene shows, for a
    /// lifecycle change no product call published (pause, time mode).
    /// Handed over like a call's changes; never waits on a frame.
    pub fn set_simulation(&self, held: bool, step: u64) {
        if self.beside().simulation != (held, step) {
            self.hand_over(Edit::Simulation(held, step));
        }
    }

    /// Draws the primary views from `pose` instead of the product's cameras,
    /// or from the product's cameras again with `None`.
    pub fn set_observer(&self, pose: Option<RendererCameraPose>) {
        let mut scene = self.scene();
        scene.observer = pose;
        scene.renderer.set_observer(pose);
        scene.changed = true;
    }

    /// What inspection shows: overlay sets, the Debug layer and the
    /// renderer's own inspection drawing. The next frame follows.
    pub fn set_overlay_shown(&self, shown: crate::OverlayShown) {
        let mut scene = self.scene();
        if scene.overlay_shown == shown {
            return;
        }
        scene.overlay_shown = shown.clone();
        scene.renderer.set_overlay_shown(shown);
        scene.changed = true;
    }

    /// Replaces the collision wires inspection draws while
    /// [`crate::OverlayShown::collision`] shows them; the next frame follows.
    /// Handed over like a call's changes; never waits on a frame.
    pub fn set_collision_lines(&self, lines: &[crate::OverlayLine]) {
        self.hand_over(Edit::CollisionLines(lines.to_vec()));
    }

    /// Replaces the inspection views; the next frame draws them.
    pub fn set_inspection_views(&self, views: Vec<crate::InspectionView>) {
        let mut scene = self.scene();
        if scene.inspection_views == views {
            return;
        }
        scene.inspection_views = views.clone();
        scene.renderer.set_inspection_views(views);
        scene.changed = true;
    }

    pub fn inspection_views(&self) -> Vec<crate::InspectionView> {
        self.scene().inspection_views.clone()
    }

    /// The inspection view that takes pointer input; a host keeps it.
    pub fn select_inspection_view(&self, id: Option<String>) {
        self.scene().selected_view = id;
    }

    pub fn selected_inspection_view(&self) -> Option<String> {
        self.scene().selected_view.clone()
    }

    /// The camera of the composition's first primary view as the last frame
    /// drew it (never the observer): what inspection views frame the
    /// player's camera by. Reading it never waits on a frame.
    pub fn primary_camera(&self) -> Option<PrimaryCamera> {
        self.beside().readings.primary_camera
    }

    /// A point and frame (`[x, y, z, w]`) in an overlay space, in the world
    /// now; `None` when the space's node or joint draws nothing.
    pub fn resolve_point(
        &self,
        space: render_model::OverlaySpace,
        point: [f32; 3],
        rotation: [f32; 4],
    ) -> Option<([f32; 3], [f32; 4])> {
        let now = self.now();
        let mut scene = self.scene();
        let size = scene
            .renderer
            .surface_size()
            .unwrap_or(DEFAULT_CAPTURE_SIZE);
        let primary = scene
            .renderer
            .primary_overlay_camera(now, size)
            .map(|camera| camera.pose);
        let [x, y, z, w] = rotation;
        let (point, frame) = scene.renderer.resolve_overlay_point(
            space,
            glam::Vec3::from_array(point),
            glam::Quat::from_xyzw(x, y, z, w).normalize(),
            primary,
        )?;
        Some((point.to_array(), frame.to_array()))
    }

    /// Every overlay set's landmark pairs as the last frame drew them, with
    /// the simulation step the scene shows.
    pub fn landmark_sets(&self) -> (u64, Vec<crate::LandmarkSetReadout>) {
        self.beside().readings.landmarks.clone()
    }

    /// Draws one inspection view once into a target of its own, as
    /// [`SceneDriver::capture`] draws the composition; `None` when there is
    /// no such view.
    pub fn capture_view(
        &self,
        id: &str,
        size: Option<(u32, u32)>,
        overlays: bool,
    ) -> Option<Capture> {
        let now = self.now();
        let mut scene = self.scene();
        let view = scene
            .inspection_views
            .iter()
            .find(|view| view.id == id)?
            .clone();
        let (width, height) = size
            .or(scene.renderer.surface_size())
            .unwrap_or(DEFAULT_CAPTURE_SIZE);
        let target = OffscreenTarget::new(&self.gpu, width, height, scene.renderer.samples());
        let (width, height) = target.size();
        let shown = scene.overlay_shown.clone();
        if !overlays && shown.any() {
            scene
                .renderer
                .set_overlay_shown(crate::OverlayShown::default());
        }
        scene.present_at(now);
        scene.renderer.render_inspection_view(&target, id, now);
        // Only the sets this view shows.
        let drawn_overlays =
            (overlays && shown.any()).then(|| scene.renderer.overlay_revisions(&[id]));
        if !overlays && shown.any() {
            scene.renderer.set_overlay_shown(shown);
        }
        scene.realized_seconds = scene.elapsed_seconds;
        scene.collect_facts();
        scene.captures += 1;
        let capture = Capture {
            sequence: scene.captures,
            step: scene.step,
            held: scene.held,
            width,
            height,
            rgba: Vec::new(),
            video: false,
            composition: scene.composition.clone(),
            cameras: Vec::new(),
            overlays: drawn_overlays,
            view: Some(view),
            presentation_seconds: scene.drawn_seconds,
        };
        drop(scene);
        let mut rgba = Vec::new();
        target.read_rgba_into(&self.gpu, &mut rgba);
        Some(Capture { rgba, ..capture })
    }

    /// What inspection shows, and the overlay sets the renderer holds.
    pub fn overlays(&self) -> (crate::OverlayShown, Vec<crate::OverlaySetReadout>) {
        let scene = self.scene();
        (scene.overlay_shown.clone(), scene.renderer.overlay_sets())
    }

    /// Where collision wires are read while they show.
    pub fn collision_shown(&self) -> Option<crate::CollisionShown> {
        self.beside().readings.collision
    }

    /// Where the product UI's anchored views draw (see
    /// [`Renderer::set_viewport_anchors`]); the next frame follows.
    pub fn set_viewport_anchors(&self, anchors: RendererViewportAnchors) {
        let mut scene = self.scene();
        if scene.viewport_anchors == anchors {
            return;
        }
        scene.viewport_anchors = anchors.clone();
        scene.renderer.set_viewport_anchors(anchors);
        scene.changed = true;
    }

    /// The player's choices of renderer settings (video options), over
    /// every product request; the next frame draws with them.
    pub fn set_player_settings(&self, player: render_model::RendererSettingsOverrides) {
        let mut scene = self.scene();
        scene.player_settings = player;
        scene.renderer.set_player_settings(player);
        scene.changed = true;
    }

    /// As the scene was last left, so reading it never waits on a frame.
    pub fn view_state(&self) -> SceneView {
        let beside = self.beside();
        SceneView {
            held: beside.readings.held,
            observer: beside.readings.observer,
            composition: beside.readings.composition.clone(),
            changed: beside.changed,
        }
    }

    /// Animation facts the drawn frames produced since the last call.
    pub fn take_animation_facts(&self) -> Vec<AnimationFact> {
        std::mem::take(&mut self.beside().readings.animation_facts)
    }

    /// How video playbacks ended since the last call.
    pub fn take_video_facts(&self) -> Vec<VideoFact> {
        std::mem::take(&mut self.beside().readings.video_facts)
    }

    /// The scene's shadow layers and choice, as the last frame left them.
    pub fn shadow_report(&self) -> crate::ShadowReport {
        self.beside().readings.shadows.clone()
    }

    /// Every realized ghost plate as the last drawn view left it.
    pub fn ghost_plate_readouts(&self) -> Vec<GhostPlateReadout> {
        self.beside().readings.ghost_plates.clone()
    }

    /// Draw the committed scene: `draw` renders with the renderer at the
    /// presentation time the running world has reached (see
    /// [`PresentationLead`]), to its own target, given the camera clock. Facts the frame
    /// produced are kept for [`Self::take_animation_facts`] and
    /// [`Self::take_video_facts`]. Returns `draw`'s result and what the
    /// frame showed.
    pub fn draw<R>(&self, draw: impl FnOnce(&mut Renderer, f64) -> R) -> (R, SceneFrame) {
        let now = self.now();
        let mut scene = self.scene();
        scene.changed = false;
        self.beside().changed = false;
        scene.present_at(now);
        let result = draw(&mut scene.renderer, now);
        scene.realized_seconds = scene.elapsed_seconds;
        scene.collect_facts();
        let shown = SceneFrame {
            step: scene.step,
            held: scene.held,
            presentation_seconds: scene.drawn_seconds,
            renderer_id: scene.renderer_id,
            world_revision: scene.world_revision,
            composition: scene.composition.clone(),
            composition_revision: scene.composition_revision,
            observer: scene.observer,
            cameras: scene.renderer.drawn_cameras(),
            inspection_views: scene.renderer.drawn_inspection_views(),
        };
        (result, shown)
    }

    /// Waits up to `timeout` for `ready` to answer, for an output that
    /// draws only when something changed or was asked for. `ready` sees
    /// whether a change was applied since the last frame, and is asked
    /// again after every wake. Clips and video still end on Engine time
    /// while nothing draws: before waiting, the scene advances to the
    /// latest call undrawn, so their facts reach the Engine without a frame.
    pub fn wait_for_change<T>(
        &self,
        timeout: Duration,
        mut ready: impl FnMut(bool) -> Option<T>,
    ) -> Option<T> {
        if let Some(answer) = ready(self.beside().changed) {
            return Some(answer);
        }
        self.advance_undrawn();
        let beside = self.beside();
        if let Some(answer) = ready(beside.changed) {
            return Some(answer);
        }
        let (beside, _) = self
            .wake
            .wait_timeout(beside, timeout)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        ready(beside.changed)
    }

    /// Takes up what was handed over and moves the scene to the latest call
    /// without drawing, for an output that draws nothing now: clips and video
    /// still end on Engine time, and their facts reach the runtime.
    pub fn advance_undrawn(&self) {
        let mut scene = self.scene();
        if scene.realized_seconds != scene.elapsed_seconds {
            scene.realized_seconds = scene.elapsed_seconds;
            scene.renderer.advance_undrawn();
            scene.collect_facts();
        }
    }

    /// Marks the scene changed, so an output waiting in
    /// [`Self::wait_for_change`] draws it again.
    pub fn mark_changed(&self) {
        self.beside().changed = true;
        self.wake.notify_all();
    }

    /// Wakes an output waiting in [`Self::wait_for_change`] to ask `ready`
    /// again, after something it reads outside the scene changed. The lock
    /// is taken first, so the wake cannot fall between `ready` answering and
    /// the wait starting.
    pub fn notify(&self) {
        let _beside = self.beside();
        self.wake.notify_all();
    }

    /// Draws the committed scene once into a target of its own and reads it
    /// back, at `size`, else the window's surface size, else 1920x1080, at
    /// the pixel ratio the last output drew with. A capture changes no
    /// output's frame size or pixel ratio and draws into no output; its
    /// facts are reported like a drawn frame's. It is a reading
    /// ([`Renderer::capture_view_composition`]): under a temporal stage it
    /// draws without one, and leaves the outputs' history alone.
    pub fn capture(&self, size: Option<(u32, u32)>, overlays: bool) -> Capture {
        let now = self.now();
        let mut scene = self.scene();
        // A capture leaves out what inspection shows unless it asks for it.
        let shown = scene.overlay_shown.clone();
        if !overlays && shown.any() {
            scene
                .renderer
                .set_overlay_shown(crate::OverlayShown::default());
        }
        let (width, height) = size
            .or(scene.renderer.surface_size())
            .unwrap_or(DEFAULT_CAPTURE_SIZE);
        let target = OffscreenTarget::new(&self.gpu, width, height, scene.renderer.samples());
        let ratio = scene.renderer.pixel_ratio();
        let fitted_ratio = target.fitted_pixel_ratio(width, ratio);
        let (width, height) = target.size();
        scene.present_at(now);
        scene.renderer.set_pixel_ratio(fitted_ratio);
        let video = scene.renderer.capture_view_composition(&target, now).video;
        scene.renderer.set_pixel_ratio(ratio);
        // The sets the player view and the inspection views it drew show.
        let drawn_overlays = (overlays && shown.any()).then(|| {
            let drawn = scene.renderer.drawn_inspection_views();
            let views: Vec<&str> = std::iter::once(crate::PLAYER_VIEW)
                .chain(drawn.iter().map(|view| view.id.as_str()))
                .collect();
            scene.renderer.overlay_revisions(&views)
        });
        if !overlays && shown.any() {
            scene.renderer.set_overlay_shown(shown);
        }
        let cameras = scene.renderer.drawn_cameras();
        scene.realized_seconds = scene.elapsed_seconds;
        scene.collect_facts();
        scene.captures += 1;
        let capture = Capture {
            sequence: scene.captures,
            step: scene.step,
            held: scene.held,
            width,
            height,
            rgba: Vec::new(),
            video,
            composition: scene.composition.clone(),
            cameras,
            overlays: drawn_overlays,
            view: None,
            presentation_seconds: scene.drawn_seconds,
        };
        drop(scene);
        let mut rgba = Vec::new();
        target.read_rgba_into(&self.gpu, &mut rgba);
        Capture { rgba, ..capture }
    }

    /// The renderer's GPU pass readout ([`Renderer::gpu_readout`]).
    /// Samples per pixel the primary target should have (`Renderer::samples`).
    pub fn primary_samples(&self) -> u32 {
        self.scene().renderer.samples()
    }

    /// The renderer settings in effect and what the device refused.
    pub fn settings_readout(&self) -> crate::RendererSettingsReadout {
        self.beside().readings.settings
    }

    pub fn gpu_readout(&self) -> GpuReadout {
        self.beside().readings.gpu.clone()
    }

    /// The scatter patches taken in so far ([`Renderer::scatter_upload_readout`]).
    pub fn scatter_upload_readout(&self) -> crate::ScatterUploadReadout {
        self.beside().readings.scatter
    }

    /// Ops the renderer could not realize, by op, and the last one's detail.
    pub fn skipped_ops(&self) -> (BTreeMap<&'static str, u64>, Option<String>) {
        let beside = self.beside();
        (
            beside.readings.skipped_ops.clone(),
            beside.readings.last_skip.clone(),
        )
    }

    /// How far deferred preparation is ([`Renderer::preparation_readout`]).
    pub fn preparation_readout(&self) -> crate::PreparationReadout {
        self.beside().readings.preparation
    }

    /// Waits for every deferred texture and mesh being prepared and takes
    /// them in, so the scene shows everything committed: what a snapshot
    /// does before it is written. Every reading shows them from then on.
    pub fn settle_prepared(&self) {
        let mut scene = self.scene();
        let before = scene.renderer.preparation_readout();
        let issues = scene.renderer.settle_prepared();
        scene.record_issues(issues);
        if scene.renderer.preparation_readout() != before {
            scene.changed = true;
        }
    }

    /// The device the renderer draws with.
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    /// Holds the scene, first taking up everything handed over, in order.
    fn scene(&self) -> HeldScene<'_> {
        let mut scene = self
            .scene
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let edits = std::mem::take(&mut self.beside().edits);
        for edit in edits {
            self.take_up(&mut scene, edit);
        }
        HeldScene {
            scene,
            driver: self,
        }
    }

    fn beside(&self) -> MutexGuard<'_, Beside> {
        self.beside
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The presentation clock camera motion is sampled on.
    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }
}

/// The camera of the composition's first primary view as the last frame
/// drew it (never the observer).
fn primary_camera(renderer: &Renderer) -> Option<PrimaryCamera> {
    let size = renderer.surface_size().unwrap_or(DEFAULT_CAPTURE_SIZE);
    let camera = renderer.primary_drawn_camera(size)?;
    Some(PrimaryCamera {
        pose: crate::camera::pose_readout(camera.pose).0,
        orientation: camera.pose.orientation.to_array().map(f64::from),
        projection: camera.projection,
    })
}

/// The scene while held. Letting it go leaves the renderer's readings beside
/// it and wakes a waiting output when something changed.
struct HeldScene<'a> {
    scene: MutexGuard<'a, Scene>,
    driver: &'a SceneDriver,
}

impl std::ops::Deref for HeldScene<'_> {
    type Target = Scene;

    fn deref(&self) -> &Scene {
        &self.scene
    }
}

impl std::ops::DerefMut for HeldScene<'_> {
    fn deref_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }
}

impl Drop for HeldScene<'_> {
    fn drop(&mut self) {
        // The readings are read from the renderer before the small lock is
        // taken, which then only swaps them in: nothing the runtime calls
        // waits on the renderer through it.
        let scene = &mut *self.scene;
        let mut readings = Readings {
            settings: scene.renderer.settings_readout(),
            preparation: scene.renderer.preparation_readout(),
            gpu: scene.renderer.gpu_readout(),
            scatter: scene.renderer.scatter_upload_readout(),
            skipped_ops: scene.skipped_ops.clone(),
            last_skip: scene.last_skip.clone(),
            landmarks: (scene.step, scene.renderer.landmark_sets()),
            ghost_plates: scene.renderer.ghost_plate_readouts(),
            shadows: scene.renderer.shadow_report(),
            collision: scene.overlay_shown.collision,
            held: scene.held,
            observer: scene.observer,
            composition: scene.composition.clone(),
            primary_camera: primary_camera(&scene.renderer),
            animation_facts: std::mem::take(&mut scene.animation_facts),
            video_facts: std::mem::take(&mut scene.video_facts),
        };
        let changed = std::mem::take(&mut scene.changed);
        let mut beside = self.driver.beside();
        // Facts the runtime has not taken yet come first.
        let mut animation = std::mem::take(&mut beside.readings.animation_facts);
        animation.append(&mut readings.animation_facts);
        readings.animation_facts = animation;
        let mut video = std::mem::take(&mut beside.readings.video_facts);
        video.append(&mut readings.video_facts);
        readings.video_facts = video;
        let replaced = std::mem::replace(&mut beside.readings, readings);
        beside.changed |= changed;
        drop(beside);
        drop(replaced);
        if changed {
            self.driver.wake.notify_all();
        }
    }
}

impl Scene {
    /// Sets the presentation time a frame drawn at `now` shows: the
    /// committed time, plus the running world's lead since the last call.
    fn present_at(&mut self, now: f64) {
        let time = match self.lead {
            Some((lead, applied_at)) => {
                let owed = lead.owed_seconds + (now - applied_at).max(0.0) * lead.rate;
                let ahead = owed.clamp(0.0, lead.step_seconds);
                (self.elapsed_seconds + ahead)
                    .max(self.drawn_seconds)
                    .min(self.elapsed_seconds + lead.step_seconds)
            }
            None => self.elapsed_seconds,
        };
        self.drawn_seconds = time;
        self.renderer.set_presentation_time(time);
    }

    fn collect_facts(&mut self) {
        let animation = self.renderer.take_animation_facts();
        self.animation_facts.extend(animation);
        let video = self.renderer.take_video_facts();
        self.video_facts.extend(video);
    }

    /// Applies changes and the state they reach under the caller's lock.
    /// Returns whether anything changed.
    fn apply(
        &mut self,
        changes: &[SceneChange],
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
        state: SceneState,
        now: f64,
    ) -> bool {
        // What workers prepared since the last call is taken in first, so
        // every reading shows it from this call on.
        self.renderer.set_preparation_batch(state.preparation_batch);
        let installed = self.renderer.preparation_readout();
        let issues = self.renderer.take_prepared();
        self.record_issues(issues);
        let mut applied = self.renderer.preparation_readout() != installed;
        for change in changes {
            let issues = match change {
                SceneChange::Frame(frame) => self.renderer.apply(frame, resources),
                SceneChange::Presentation(frame) => {
                    self.renderer.apply_presentation(frame, resources, entities)
                }
                SceneChange::ViewComposition(composition) => {
                    self.renderer.set_view_composition(composition, now);
                    self.composition = Some(Arc::new(composition.clone()));
                    self.composition_revision += 1;
                    Vec::new()
                }
            };
            applied = true;
            self.record_issues(issues);
        }
        let previous_seconds = self.elapsed_seconds;
        let advanced = state.elapsed_seconds - self.elapsed_seconds;
        if advanced > 0.0 {
            let issues = self.renderer.advance_effects(advanced, entities);
            self.record_issues(issues);
            self.renderer.set_animation_time(state.elapsed_seconds);
            applied = true;
        }
        self.elapsed_seconds = state.elapsed_seconds;
        // A call without a new host observation (a debug command, a product
        // call between updates) carries the same owed time: anchoring it anew
        // would set the frames' clock back to it.
        let unobserved = |kept: &PresentationLead, lead: &PresentationLead| {
            kept.observed_ns == lead.observed_ns && previous_seconds == state.elapsed_seconds
        };
        self.lead = state
            .lead
            .filter(|_| !state.held)
            .map(|lead| match self.lead {
                Some((kept, anchored)) if unobserved(&kept, &lead) => (lead, anchored),
                _ => (lead, now),
            });
        self.world_revision = state.world_revision;
        if (self.step, self.held) != (state.step, state.held) {
            (self.step, self.held) = (state.step, state.held);
            applied = true;
        }
        // Video playbacks that could not start end here, not in a frame.
        self.collect_facts();
        self.changed |= applied;
        applied
    }

    fn record_issues(&mut self, issues: Vec<ApplyIssue>) {
        for issue in issues {
            *self.skipped_ops.entry(issue.op).or_default() += 1;
            self.last_skip = Some(format!("{}: {}", issue.op, issue.detail));
        }
    }
}
