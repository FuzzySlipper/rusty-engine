//! What a temporal upscale stage reads beside the picture and its depth
//! (`upscale.rs`): a sub-pixel jitter of the primary views' projection, a
//! motion target, and whether the history still holds.
//!
//! - **Jitter.** While a temporal stage is selected, each primary frame
//!   offsets the projection its primary views draw with by the next point
//!   of a Halton(2, 3) sequence, within half a render pixel. Only what
//!   those views draw takes it (`ViewPass::jitter`): the frame uniform and
//!   the background. Culling, draw lists and shadow fitting keep the
//!   unjittered camera, so a still camera rebuilds nothing. Labels and
//!   offscreen and inspection views never jitter.
//! - **Motion.** While a temporal stage is selected, or inspection shows the
//!   motion (`OverlayShown::motion`), the primary views' world and viewmodel
//!   passes write a motion target beside their HDR colour at render
//!   resolution: per pixel, how far its point moved since the previous
//!   frame, in render pixels, x right and y down, without the jitter
//!   (`rusty::motion`). Every vertex path places its vertex where it stood
//!   then: a part by its previous model matrix, kept here by part id and
//!   rewritten only for parts that moved; a scatter copy under its part's;
//!   wind sway at the previous presentation time; particles from where they
//!   were drawn; falling drops back along their fall; a skinned vertex
//!   where the previous frame's pose put it (`animated.rs`), and a node or
//!   child on a joint by its part's previous model. Each view takes the
//!   previous frame's projection of its camera. Blended parts and particles
//!   blend their motion over what lies behind by their alpha.
//! - **No motion of their own** (they show what lies under them, or none):
//!   the sky and its clouds, which the background draws; water surfaces;
//!   sprites; ghost plates; overlay lines. A product shader's displacement
//!   keeps its offset, so it moves with its part but not on its own.
//! - **History reset.** The history no longer holds on the first frame with
//!   motion (a new renderer: a world replacement or baseline), and when a
//!   primary camera cuts or first draws, the observer comes or goes, the
//!   render resolution changes or the renderer's options do.
//!
//! With neither a temporal stage nor the inspection, nothing here runs:
//! no attachment, no jitter, no previous rows.

use std::collections::{HashMap, HashSet};

use glam::{DMat4, Mat4, Vec2, Vec3, Vec4};

use crate::camera::CameraMatrices;
use crate::frame::{PixelRect, ViewLayer};
use crate::tables::{PartId, Parts, PART_ROW_FLOATS};
use crate::upscale::FrameResolution;
use crate::{Gpu, RendererOptions};

/// The motion target's format: x and y in render pixels.
pub(crate) const MOTION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// `Motion` in `motion.wgsl`: two matrices and the previous camera.
const UNIFORM_FLOATS: usize = 16 + 16 + 4;

/// Floats in one previous model row (`motion.wgsl` `previous_models`).
const MODEL_FLOATS: usize = 16;

/// The jitter's phases at render scale 1: its sequence repeats after this
/// many frames, and after more as the render scale falls, so each output
/// pixel still sees about as many offsets.
const JITTER_PHASES: f32 = 8.0;

/// How a pipeline writes the motion target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotionWrite {
    /// Not at all: what lies under it keeps its motion.
    None,
    /// Its own motion over what lies under it.
    Cover,
    /// Its own motion blended over what lies under it by its alpha.
    Blend,
}

impl MotionWrite {
    pub fn target_state(self) -> wgpu::ColorTargetState {
        let blend = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        wgpu::ColorTargetState {
            format: MOTION_FORMAT,
            blend: (self == Self::Blend).then_some(wgpu::BlendState {
                color: blend,
                alpha: blend,
            }),
            write_mask: if self == Self::None {
                wgpu::ColorWrites::empty()
            } else {
                wgpu::ColorWrites::ALL
            },
        }
    }
}

