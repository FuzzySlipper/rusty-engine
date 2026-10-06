//! The committed scene in one renderer, as the runtime's product calls leave
//! it, for whichever output draws it: the stream's render thread offscreen,
//! or the desktop shell to its window.
//!
//! The runtime applies each product call's renderer changes on its own
//! thread, as it commits them, with the Engine state the call reached, so no
//! frame shows a call's changes under the previous step. An output draws the
//! scene through [`SceneDriver::draw`], and a tool captures it through
//! [`SceneDriver::capture`]. Animation and video facts the frames produce
//! wait here for the runtime to report them.

use std::collections::BTreeMap;
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
const DEFAULT_CAPTURE_SIZE: (u32, u32) = (1280, 720);

/// One renderer change a committed product call made, in call order.
#[derive(Debug, Clone, Copy)]
pub enum SceneChange<'a> {
    Frame(&'a RenderFrameDiff),
    Presentation(&'a PresentationFrameDiff),
    ViewComposition(&'a RendererViewComposition),
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
}

/// What one drawn frame showed.
#[derive(Debug, Clone)]
pub struct SceneFrame {
    pub step: u64,
    pub held: bool,
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
}

/// The inspection state of the committed scene.
#[derive(Debug, Clone)]
pub struct SceneView {
    pub held: bool,
    pub observer: Option<RendererCameraPose>,
    /// The view composition the next frame draws.
    pub composition: Option<Arc<RendererViewComposition>>,
    /// A change was applied since the last frame was drawn.
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
}

/// The committed scene in a renderer on one device.
pub struct SceneDriver {
    gpu: Gpu,
    options: RendererOptions,
    epoch: Instant,
    scene: Mutex<Scene>,
    wake: Condvar,
}

struct Scene {
    renderer: Renderer,
    /// Something changed since the last frame was drawn.
    changed: bool,
    held: bool,
    step: u64,
    /// Engine presentation time the last applied call reached.
    elapsed_seconds: f64,
    /// Engine presentation time the renderer last advanced to, drawn or not.
    realized_seconds: f64,
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
}

impl SceneDriver {
    /// A renderer for the committed scene on `gpu`.
    pub fn new(gpu: Gpu, options: RendererOptions) -> Arc<Self> {
        Arc::new(Self {
            scene: Mutex::new(Scene {
                renderer: Renderer::new(&gpu, options),
                changed: true,
                held: true,
                step: 0,
                elapsed_seconds: 0.0,
                realized_seconds: 0.0,
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
                viewport_anchors: RendererViewportAnchors::new(),
            }),
            gpu,
            options,
            epoch: Instant::now(),
            wake: Condvar::new(),
        })
    }

    /// Applies one committed call's renderer changes in order and moves
    /// effects and animation to the Engine presentation time the call
    /// reached. `state` is what the call brought the Engine to; it lands
    /// with the changes, so no frame shows them under the previous step.
    pub fn apply<'a>(
        &self,
        changes: impl IntoIterator<Item = SceneChange<'a>>,
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
        state: SceneState,
    ) {
        let now = self.now();
        let mut scene = self.scene();
        if scene.apply(changes, resources, entities, state, now) {
            drop(scene);
            self.wake.notify_all();
        }
    }

    /// Replaces the renderer with one built from a complete baseline, after
    /// a product call's renderer work was lost or the world was replaced.
    /// No draw sees the empty renderer in between.
    pub fn rebaseline<'a>(
        &self,
        baseline: impl IntoIterator<Item = SceneChange<'a>>,
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
        state: SceneState,
    ) {
        let now = self.now();
        let mut scene = self.scene();
        scene.renderer = Renderer::new(&self.gpu, self.options);
        let observer = scene.observer;
        scene.renderer.set_observer(observer);
        let anchors = scene.viewport_anchors.clone();
        scene.renderer.set_viewport_anchors(anchors);
        scene.renderer_id += 1;
        scene.composition = None;
        scene.composition_revision = 0;
        scene.elapsed_seconds = state.elapsed_seconds;
        scene.renderer.set_animation_time(state.elapsed_seconds);
        scene.animation_facts.clear();
        scene.video_facts.clear();
        scene.apply(baseline, resources, entities, state, now);
        scene.changed = true;
        drop(scene);
        self.wake.notify_all();
    }

    /// Whether the simulation is held, and the step the scene shows, for a
    /// lifecycle change no product call published (pause, time mode).
    pub fn set_simulation(&self, held: bool, step: u64) {
        let mut scene = self.scene();
        if scene.held != held || scene.step != step {
            scene.held = held;
            scene.step = step;
            scene.changed = true;
            drop(scene);
            self.wake.notify_all();
        }
    }

    /// Draws the primary views from `pose` instead of the product's cameras,
    /// or from the product's cameras again with `None`.
    pub fn set_observer(&self, pose: Option<RendererCameraPose>) {
        let mut scene = self.scene();
        scene.observer = pose;
        scene.renderer.set_observer(pose);
        scene.changed = true;
        drop(scene);
        self.wake.notify_all();
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
        drop(scene);
        self.wake.notify_all();
    }

    pub fn view_state(&self) -> SceneView {
        let scene = self.scene();
        SceneView {
            held: scene.held,
            observer: scene.observer,
            composition: scene.composition.clone(),
            changed: scene.changed,
        }
    }

    /// Animation facts the drawn frames produced since the last call.
    pub fn take_animation_facts(&self) -> Vec<AnimationFact> {
        std::mem::take(&mut self.scene().animation_facts)
    }

    /// How video playbacks ended since the last call.
    pub fn take_video_facts(&self) -> Vec<VideoFact> {
        std::mem::take(&mut self.scene().video_facts)
    }

    /// The scene's shadow layers and choice, as the last frame left them.
    pub fn shadow_report(&self) -> crate::ShadowReport {
        self.scene().renderer.shadow_report()
    }

    /// Every realized ghost plate as the last drawn view left it.
    pub fn ghost_plate_readouts(&self) -> Vec<GhostPlateReadout> {
        self.scene().renderer.ghost_plate_readouts()
    }

    /// Draw the committed scene: `draw` renders with the renderer at the
    /// presentation time it is given, to its own target. Facts the frame
    /// produced are kept for [`Self::take_animation_facts`] and
    /// [`Self::take_video_facts`]. Returns `draw`'s result and what the
    /// frame showed.
    pub fn draw<R>(&self, draw: impl FnOnce(&mut Renderer, f64) -> R) -> (R, SceneFrame) {
        let now = self.now();
        let mut scene = self.scene();
        scene.changed = false;
        let result = draw(&mut scene.renderer, now);
        scene.realized_seconds = scene.elapsed_seconds;
        scene.collect_facts();
        let shown = SceneFrame {
            step: scene.step,
            held: scene.held,
            renderer_id: scene.renderer_id,
            world_revision: scene.world_revision,
            composition: scene.composition.clone(),
            composition_revision: scene.composition_revision,
            observer: scene.observer,
            cameras: scene.renderer.drawn_cameras(),
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
        let mut scene = self.scene();
        if let Some(answer) = ready(scene.changed) {
            return Some(answer);
        }
        if scene.realized_seconds != scene.elapsed_seconds {
            scene.realized_seconds = scene.elapsed_seconds;
            scene.renderer.advance_undrawn();
            scene.collect_facts();
        }
        let (scene, _) = self
            .wake
            .wait_timeout(scene, timeout)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        ready(scene.changed)
    }

    /// Marks the scene changed, so an output waiting in
    /// [`Self::wait_for_change`] draws it again.
    pub fn mark_changed(&self) {
        self.scene().changed = true;
        self.wake.notify_all();
    }

    /// Wakes an output waiting in [`Self::wait_for_change`] to ask `ready`
    /// again, after something it reads outside the scene changed. The lock
    /// is taken first, so the wake cannot fall between `ready` answering and
    /// the wait starting.
    pub fn notify(&self) {
        let _scene = self.scene();
        self.wake.notify_all();
    }

    /// Draws the committed scene once into a target of its own and reads it
    /// back, at `size`, else the window's surface size, else 1280x720. A
    /// capture changes no output's frame size and draws into no output; its
    /// facts are reported like a drawn frame's.
    pub fn capture(&self, size: Option<(u32, u32)>) -> Capture {
        let now = self.now();
        let mut scene = self.scene();
        let (width, height) = size
            .or(scene.renderer.surface_size())
            .unwrap_or(DEFAULT_CAPTURE_SIZE);
        let target = OffscreenTarget::new(&self.gpu, width, height);
        let video = scene.renderer.render_view_composition(&target, now).video;
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
        };
        drop(scene);
        let mut rgba = Vec::new();
        target.read_rgba_into(&self.gpu, &mut rgba);
        Capture { rgba, ..capture }
    }

    /// The renderer's GPU pass readout ([`Renderer::gpu_readout`]).
    pub fn gpu_readout(&self) -> GpuReadout {
        self.scene().renderer.gpu_readout()
    }

    /// Ops the renderer could not realize, by op, and the last one's detail.
    pub fn skipped_ops(&self) -> (BTreeMap<&'static str, u64>, Option<String>) {
        let scene = self.scene();
        (scene.skipped_ops.clone(), scene.last_skip.clone())
    }

    /// The device the renderer draws with.
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    fn scene(&self) -> MutexGuard<'_, Scene> {
        self.scene
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The presentation clock camera motion is sampled on.
    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }
}

impl Scene {
    fn collect_facts(&mut self) {
        let animation = self.renderer.take_animation_facts();
        self.animation_facts.extend(animation);
        let video = self.renderer.take_video_facts();
        self.video_facts.extend(video);
    }

    /// Applies changes and the state they reach under the caller's lock.
    /// Returns whether anything changed.
    fn apply<'a>(
        &mut self,
        changes: impl IntoIterator<Item = SceneChange<'a>>,
        resources: &dyn ResourceSource,
        entities: EntityPositions<'_>,
        state: SceneState,
        now: f64,
    ) -> bool {
        let mut applied = false;
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
        let advanced = state.elapsed_seconds - self.elapsed_seconds;
        if advanced > 0.0 {
            let issues = self.renderer.advance_effects(advanced, entities);
            self.record_issues(issues);
            self.renderer.set_animation_time(state.elapsed_seconds);
            applied = true;
        }
        self.elapsed_seconds = state.elapsed_seconds;
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