/// The jitter's phases at `render_scale`.
pub(crate) fn jitter_phases(render_scale: f32) -> u32 {
    let scale = render_scale.clamp(0.25, 1.0);
    (JITTER_PHASES / (scale * scale)).round() as u32
}

/// The jitter at `phase`, in render pixels, x right and y down: the
/// Halton(2, 3) point `phase + 1`, centred on the pixel.
pub(crate) fn jitter(phase: u32) -> Vec2 {
    use crate::volumetric_fog::halton;
    Vec2::new(halton(phase + 1, 2) - 0.5, halton(phase + 1, 3) - 0.5)
}

/// `projection` offset by `pixels` of a viewport `size` pixels across:
/// every point it projects moves that far on screen.
pub(crate) fn jittered(projection: Mat4, pixels: Vec2, size: Vec2) -> Mat4 {
    let ndc = Vec2::new(2.0 * pixels.x / size.x, -2.0 * pixels.y / size.y);
    Mat4::from_translation(ndc.extend(0.0)) * projection
}

/// Clip space to the target pixels of `viewport`, offset by `jitter`
/// pixels: homogeneous, x and y over w.
fn to_pixels(viewport: PixelRect, jitter: Vec2) -> Mat4 {
    let (width, height) = (viewport.width as f32, viewport.height as f32);
    Mat4::from_cols(
        Vec4::new(0.5 * width, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -0.5 * height, 0.0, 0.0),
        Vec4::Z,
        Vec4::new(
            viewport.x as f32 + 0.5 * width + jitter.x,
            viewport.y as f32 + 0.5 * height + jitter.y,
            0.0,
            1.0,
        ),
    )
}

/// `camera`'s projection of a point's offset from its eye
/// (`Frame.relative_view_proj`), in f64 so the eye's own large coordinates
/// cancel.
fn relative_view_proj(camera: &CameraMatrices) -> Mat4 {
    (camera.view_proj.as_dmat4() * DMat4::from_translation(camera.eye.as_dvec3())).as_mat4()
}

/// Where a view's camera stood in the previous frame, unjittered.
#[derive(Clone, Copy)]
struct PreviousCamera {
    relative_view_proj: Mat4,
    eye: Vec3,
}

/// One primary view's camera, by its composition camera and layer.
type ViewKey = (String, ViewLayer);

/// The motion target at render resolution, at the world's samples, and its
/// single-sample resolve when those are more than one.
struct MotionTarget {
    size: (u32, u32),
    samples: u32,
    attachment: wgpu::TextureView,
    resolve: Option<wgpu::TextureView>,
}

impl MotionTarget {
    fn new(gpu: &Gpu, (width, height): (u32, u32), samples: u32) -> Self {
        let texture = |samples| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("render-wgpu motion"),
                    size: crate::target::extent(width, height),
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: MOTION_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        Self {
            size: (width, height),
            samples,
            attachment: texture(samples),
            resolve: (samples > 1).then(|| texture(1)),
        }
    }
}

/// What a view pass writes of the motion target (`ViewPass::motion`).
#[derive(Clone)]
pub(crate) struct MotionPass {
    pub attachment: wgpu::TextureView,
    pub resolve: Option<wgpu::TextureView>,
    /// The world pass clears it; the viewmodel's adds to it.
    pub clear: bool,
    /// The view's `Motion` uniform (`motion.wgsl`).
    pub uniform: [f32; UNIFORM_FLOATS],
}

impl MotionPass {
    /// The pass's colour attachment, after its HDR colour.
    pub fn attachment(&self, whole: bool) -> wgpu::RenderPassColorAttachment<'_> {
        wgpu::RenderPassColorAttachment {
            view: &self.attachment,
            depth_slice: None,
            resolve_target: self.resolve.as_ref(),
            ops: wgpu::Operations {
                load: if self.clear && whole {
                    wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                } else {
                    wgpu::LoadOp::Load
                },
                store: wgpu::StoreOp::Store,
            },
        }
    }

    /// The same attachment kept: a later pass of the same view.
    pub fn kept(&self) -> wgpu::RenderPassColorAttachment<'_> {
        wgpu::RenderPassColorAttachment {
            view: &self.attachment,
            depth_slice: None,
            resolve_target: self.resolve.as_ref(),
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        }
    }
}

/// What a primary frame drew for a temporal stage (`FrameStats::motion`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MotionFrame {
    /// The previous frame's picture no longer matches this one: a camera
    /// cut, the world was replaced, the render resolution or the settings
    /// changed, or it is the first frame with motion.
    pub history_reset: bool,
    /// The jitter's phase, while a temporal stage jitters the projection;
    /// its offset is `halton(phase + 1, 2) - 0.5, halton(phase + 1, 3) -
    /// 0.5` render pixels, x right and y down.
    pub jitter_phase: Option<u32>,
}

/// What the upscale stage reads of a frame's motion (`Upscale::run`).
pub(crate) struct TemporalInputs<'a> {
    /// The motion target, single-sample.
    pub motion: &'a wgpu::TextureView,
    /// The frame's jitter, render pixels.
    #[expect(
        dead_code,
        reason = "a temporal upscale method reads it; bilinear does not"
    )]
    pub jitter: Vec2,
    #[expect(
        dead_code,
        reason = "a temporal upscale method reads it; bilinear does not"
    )]
    pub history_reset: bool,
}

/// What draws the motion this primary frame.
pub(crate) struct FrameMotion {
    target: MotionPass,
    /// The jitter in render pixels, while a temporal stage jitters.
    pub jitter: Option<Vec2>,
}

/// Each part's previous model matrix (`motion.wgsl` `previous_models`).
struct PreviousModels {
    /// Each part's model as the last frame with motion drew it.
    drawn: Vec<[f32; MODEL_FLOATS]>,
    /// What the GPU reads: the frame before's, for parts that moved.
    rows: Vec<[f32; MODEL_FLOATS]>,
    /// Parts moved or made since the last frame with motion.
    moved: HashSet<PartId>,
    created: HashSet<PartId>,
    /// Parts that moved in the last frame with motion: their previous row
    /// catches up with them in the next.
    settling: Vec<PartId>,
    buffer: wgpu::Buffer,
}

impl PreviousModels {
    /// Every part's previous model as its current: no part has moved.
    fn new(gpu: &Gpu, parts: &Parts) -> Self {
        let count = parts.meta.len();
        let drawn: Vec<[f32; MODEL_FLOATS]> = (0..count).map(|id| model(parts, id)).collect();
        let buffer = models_buffer(gpu, count);
        if !drawn.is_empty() {
            gpu.queue
                .write_buffer(&buffer, 0, bytemuck::cast_slice(&drawn));
        }
        Self {
            rows: drawn.clone(),
            drawn,
            moved: HashSet::new(),
            created: HashSet::new(),
            settling: Vec::new(),
            buffer,
        }
    }

    /// Bring the rows to this frame: a part that moved since the last frame
    /// takes the model it was drawn with there, and one that moved there
    /// but not since takes its current. Returns whether the buffer was
    /// replaced (the frame binds it).
    fn advance(&mut self, gpu: &Gpu, parts: &Parts) -> bool {
        let count = parts.meta.len();
        let zero = [0.0; MODEL_FLOATS];
        self.drawn.resize(count, zero);
        self.rows.resize(count, zero);
        let mut dirty: Vec<PartId> = Vec::new();
        for id in self.created.drain() {
            if (id as usize) < count {
                self.drawn[id as usize] = model(parts, id as usize);
            }
        }
        let moved: Vec<PartId> = self.moved.drain().collect();
        for &id in moved.iter().chain(&self.settling) {
            let index = id as usize;
            if index >= count {
                continue;
            }
            self.rows[index] = self.drawn[index];
            self.drawn[index] = model(parts, index);
            dirty.push(id);
        }
        self.settling = moved;
        let grown = (count * MODEL_FLOATS * 4) as u64 > self.buffer.size();
        if grown {
            self.buffer = models_buffer(gpu, count.next_power_of_two());
            gpu.queue
                .write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.rows));
            return true;
        }
        dirty.sort_unstable();
        dirty.dedup();
        for id in dirty {
            gpu.queue.write_buffer(
                &self.buffer,
                u64::from(id) * (MODEL_FLOATS * 4) as u64,
                bytemuck::cast_slice(&self.rows[id as usize]),
            );
        }
        false
    }
}

/// A part's model matrix in its GPU row.
fn model(parts: &Parts, id: usize) -> [f32; MODEL_FLOATS] {
    let start = id * PART_ROW_FLOATS;
    let mut model = [0.0; MODEL_FLOATS];
    if let Some(row) = parts.gpu.get(start..start + MODEL_FLOATS) {
        model.copy_from_slice(row);
    }
    model
}

fn models_buffer(gpu: &Gpu, count: usize) -> wgpu::Buffer {
    crate::frame::storage_buffer(
        &gpu.device,
        "render-wgpu previous models",
        (count.max(1) * MODEL_FLOATS * 4) as u64,
    )
}

/// The renderer's temporal state: the motion uniform and previous models
/// the frame bind group binds, and what the last frame with motion drew.
pub(crate) struct Temporal {
    /// `rusty::motion` binding 15, written by each pass that draws motion.
    uniform: wgpu::Buffer,
    /// Binding 16 while no frame draws motion: one row nothing reads.
    placeholder: wgpu::Buffer,
    previous: Option<PreviousModels>,
    target: Option<MotionTarget>,
    /// The cameras of the last frame with motion, and this frame's so far.
    cameras: HashMap<ViewKey, PreviousCamera>,
    drawn: HashMap<ViewKey, PreviousCamera>,
    /// The last frame with motion: its presentation time, render
    /// resolution, options and observer.
    last: Option<(f64, FrameResolution, RendererOptions, bool)>,
    /// Frames drawn with a temporal stage: the jitter's place.
    jittered_frames: u32,
    /// Frames drawn with motion: what a skinned mesh's pose was last
    /// written after (`animated.rs`).
    motion_frames: u64,
    /// This frame's history no longer holds.
    reset: bool,
    /// The frame's presentation time, and the previous frame's.
    time: f64,
    previous_time: f64,
    frame: Option<FrameMotion>,
    shown: Option<MotionFrame>,
}

impl Temporal {
    pub fn new(gpu: &Gpu) -> Self {
        Self {
            uniform: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("render-wgpu motion"),
                size: (UNIFORM_FLOATS * 4) as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            placeholder: models_buffer(gpu, 1),
            previous: None,
            target: None,
            cameras: HashMap::new(),
            drawn: HashMap::new(),
            last: None,
            jittered_frames: 0,
            motion_frames: 0,
            reset: false,
            time: 0.0,
            previous_time: 0.0,
            frame: None,
            shown: None,
        }
    }

    /// Bindings 15 and 16 of the frame bind group.
    pub fn uniform(&self) -> &wgpu::Buffer {
        &self.uniform
    }

    pub fn previous_models(&self) -> &wgpu::Buffer {
        self.previous
            .as_ref()
            .map_or(&self.placeholder, |previous| &previous.buffer)
    }

    /// Whether frames draw motion, so what they drew is kept for the next:
    /// from the first frame with motion until one without.
    pub fn keeps_history(&self) -> bool {
        self.previous.is_some()
    }

    /// The frames with motion drawn so far.
    pub fn motion_frames(&self) -> u64 {
        self.motion_frames
    }

    /// Note the parts moved and made since `prepare` last ran, while frames
    /// draw motion.
    pub fn note_parts(&mut self, moved: &HashSet<PartId>, created: &[PartId]) {
        if let Some(previous) = &mut self.previous {
            previous.moved.extend(moved.iter().copied());
            previous.created.extend(created.iter().copied());
        }
    }

    /// Begin a primary frame. With `motion`, it draws the motion target at
    /// `resolution`'s render size and `samples`, jittered with `jittering`;
    /// `cut` when a primary camera cut or the observer came or went.
    /// Returns whether the frame bind group must be made again (the
    /// previous models' buffer changed).
    #[allow(clippy::too_many_arguments, reason = "one frame's temporal facts")]
    pub fn begin_frame(
        &mut self,
        gpu: &Gpu,
        parts: &Parts,
        motion: bool,
        jittering: bool,
        resolution: FrameResolution,
        samples: u32,
        options: &RendererOptions,
        observer: bool,
        cut: bool,
        time: f64,
    ) -> bool {
        self.drawn.clear();
        self.frame = None;
        self.shown = None;
        if !motion {
            // Nothing draws motion: drop it all, and the history with it.
            let rebind = self.previous.take().is_some();
            self.target = None;
            self.cameras.clear();
            self.last = None;
            return rebind;
        }
        let mut rebind = false;
        match &mut self.previous {
            Some(previous) => rebind |= previous.advance(gpu, parts),
            None => {
                self.previous = Some(PreviousModels::new(gpu, parts));
                rebind = true;
            }
        }
        let render = resolution.render;
        if self
            .target
            .as_ref()
            .is_none_or(|target| target.size != render || target.samples != samples)
        {
            self.target = Some(MotionTarget::new(gpu, render, samples));
        }
        self.reset = cut
            || self
                .last
                .is_none_or(|(_, last_resolution, last_options, last_observer)| {
                    last_resolution != resolution
                        || last_options != *options
                        || last_observer != observer
                });
        self.previous_time = self.last.map_or(time, |(last_time, ..)| last_time);
        self.time = time;
        if self.reset {
            self.cameras.clear();
        }
        self.last = Some((time, resolution, *options, observer));
        let phase = jittering.then(|| {
            let phases = jitter_phases(options.render_scale);
            let phase = self.jittered_frames % phases;
            self.jittered_frames = self.jittered_frames.wrapping_add(1);
            phase
        });
        let target = self.target.as_ref().expect("made above");
        self.frame = Some(FrameMotion {
            target: MotionPass {
                attachment: target.attachment.clone(),
                resolve: target.resolve.clone(),
                clear: true,
                uniform: [0.0; UNIFORM_FLOATS],
            },
            jitter: phase.map(jitter),
        });
        self.shown = Some(MotionFrame {
            history_reset: self.reset,
            jitter_phase: phase,
        });
        rebind
    }

    /// The frame's motion, while it draws motion.
    pub fn frame(&self) -> Option<&FrameMotion> {
        self.frame.as_ref()
    }

    /// The motion pass of one primary view's `layer`, drawn by `camera`
    /// (unjittered) in `viewport`, keyed by its composition camera: its
    /// uniform takes that camera's previous projection, or this one's when
    /// it has none (and the history resets).
    pub fn view_pass(
        &mut self,
        camera_id: &str,
        layer: ViewLayer,
        camera: &CameraMatrices,
        viewport: PixelRect,
    ) -> Option<MotionPass> {
        let frame = self.frame.as_ref()?;
        let key = (camera_id.to_owned(), layer);
        let now = PreviousCamera {
            relative_view_proj: relative_view_proj(camera),
            eye: camera.eye,
        };
        let previous = match self.cameras.get(&key) {
            Some(previous) => *previous,
            None => {
                self.reset = true;
                now
            }
        };
        self.drawn.insert(key, now);
        let mut pass = frame.target.clone();
        pass.clear = layer == ViewLayer::World;
        let mut uniform = Vec::with_capacity(UNIFORM_FLOATS);
        uniform.extend_from_slice(&previous.relative_view_proj.to_cols_array());
        uniform.extend_from_slice(
            &to_pixels(viewport, frame.jitter.unwrap_or(Vec2::ZERO)).to_cols_array(),
        );
        uniform.extend_from_slice(&[
            previous.eye.x,
            previous.eye.y,
            previous.eye.z,
            self.previous_time as f32,
        ]);
        pass.uniform.copy_from_slice(&uniform);
        Some(pass)
    }

    /// Write a view pass's motion uniform, before it is encoded.
    pub fn write(&self, gpu: &Gpu, pass: &MotionPass) {
        gpu.queue
            .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&pass.uniform));
    }

    /// End a primary frame: its cameras are the next frame's previous ones.
    /// Returns what the frame drew, while it drew motion.
    pub fn end_frame(&mut self) -> Option<MotionFrame> {
        self.frame.as_ref()?;
        self.motion_frames += 1;
        self.cameras = std::mem::take(&mut self.drawn);
        if let Some(shown) = &mut self.shown {
            shown.history_reset |= self.reset;
        }
        self.shown
    }

    /// The single-sample motion target the last frame drew, with its
    /// jitter and reset, for the upscale stage and the inspection.
    pub fn inputs(&self) -> Option<TemporalInputs<'_>> {
        let target = self.target.as_ref()?;
        let frame = self.frame.as_ref()?;
        Some(TemporalInputs {
            motion: target.resolve.as_ref().unwrap_or(&target.attachment),
            jitter: frame.jitter.unwrap_or(Vec2::ZERO),
            history_reset: self.shown.is_some_and(|shown| shown.history_reset) || self.reset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_jitter_stays_within_half_a_pixel_and_covers_the_pixel() {
        let points: Vec<Vec2> = (0..jitter_phases(1.0)).map(jitter).collect();
        assert_eq!(points.len(), 8);
        for point in &points {
            assert!(point.abs().max_element() < 0.5, "{point}");
        }
        // Halton(2, 3) from 1: 1/2 - 1/2, 1/3 - 1/2.
        assert!((points[0] - Vec2::new(0.0, -1.0 / 6.0)).length() < 1e-6);
        // Every quadrant of the pixel is visited.
        for (x, y) in [(false, false), (false, true), (true, false), (true, true)] {
            assert!(points.iter().any(|p| (p.x > 0.0) == x && (p.y > 0.0) == y));
        }
        // A lower render scale takes more phases.
        assert_eq!(jitter_phases(0.5), 32);
        assert_eq!(jitter_phases(0.75), 14);
    }

    /// The frames' temporal state, without a scene: phases in order, each
    /// view's previous camera, and the resets.
    #[test]
    fn frames_jitter_through_their_phases_and_reset_when_their_history_breaks() {
        let gpu = Gpu::headless().expect("a wgpu adapter");
        let parts = Parts::default();
        let mut temporal = Temporal::new(&gpu);
        let options = RendererOptions::default();
        let resolution = FrameResolution::scaled((320, 180), 1.0);
        let viewport = PixelRect::whole(320, 180);
        let projection = render_host_contracts::RendererCameraProjection::Perspective {
            fov_y_degrees: 60.0,
            near: 0.1,
            far: 100.0,
        };
        let at = |x: f32| {
            crate::camera::camera_matrices(
                crate::camera::CameraPose {
                    position: Vec3::new(x, 1.0, 0.0),
                    orientation: glam::Quat::IDENTITY,
                },
                &projection,
                viewport.aspect(),
            )
        };
        let frame = |temporal: &mut Temporal, options: &RendererOptions, x: f32, cut: bool| {
            temporal.begin_frame(
                &gpu, &parts, true, true, resolution, 1, options, false, cut, 0.0,
            );
            let pass = temporal
                .view_pass("eye", ViewLayer::World, &at(x), viewport)
                .expect("the frame draws motion");
            (temporal.end_frame().expect("a frame with motion"), pass)
        };
        let mut phases = Vec::new();
        for index in 0..10 {
            let (shown, pass) = frame(&mut temporal, &options, index as f32, false);
            assert_eq!(shown.history_reset, index == 0, "frame {index}");
            let phase = shown.jitter_phase.expect("a temporal stage jitters");
            phases.push(phase);
            // The uniform carries this frame's jitter to pixels, and the
            // previous frame's camera (this one's on the first).
            let jitter = jitter(phase);
            assert_eq!(pass.uniform[28], 160.0 + jitter.x);
            assert_eq!(pass.uniform[29], 90.0 + jitter.y);
            assert_eq!(pass.uniform[32], (index.max(1) - 1) as f32);
        }
        assert_eq!(phases, [0, 1, 2, 3, 4, 5, 6, 7, 0, 1]);
        assert!(
            frame(&mut temporal, &options, 0.0, true).0.history_reset,
            "a cut"
        );
        let scaled = RendererOptions {
            render_scale: 0.5,
            ..options
        };
        assert!(
            frame(&mut temporal, &scaled, 0.0, false).0.history_reset,
            "a setting"
        );
        assert!(!frame(&mut temporal, &scaled, 0.0, false).0.history_reset);
        // Another camera has no history of its own.
        temporal.begin_frame(
            &gpu, &parts, true, true, resolution, 1, &scaled, false, false, 0.0,
        );
        temporal.view_pass("other", ViewLayer::World, &at(0.0), viewport);
        assert!(temporal.end_frame().unwrap().history_reset, "a new camera");
        // A frame without motion draws none and drops the history.
        temporal.begin_frame(
            &gpu, &parts, false, false, resolution, 1, &scaled, false, false, 0.0,
        );
        assert!(temporal
            .view_pass("other", ViewLayer::World, &at(0.0), viewport)
            .is_none());
        assert_eq!(temporal.end_frame(), None);
        assert!(frame(&mut temporal, &scaled, 0.0, false).0.history_reset);
    }

    #[test]
    fn a_jittered_projection_moves_every_point_by_the_jitter_and_to_pixels_undoes_it() {
        let projection = glam::camera::rh::proj::directx::perspective(1.0, 1.5, 0.1, 100.0);
        let size = Vec2::new(300.0, 200.0);
        let viewport = PixelRect {
            x: 20,
            y: 10,
            width: 300,
            height: 200,
        };
        let jitter = Vec2::new(0.25, -0.375);
        let shifted = jittered(projection, jitter, size);
        let pixels = |matrix: Mat4, offset: Vec2| {
            let point = Vec4::new(0.7, -0.4, -6.0, 1.0);
            let clip = to_pixels(viewport, offset) * matrix * point;
            Vec2::new(clip.x, clip.y) / clip.w
        };
        // The jittered projection draws the point `jitter` pixels over...
        let moved = pixels(shifted, Vec2::ZERO) - pixels(projection, Vec2::ZERO);
        assert!((moved - jitter).length() < 1e-3, "{moved}");
        // ...where the unjittered projection carried by the jitter puts it,
        // so a still point has no motion.
        let still = pixels(shifted, Vec2::ZERO) - pixels(projection, jitter);
        assert!(still.length() < 1e-3, "{still}");
        // NDC (0, 0) is the viewport's centre, y down.
        let centre = to_pixels(viewport, Vec2::ZERO) * Vec4::new(0.0, 0.0, 0.5, 1.0);
        assert_eq!(Vec2::new(centre.x, centre.y), Vec2::new(170.0, 110.0));
        let top = to_pixels(viewport, Vec2::ZERO) * Vec4::new(0.0, 1.0, 0.5, 1.0);
        assert_eq!(top.y, 10.0);
    }
}
