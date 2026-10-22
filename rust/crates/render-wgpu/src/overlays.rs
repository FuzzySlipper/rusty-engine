//! Overlay sets (`RenderDiff::SetOverlaySet`) and the renderer's own
//! inspection drawing, as coloured lines over primary views while inspection
//! shows them. A set's landmark pairs are measured here as drawn, for their
//! lines and for `engine.inspect.landmarks`.
//!
//! Each frame that shows anything, the items are resolved into the spaces
//! the view passes draw in: world items into the world pass, items on a
//! viewmodel node or in the primary camera's space into the viewmodel pass.
//! Lines draw at the end of each pass, depth-tested against it or on top.
//! While nothing is shown, nothing is resolved, uploaded or drawn, and the
//! Debug layer's own parts are hidden.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Range;

use glam::{Mat4, Quat, Vec3};
use render_host_contracts::RendererCameraProjection;
use render_model::{
    Landmark, LandmarkAim, LandmarkPair, OverlaySetDescriptor, OverlayShape, OverlaySpace,
    RenderHandle, RenderLayer,
};

use crate::camera::{self, CameraPose};
use crate::frame::ViewLayer;
use crate::target::{ColorTarget, DEPTH_FORMAT};
use crate::Renderer;

/// Segments in a wire sphere's circle.
const CIRCLE_SEGMENTS: usize = 24;
/// The length of the crossed lines marking a joint, in the instance's space.
const JOINT_MARKER: f32 = 0.02;
/// The length of the primary camera's axes.
const CAMERA_AXES: f32 = 0.2;
/// The cross marking an IK target.
const TARGET_MARKER: f32 = 0.04;
const JOINT_COLOR: [f32; 4] = [1.0, 0.82, 0.2, 1.0];
const BOUNDS_COLOR: [f32; 4] = [0.3, 0.9, 1.0, 1.0];
const CAMERA_COLOR: [f32; 4] = [0.95, 0.95, 0.95, 1.0];
const REACHED_COLOR: [f32; 4] = [0.3, 1.0, 0.4, 1.0];
const SHORT_COLOR: [f32; 4] = [1.0, 0.25, 0.25, 1.0];
/// The cross on a pair's `to` landmark, and how far a pair's aimed axis
/// draws from its `from` landmark, in metres.
const LANDMARK_MARKER: f32 = 0.008;
const AIM_LENGTH: f32 = 0.05;
/// Overlay labels take billboard handles counting down from here, apart from
/// the product's, which count up from 1.
const FIRST_LABEL_HANDLE: u64 = u64::MAX;
/// Label text height in CSS pixels, and the plate behind it.
const LABEL_HEIGHT: f32 = 13.0;
const LABEL_BACKGROUND: [f32; 4] = [0.0, 0.0, 0.0, 0.6];
const AXIS_COLORS: [[f32; 4]; 3] = [
    [1.0, 0.2, 0.2, 1.0],
    [0.2, 1.0, 0.2, 1.0],
    [0.3, 0.5, 1.0, 1.0],
];

const LINE_ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LineVertex {
    position: [f32; 3],
    color: [f32; 4],
}

/// What inspection shows; nothing by default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OverlayShown {
    /// Every product set, present and future.
    pub all_sets: bool,
    /// Product sets shown by name.
    pub sets: BTreeSet<String>,
    /// The parts of nodes on the Debug layer.
    pub debug_layer: bool,
    /// The primary camera's frustum and axes.
    pub camera: bool,
    /// Animated instances whose joint chain draws.
    pub joints: BTreeSet<u64>,
    /// Nodes whose part bounds draw.
    pub bounds: BTreeSet<u64>,
    /// Animated instances whose IK targets and shortfalls draw.
    pub ik: BTreeSet<u64>,
    /// Sets shown only in one view: the player view ([`PLAYER_VIEW`]) or an
    /// inspection view, by id.
    pub only_in: BTreeMap<String, String>,
    /// Views that show every product set, whatever else is shown: an
    /// inspection view pane's own overlay toggle.
    pub sets_in: BTreeSet<String>,
    /// Sets hidden by name from [`all_sets`](Self::all_sets) and
    /// [`sets_in`](Self::sets_in).
    pub hidden: BTreeSet<String>,
}

/// The view key of the primary views, for [`OverlayShown::only_in`].
pub const PLAYER_VIEW: &str = "player";

impl OverlayShown {
    pub fn any(&self) -> bool {
        self.all_sets
            || !self.sets.is_empty()
            || self.debug_layer
            || self.camera
            || !self.joints.is_empty()
            || !self.bounds.is_empty()
            || !self.ik.is_empty()
            || !self.sets_in.is_empty()
    }

    fn shows_set(&self, name: &str) -> bool {
        self.sets.contains(name) || (self.all_sets && !self.hidden.contains(name))
    }

    /// Where a set draws: `None` for every view, else each view by key.
    fn set_scopes(&self, name: &str) -> Vec<Option<&str>> {
        let mut scopes: Vec<Option<&str>> = Vec::new();
        if self.shows_set(name) {
            scopes.push(self.only_in.get(name).map(String::as_str));
        }
        if !scopes.contains(&None) && !self.hidden.contains(name) {
            for view in &self.sets_in {
                if !scopes.contains(&Some(view.as_str())) {
                    scopes.push(Some(view.as_str()));
                }
            }
        }
        scopes
    }
}

/// One product set as the renderer holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlaySetReadout {
    pub name: String,
    pub items: usize,
    pub pairs: usize,
    /// Counts this renderer's changes to the set.
    pub revision: u64,
    pub shown: bool,
}

/// One set's landmark pairs, measured as drawn now.
#[derive(Debug, Clone, PartialEq)]
pub struct LandmarkSetReadout {
    pub name: String,
    pub revision: u64,
    pub shown: bool,
    pub pairs: Vec<LandmarkMeasure>,
}

/// One landmark pair as drawn: its judging and, when both landmarks are
/// on something drawn, where they are and how they measure.
#[derive(Debug, Clone, PartialEq)]
pub struct LandmarkMeasure {
    pub name: String,
    pub tolerance: f32,
    pub aim: LandmarkAim,
    pub angle_tolerance: f32,
    pub drawn: Option<LandmarkDrawn>,
}

/// Where a pair's landmarks are drawn, in the world, and how they measure:
/// the distance between them (metres), the angle of the `from` axis to
/// what it aims at (degrees; `None` without an aim or an axis to measure),
/// whether both judged measures are within tolerance, and where `from` is
/// from `to` along the `to` landmark's frame (x right, y up, z back).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LandmarkDrawn {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub distance: f32,
    pub angle: Option<f32>,
    pub within: bool,
    pub offset: [f32; 3],
}

/// A landmark found as drawn, in its layer's pass coordinates.
#[derive(Clone, Copy)]
struct Placed {
    layer: ViewLayer,
    point: Vec3,
    axis: Option<Vec3>,
    frame: Quat,
}

impl Placed {
    /// This landmark in `layer`'s pass coordinates: a viewmodel pass's are
    /// the primary camera's space. `None` across layers without a camera.
    fn in_layer(self, layer: ViewLayer, camera: Option<CameraPose>) -> Option<Self> {
        if self.layer == layer {
            return Some(self);
        }
        let camera = camera?;
        let (turn, shift) = match layer {
            // World to the camera's space.
            ViewLayer::Viewmodel => {
                let inverse = camera.orientation.inverse();
                (inverse, -(inverse * camera.position))
            }
            // The camera's space to the world.
            _ => (camera.orientation, camera.position),
        };
        Some(Self {
            layer,
            point: turn * self.point + shift,
            axis: self.axis.map(|axis| turn * axis),
            frame: (turn * self.frame).normalize(),
        })
    }
}

/// A pair measured in its `from` landmark's pass coordinates.
struct PairDrawn {
    layer: ViewLayer,
    from: Placed,
    to: Placed,
    angle: Option<f32>,
    within: bool,
}

impl PairDrawn {
    fn measure(pair: &LandmarkPair, from: Placed, to: Placed) -> Self {
        let distance = from.point.distance(to.point);
        let aimed = match pair.aim {
            LandmarkAim::None => None,
            LandmarkAim::Axis => to.axis,
            LandmarkAim::Point => Some(to.point - from.point),
            LandmarkAim::Line => to
                .axis
                .map(|axis| to.point + axis * (from.point - to.point).dot(axis) - from.point),
        };
        let angle = from
            .axis
            .zip(aimed.and_then(Vec3::try_normalize))
            .map(|(axis, aimed)| axis.dot(aimed).clamp(-1.0, 1.0).acos().to_degrees());
        let within = (pair.tolerance <= 0.0 || distance <= pair.tolerance)
            && (pair.aim == LandmarkAim::None
                || angle.is_some_and(|angle| angle <= pair.angle_tolerance));
        Self {
            layer: from.layer,
            from,
            to,
            angle,
            within,
        }
    }

    fn label(&self, pair: &LandmarkPair) -> String {
        let mut text = pair.name.clone();
        if pair.tolerance > 0.0 || pair.aim == LandmarkAim::None {
            text += &format!(
                " {:.1} mm",
                self.from.point.distance(self.to.point) * 1000.0
            );
        }
        if pair.aim != LandmarkAim::None {
            match self.angle {
                Some(angle) => text += &format!(" {angle:.0}°"),
                None => text += " no axis",
            }
        }
        text
    }
}

/// The primary camera a frame's overlays are resolved for: the camera of
/// its first primary view, never the observer.
#[derive(Clone, Copy)]
pub(crate) struct OverlayCamera {
    pub pose: CameraPose,
    pub projection: RendererCameraProjection,
    pub aspect: f32,
}

struct RetainedSet {
    set: OverlaySetDescriptor,
    revision: u64,
}

pub(crate) struct Overlays {
    sets: BTreeMap<String, RetainedSet>,
    revisions: u64,
    pub shown: OverlayShown,
    /// This frame's vertices, by bucket (see [`bucket`]): every view's,
    /// and each view's own.
    ranges: [Range<u32>; 4],
    view_ranges: BTreeMap<String, [Range<u32>; 4]>,
    vertices: Vec<LineVertex>,
    buffer: Option<wgpu::Buffer>,
    shader: Option<(wgpu::ShaderModule, wgpu::PipelineLayout)>,
    /// Per target: depth-tested and on-top pipelines.
    pipelines: Vec<(ColorTarget, [wgpu::RenderPipeline; 2])>,
    /// The frame's labels, by reserved handle offset, for aiming them per
    /// view.
    labels: Vec<LabelRecord>,
    /// The primary camera the last drawn frame resolved overlays for: what
    /// landmark readouts place viewmodel and camera-space points by, so
    /// they read what was drawn without sampling camera motion again.
    drawn_camera: Option<CameraPose>,
}

/// Where one overlay label is, in its pass's coordinates.
struct LabelRecord {
    layer: ViewLayer,
    point: Vec3,
    /// The only view it shows in.
    view: Option<String>,
}

/// Index of a bucket of lines: world or viewmodel pass, tested or on top.
fn bucket(layer: ViewLayer, on_top: bool) -> usize {
    usize::from(layer == ViewLayer::Viewmodel) * 2 + usize::from(on_top)
}

impl Overlays {
    pub fn new() -> Self {
        Self {
            sets: BTreeMap::new(),
            revisions: 0,
            shown: OverlayShown::default(),
            ranges: Default::default(),
            view_ranges: BTreeMap::new(),
            vertices: Vec::new(),
            buffer: None,
            shader: None,
            pipelines: Vec::new(),
            labels: Vec::new(),
            drawn_camera: None,
        }
    }

    /// Replaces a product set; an empty one removes it.
    pub fn set(&mut self, name: &str, set: &OverlaySetDescriptor) {
        if set.is_empty() {
            self.sets.remove(name);
            return;
        }
        self.revisions += 1;
        self.sets.insert(
            name.to_owned(),
            RetainedSet {
                set: set.clone(),
                revision: self.revisions,
            },
        );
    }

    pub fn readout(&self) -> Vec<OverlaySetReadout> {
        self.sets
            .iter()
            .map(|(name, retained)| OverlaySetReadout {
                name: name.clone(),
                items: retained.set.items.len(),
                pairs: retained.set.pairs.len(),
                revision: retained.revision,
                shown: !self.shown.set_scopes(name).is_empty(),
            })
            .collect()
    }

    /// The revisions of the sets shown in any of `views` (view keys), as a
    /// capture that drew those views reports them.
    pub fn drawn_revisions(&self, views: &[&str]) -> BTreeMap<String, u64> {
        self.sets
            .iter()
            .filter(|(name, _)| {
                self.shown
                    .set_scopes(name)
                    .iter()
                    .any(|scope| scope.is_none_or(|view| views.contains(&view)))
            })
            .map(|(name, retained)| (name.clone(), retained.revision))
            .collect()
    }

    /// The bucket ranges a view draws: every view's, then its own.
    fn view_buckets<'a>(&'a self, view: &str) -> impl Iterator<Item = &'a [Range<u32>; 4]> {
        std::iter::once(&self.ranges).chain(self.view_ranges.get(view))
    }

    /// Lines this frame draws in `view`'s pass of `layer`.
    pub fn lines(&self, layer: ViewLayer, view: &str) -> u32 {
        self.view_buckets(view)
            .flat_map(|ranges| {
                [false, true].map(|on_top| ranges[bucket(layer, on_top)].len() as u32 / 2)
            })
            .sum()
    }

    /// Makes the pipelines a pass into `target` draws lines with.
    pub fn ensure_pipelines(
        &mut self,
        device: &wgpu::Device,
        frame_layout: &wgpu::BindGroupLayout,
        target: ColorTarget,
    ) {
        if self
            .pipelines
            .iter()
            .any(|(existing, _)| *existing == target)
        {
            return;
        }
        let (shader, layout) = self.shader.get_or_insert_with(|| {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("render-wgpu overlay lines"),
                source: wgpu::ShaderSource::Wgsl(include_str!("overlays.wgsl").into()),
            });
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-wgpu overlay lines"),
                bind_group_layouts: &[Some(frame_layout)],
                immediate_size: 0,
            });
            (shader, layout)
        });
        let pipeline = |on_top: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(if on_top {
                    "render-wgpu overlay lines on top"
                } else {
                    "render-wgpu overlay lines"
                }),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_line"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<LineVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &LINE_ATTRIBUTES,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs_line"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(if on_top {
                        wgpu::CompareFunction::Always
                    } else {
                        wgpu::CompareFunction::LessEqual
                    }),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: target.multisample(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipelines = [pipeline(false), pipeline(true)];
        self.pipelines.push((target, pipelines));
    }

    /// Draws this frame's lines for a pass of `layer` into `target`, whose
    /// frame bind group is bound. Returns the draws.
    pub fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        layer: ViewLayer,
        target: ColorTarget,
        view: &str,
    ) -> u32 {
        let (Some(buffer), Some((_, pipelines))) = (
            &self.buffer,
            self.pipelines
                .iter()
                .find(|(existing, _)| *existing == target),
        ) else {
            return 0;
        };
        let mut draws = 0;
        for ranges in self.view_buckets(view) {
            for (on_top, pipeline) in [false, true].into_iter().zip(pipelines) {
                let range = ranges[bucket(layer, on_top)].clone();
                if range.is_empty() {
                    continue;
                }
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(range, 0..1);
                draws += 1;
            }
        }
        draws
    }
}

/// Lines in one item space, appended to a bucket.
struct Pen<'a> {
    out: &'a mut Vec<LineVertex>,
    to_pass: Mat4,
    color: [f32; 4],
}

impl Pen<'_> {
    fn line(&mut self, from: Vec3, to: Vec3) {
        for point in [from, to] {
            self.out.push(LineVertex {
                position: self.to_pass.transform_point3(point).to_array(),
                color: self.color,
            });
        }
    }

    fn colored(&mut self, color: [f32; 4], from: Vec3, to: Vec3) {
        let kept = std::mem::replace(&mut self.color, color);
        self.line(from, to);
        self.color = kept;
    }

    fn cross(&mut self, at: Vec3, size: f32) {
        let half = size * 0.5;
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            self.line(at - axis * half, at + axis * half);
        }
    }

    fn axes(&mut self, at: Vec3, rotation: Quat, length: f32) {
        for (axis, color) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().zip(AXIS_COLORS) {
            self.colored(color, at, at + rotation * axis * length);
        }
    }

    fn wire_box(&mut self, corner: impl Fn(usize) -> Vec3) {
        for a in 0..8 {
            for axis in 0..3 {
                let b = a | (1 << axis);
                if b != a {
                    self.line(corner(a), corner(b));
                }
            }
        }
    }

    fn circle(&mut self, center: Vec3, u: Vec3, v: Vec3, radius: f32) {
        let point = |step: usize| {
            let angle = step as f32 / CIRCLE_SEGMENTS as f32 * std::f32::consts::TAU;
            center + (u * angle.cos() + v * angle.sin()) * radius
        };
        for step in 0..CIRCLE_SEGMENTS {
            self.line(point(step), point(step + 1));
        }
    }

    /// A frustum from its eight corners: near face, far face, and the edges
    /// between them; corner bits are x, y, far.
    fn frustum(&mut self, corner: impl Fn(usize) -> Vec3) {
        self.wire_box(corner);
    }

    fn shape(&mut self, shape: &OverlayShape) {
        let vec = Vec3::from_array;
        let quat = |[x, y, z, w]: [f32; 4]| Quat::from_xyzw(x, y, z, w).normalize();
        match shape {
            OverlayShape::Segment { from, to } => self.line(vec(*from), vec(*to)),
            OverlayShape::Marker { at, size } => self.cross(vec(*at), *size),
            OverlayShape::Axes {
                at,
                rotation,
                length,
            } => self.axes(vec(*at), quat(*rotation), *length),
            OverlayShape::WireBox {
                center,
                half_extents,
                rotation,
            } => {
                let (center, half, rotation) = (vec(*center), vec(*half_extents), quat(*rotation));
                self.wire_box(|bits| {
                    let sign = |axis: usize| if bits & (1 << axis) == 0 { -1.0 } else { 1.0 };
                    center + rotation * (half * Vec3::new(sign(0), sign(1), sign(2)))
                });
            }
            OverlayShape::WireSphere { center, radius } => {
                let center = vec(*center);
                self.circle(center, Vec3::X, Vec3::Y, *radius);
                self.circle(center, Vec3::Y, Vec3::Z, *radius);
                self.circle(center, Vec3::Z, Vec3::X, *radius);
            }
            OverlayShape::Frustum {
                at,
                rotation,
                fov_y_degrees,
                aspect,
                near,
                far,
            } => {
                let (at, rotation) = (vec(*at), quat(*rotation));
                let tan = (fov_y_degrees.to_radians() * 0.5).tan();
                self.frustum(|bits| {
                    let depth = if bits & 4 == 0 { *near } else { *far };
                    let x = if bits & 1 == 0 { -1.0 } else { 1.0 } * tan * aspect * depth;
                    let y = if bits & 2 == 0 { -1.0 } else { 1.0 } * tan * depth;
                    at + rotation * Vec3::new(x, y, -depth)
                });
            }
            // Labels are text, drawn by the label pass.
            OverlayShape::Label { .. } => {}
        }
    }
}

impl Renderer {
    /// The overlay sets this renderer holds.
    pub fn overlay_sets(&self) -> Vec<OverlaySetReadout> {
        self.overlays.readout()
    }

    /// The revisions of the overlay sets shown in any of `views`.
    pub(crate) fn overlay_revisions(&self, views: &[&str]) -> BTreeMap<String, u64> {
        self.overlays.drawn_revisions(views)
    }

    /// Shows or hides what inspection draws. The Debug layer's parts follow.
    pub fn set_overlay_shown(&mut self, shown: OverlayShown) {
        let debug_changed = shown.debug_layer != self.overlays.shown.debug_layer;
        self.overlays.shown = shown;
        if debug_changed {
            self.tables
                .parts
                .set_debug_shown(self.overlays.shown.debug_layer);
            let debug: Vec<RenderHandle> = self
                .tables
                .nodes
                .iter()
                .filter(|(_, node)| node.world_layer == RenderLayer::Debug)
                .map(|(handle, _)| *handle)
                .collect();
            self.tables.dirty_nodes.extend(debug);
        }
    }

    /// Resolves the frame's overlay lines for `camera` and uploads them;
    /// clears them when nothing is shown.
    pub(crate) fn resolve_overlays(&mut self, camera: Option<OverlayCamera>) {
        self.overlays.drawn_camera = camera.map(|camera| camera.pose);
        let mut vertices = std::mem::take(&mut self.overlays.vertices);
        vertices.clear();
        self.overlays.ranges = Default::default();
        self.overlays.view_ranges.clear();
        let mut labels: Vec<(LabelRecord, String, [f32; 4], bool)> = Vec::new();
        if !self.overlays.shown.any() {
            self.overlays.vertices = vertices;
            self.realize_overlay_labels(labels);
            return;
        }
        let mut buckets: [Vec<LineVertex>; 4] = Default::default();
        let mut view_buckets: BTreeMap<String, [Vec<LineVertex>; 4]> = BTreeMap::new();
        let entities = self.entity_handles();
        let shown = &self.overlays.shown;
        for (name, retained) in &self.overlays.sets {
            for scope in shown.set_scopes(name) {
                let out = match scope {
                    Some(view) => view_buckets.entry(view.to_owned()).or_default(),
                    None => &mut buckets,
                };
                for item in &retained.set.items {
                    let Some((layer, to_pass)) = self.item_space(item.space, &entities) else {
                        continue;
                    };
                    if let OverlayShape::Label { at, text } = &item.shape {
                        let point = to_pass.transform_point3(Vec3::from_array(*at));
                        // A viewmodel label is over the world it is drawn on.
                        let on_top = item.on_top || layer == ViewLayer::Viewmodel;
                        let record = LabelRecord {
                            layer,
                            point,
                            view: scope.map(str::to_owned),
                        };
                        labels.push((record, text.clone(), item.color, on_top));
                        continue;
                    }
                    let mut pen = Pen {
                        out: &mut out[bucket(layer, item.on_top)],
                        to_pass,
                        color: item.color,
                    };
                    pen.shape(&item.shape);
                }
            }
        }
        let primary = camera.map(|camera| camera.pose);
        for (name, retained) in &self.overlays.sets {
            if retained.set.pairs.is_empty() {
                continue;
            }
            for scope in shown.set_scopes(name) {
                let out = match scope {
                    Some(view) => view_buckets.entry(view.to_owned()).or_default(),
                    None => &mut buckets,
                };
                for pair in &retained.set.pairs {
                    let Some(drawn) = self.pair_drawn(pair, &entities, primary) else {
                        continue;
                    };
                    let color = if drawn.within {
                        REACHED_COLOR
                    } else {
                        SHORT_COLOR
                    };
                    let mut pen = Pen {
                        out: &mut out[bucket(drawn.layer, true)],
                        to_pass: Mat4::IDENTITY,
                        color,
                    };
                    let (from, to) = (drawn.from.point, drawn.to.point);
                    pen.line(from, to);
                    pen.cross(to, LANDMARK_MARKER);
                    if let (
                        LandmarkAim::Axis | LandmarkAim::Point | LandmarkAim::Line,
                        Some(axis),
                    ) = (pair.aim, drawn.from.axis)
                    {
                        pen.line(from, from + axis * AIM_LENGTH);
                    }
                    let record = LabelRecord {
                        layer: drawn.layer,
                        point: (from + to) * 0.5,
                        view: scope.map(str::to_owned),
                    };
                    labels.push((record, drawn.label(pair), color, true));
                }
            }
        }
        for &handle in &shown.joints {
            self.draw_joint_chain(RenderHandle::new(handle), &mut buckets);
        }
        for &handle in &shown.bounds {
            self.draw_bounds(RenderHandle::new(handle), &mut buckets);
        }
        for &handle in &shown.ik {
            self.draw_ik(RenderHandle::new(handle), &mut buckets);
        }
        if let (true, Some(camera)) = (shown.camera, camera) {
            let matrices = camera::camera_matrices(camera.pose, &camera.projection, camera.aspect);
            let to_world = matrices.view_proj.inverse();
            let mut pen = Pen {
                out: &mut buckets[bucket(ViewLayer::World, false)],
                to_pass: Mat4::IDENTITY,
                color: CAMERA_COLOR,
            };
            // wgpu clip depth runs from 0 (near) to 1 (far).
            pen.frustum(|bits| {
                let ndc = Vec3::new(
                    if bits & 1 == 0 { -1.0 } else { 1.0 },
                    if bits & 2 == 0 { -1.0 } else { 1.0 },
                    if bits & 4 == 0 { 0.0 } else { 1.0 },
                );
                to_world.project_point3(ndc)
            });
            pen.axes(camera.pose.position, camera.pose.orientation, CAMERA_AXES);
        }
        let mut append = |lines: &[Vec<LineVertex>; 4]| -> [Range<u32>; 4] {
            std::array::from_fn(|index| {
                let start = vertices.len() as u32;
                vertices.extend_from_slice(&lines[index]);
                start..vertices.len() as u32
            })
        };
        self.overlays.ranges = append(&buckets);
        for (view, lines) in &view_buckets {
            let ranges = append(lines);
            self.overlays.view_ranges.insert(view.clone(), ranges);
        }
        if !vertices.is_empty() {
            let bytes: &[u8] = bytemuck::cast_slice(&vertices);
            let fits = self
                .overlays
                .buffer
                .as_ref()
                .is_some_and(|buffer| buffer.size() >= bytes.len() as u64);
            if !fits {
                self.overlays.buffer =
                    Some(self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("render-wgpu overlay lines"),
                        size: (bytes.len() as u64).next_power_of_two(),
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }));
            }
            let buffer = self.overlays.buffer.as_ref().expect("made above");
            self.gpu.queue.write_buffer(buffer, 0, bytes);
        }
        self.overlays.vertices = vertices;
        self.realize_overlay_labels(labels);
    }

    /// Makes the frame's overlay labels billboard labels on reserved
    /// handles, aimed by each view that draws them, and removes the ones the last
    /// frame had beyond them. A label whose text and look are unchanged
    /// keeps its raster.
    fn realize_overlay_labels(&mut self, labels: Vec<(LabelRecord, String, [f32; 4], bool)>) {
        use render_presentation::{
            BillboardAnchor, BillboardContent, BillboardDescriptor, BillboardFontRef,
            BillboardHandle, BillboardLayer, BillboardProjectionOp,
        };
        let before = self.overlays.labels.len();
        let mut records = Vec::with_capacity(labels.len());
        for (index, (record, text, color, on_top)) in labels.into_iter().enumerate() {
            let descriptor = BillboardDescriptor {
                anchor: BillboardAnchor::World {
                    position: record.point.to_array(),
                },
                content: BillboardContent::Text {
                    localization_key: String::new(),
                    fallback_text: text,
                    arguments: Vec::new(),
                },
                font: BillboardFontRef::System {
                    family: "sans-serif".to_owned(),
                },
                height_pixels: LABEL_HEIGHT,
                color,
                background: LABEL_BACKGROUND,
                max_distance: f32::MAX,
                layer: if on_top {
                    BillboardLayer::AlwaysOnTop
                } else {
                    BillboardLayer::DepthTested
                },
                visible: true,
                layout: None,
            };
            let handle = BillboardHandle::new(FIRST_LABEL_HANDLE - index as u64);
            // Only a font failure can refuse, and the bundled face is used.
            let _ = self.labels.apply(
                &BillboardProjectionOp::Create { handle, descriptor },
                &crate::resources::NoResources,
                &|_| None,
            );
            records.push(record);
        }
        for index in records.len()..before {
            let handle = BillboardHandle::new(FIRST_LABEL_HANDLE - index as u64);
            let _ = self.labels.apply(
                &BillboardProjectionOp::Destroy { handle },
                &crate::resources::NoResources,
                &|_| None,
            );
        }
        self.overlays.labels = records;
    }

    /// Anchors the overlay labels for one view: a world label where it is, a
    /// viewmodel label through `viewmodel_to_world` (hidden without one),
    /// and a label of a set shown only in another view hidden.
    pub(crate) fn aim_overlay_labels(&mut self, view: &str, viewmodel_to_world: Option<Mat4>) {
        for (index, record) in self.overlays.labels.iter().enumerate() {
            let handle =
                render_presentation::BillboardHandle::new(FIRST_LABEL_HANDLE - index as u64);
            let Some(label) = self.labels.active.get_mut(&handle) else {
                continue;
            };
            label.anchor = if record.view.as_deref().is_some_and(|only| only != view) {
                None
            } else {
                match record.layer {
                    ViewLayer::Viewmodel => {
                        viewmodel_to_world.map(|to_world| to_world.project_point3(record.point))
                    }
                    _ => Some(record.point),
                }
            };
        }
    }

    /// A point and frame in an overlay space, in the world: a viewmodel's
    /// placed by `primary`, the primary camera. `None` when the space's node
    /// or joint draws nothing, or a viewmodel point has no camera.
    pub(crate) fn resolve_overlay_point(
        &self,
        space: OverlaySpace,
        point: Vec3,
        rotation: Quat,
        primary: Option<CameraPose>,
    ) -> Option<(Vec3, Quat)> {
        let (layer, to_pass) = self.item_space(space, &self.entity_handles())?;
        let to_world = match layer {
            ViewLayer::Viewmodel => {
                let primary = primary?;
                Mat4::from_rotation_translation(primary.orientation, primary.position) * to_pass
            }
            _ => to_pass,
        };
        let (_, frame, _) = to_world.to_scale_rotation_translation();
        Some((
            to_world.transform_point3(point),
            (frame * rotation).normalize(),
        ))
    }

    /// Every set's landmark pairs measured as the last frame drew them, in
    /// the world: a viewmodel's placed by the primary camera that frame drew
    /// with (none before a frame, when only world-to-world pairs place).
    pub fn landmark_sets(&self) -> Vec<LandmarkSetReadout> {
        if self
            .overlays
            .sets
            .values()
            .all(|retained| retained.set.pairs.is_empty())
        {
            return Vec::new();
        }
        let primary = self.overlays.drawn_camera;
        let entities = self.entity_handles();
        let to_world = |placed: Placed| placed.in_layer(ViewLayer::World, primary);
        self.overlays
            .sets
            .iter()
            .filter(|(_, retained)| !retained.set.pairs.is_empty())
            .map(|(name, retained)| LandmarkSetReadout {
                name: name.clone(),
                revision: retained.revision,
                shown: !self.overlays.shown.set_scopes(name).is_empty(),
                pairs: retained
                    .set
                    .pairs
                    .iter()
                    .map(|pair| LandmarkMeasure {
                        name: pair.name.clone(),
                        tolerance: pair.tolerance,
                        aim: pair.aim,
                        angle_tolerance: pair.angle_tolerance,
                        drawn: self.pair_drawn(pair, &entities, primary).and_then(|drawn| {
                            let (from, to) = (to_world(drawn.from)?, to_world(drawn.to)?);
                            let offset = to.frame.inverse() * (from.point - to.point);
                            Some(LandmarkDrawn {
                                from: from.point.to_array(),
                                to: to.point.to_array(),
                                distance: from.point.distance(to.point),
                                angle: drawn.angle,
                                within: drawn.within,
                                offset: offset.to_array(),
                            })
                        }),
                    })
                    .collect(),
            })
            .collect()
    }

    /// A pair measured as drawn, in its `from` landmark's pass coordinates;
    /// `None` when either landmark is on nothing drawn.
    fn pair_drawn(
        &self,
        pair: &LandmarkPair,
        entities: &HashMap<u64, RenderHandle>,
        primary: Option<CameraPose>,
    ) -> Option<PairDrawn> {
        let from = self.landmark(&pair.from, entities)?;
        let to = self
            .landmark(&pair.to, entities)?
            .in_layer(from.layer, primary)?;
        Some(PairDrawn::measure(pair, from, to))
    }

    /// A landmark as drawn, in its layer's pass coordinates; `None` when
    /// its node or a joint it names draws nothing.
    fn landmark(
        &self,
        landmark: &Landmark,
        entities: &HashMap<u64, RenderHandle>,
    ) -> Option<Placed> {
        let frame = |to_pass: Mat4| to_pass.to_scale_rotation_translation().1;
        let joint = |entity: u64, joint: u32| {
            self.item_space(OverlaySpace::Joint { entity, joint }, entities)
                .map(|(layer, to_pass)| (layer, to_pass.w_axis.truncate(), frame(to_pass)))
        };
        Some(match *landmark {
            Landmark::At {
                space,
                point,
                rotation: [x, y, z, w],
                axis,
            } => {
                let (layer, to_pass) = self.item_space(space, entities)?;
                let frame = (frame(to_pass) * Quat::from_xyzw(x, y, z, w).normalize()).normalize();
                Placed {
                    layer,
                    point: to_pass.transform_point3(Vec3::from_array(point)),
                    axis: (frame * Vec3::from_array(axis)).try_normalize(),
                    frame,
                }
            }
            Landmark::Between {
                entity,
                from,
                to,
                amount,
            } => {
                let (layer, from, frame) = joint(entity, from)?;
                let (_, to, _) = joint(entity, to)?;
                Placed {
                    layer,
                    point: from.lerp(to, amount),
                    axis: (to - from).try_normalize(),
                    frame,
                }
            }
            Landmark::Beyond {
                entity,
                previous,
                last,
                amount,
            } => {
                // The last bone runs along the last joint's frame as the
                // bone before it runs along the previous joint's.
                let (_, before, before_frame) = joint(entity, previous)?;
                let (layer, at, frame) = joint(entity, last)?;
                let bone = at - before;
                let axis = (frame * (before_frame.inverse() * bone)).try_normalize();
                Placed {
                    layer,
                    point: at + axis.unwrap_or(Vec3::ZERO) * bone.length() * amount,
                    axis,
                    frame,
                }
            }
            Landmark::Palm {
                entity,
                wrist,
                index,
                middle,
                pinky,
                along,
                depth,
                left,
            } => {
                let (layer, wrist, frame) = joint(entity, wrist)?;
                let (_, index, _) = joint(entity, index)?;
                let (_, middle, _) = joint(entity, middle)?;
                let (_, pinky, _) = joint(entity, pinky)?;
                // Index then pinky about the wrist turns out of a right
                // palm.
                let normal = (index - wrist).cross(pinky - wrist).try_normalize()?
                    * if left { -1.0 } else { 1.0 };
                Placed {
                    layer,
                    point: wrist.lerp(middle, along) + normal * depth,
                    axis: Some(normal),
                    frame,
                }
            }
        })
    }

    /// The lowest handle published for each entity.
    fn entity_handles(&self) -> HashMap<u64, RenderHandle> {
        let mut handles: HashMap<u64, RenderHandle> = HashMap::new();
        for (handle, node) in &self.tables.nodes {
            if let Some(entity) = node.entity {
                handles
                    .entry(entity)
                    .and_modify(|lowest| *lowest = (*lowest).min(*handle))
                    .or_insert(*handle);
            }
        }
        handles
    }

    /// The pass an item draws in and its space's transform into that pass's
    /// coordinates; `None` when its node or joint draws nothing.
    fn item_space(
        &self,
        space: OverlaySpace,
        entities: &HashMap<u64, RenderHandle>,
    ) -> Option<(ViewLayer, Mat4)> {
        let node_space = |entity: u64, joint: Option<u32>| {
            let handle = *entities.get(&entity)?;
            let node = self.tables.nodes.get(&handle)?;
            // A joint's position and orientation as drawn: a rig's joint
            // scale (an armature's unit conversion) is not the item's.
            let joint = match joint {
                Some(joint) => {
                    let pose = self.joint_pose_at(handle, self.rig_node(handle, joint)?)?;
                    let (_, rotation, translation) = pose.to_scale_rotation_translation();
                    Mat4::from_rotation_translation(rotation, translation)
                }
                None => Mat4::IDENTITY,
            };
            Some((pass_layer(node.world_layer), node.world * joint))
        };
        match space {
            OverlaySpace::World => Some((ViewLayer::World, Mat4::IDENTITY)),
            OverlaySpace::Camera => Some((ViewLayer::Viewmodel, Mat4::IDENTITY)),
            OverlaySpace::Model { entity } => node_space(entity, None),
            OverlaySpace::Joint { entity, joint } => node_space(entity, Some(joint)),
        }
    }

    fn draw_joint_chain(&self, handle: RenderHandle, buckets: &mut [Vec<LineVertex>; 4]) {
        let Some(node) = self.tables.nodes.get(&handle) else {
            return;
        };
        let mut pen = Pen {
            out: &mut buckets[bucket(pass_layer(node.world_layer), true)],
            to_pass: node.world,
            color: JOINT_COLOR,
        };
        for (joint, parent) in self.joint_bones(handle) {
            pen.line(parent, joint);
            pen.cross(joint, JOINT_MARKER);
        }
    }

    fn draw_bounds(&self, handle: RenderHandle, buckets: &mut [Vec<LineVertex>; 4]) {
        let Some(node) = self.tables.nodes.get(&handle) else {
            return;
        };
        let parts = &self.tables.parts;
        let Some(bounds) = node
            .parts
            .iter()
            .filter_map(|part| parts.world_bounds(*part))
            .reduce(|a, b| a.union(&b))
        else {
            return;
        };
        let mut pen = Pen {
            out: &mut buckets[bucket(pass_layer(node.world_layer), false)],
            to_pass: Mat4::IDENTITY,
            color: BOUNDS_COLOR,
        };
        pen.wire_box(|bits| {
            let pick = |axis: usize| {
                if bits & (1 << axis) == 0 {
                    bounds.min[axis]
                } else {
                    bounds.max[axis]
                }
            };
            Vec3::new(pick(0), pick(1), pick(2))
        });
    }

    fn draw_ik(&self, handle: RenderHandle, buckets: &mut [Vec<LineVertex>; 4]) {
        let Some(node) = self.tables.nodes.get(&handle) else {
            return;
        };
        let mut pen = Pen {
            out: &mut buckets[bucket(pass_layer(node.world_layer), true)],
            to_pass: node.world,
            color: REACHED_COLOR,
        };
        for (target, end, straightened) in self.ik_reaches(handle) {
            pen.color = if straightened {
                SHORT_COLOR
            } else {
                REACHED_COLOR
            };
            pen.cross(target, TARGET_MARKER);
            if let Some(end) = end {
                pen.line(end, target);
            }
        }
    }
}

/// The view pass a node's layer draws in, for its overlays.
fn pass_layer(layer: RenderLayer) -> ViewLayer {
    match ViewLayer::of(layer) {
        ViewLayer::Viewmodel => ViewLayer::Viewmodel,
        _ => ViewLayer::World,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use render_model::OverlayItem;

    fn set() -> OverlaySetDescriptor {
        OverlaySetDescriptor {
            items: vec![OverlayItem {
                shape: OverlayShape::Marker {
                    at: [0.0; 3],
                    size: 1.0,
                },
                space: OverlaySpace::World,
                color: [1.0; 4],
                on_top: false,
            }],
            pairs: Vec::new(),
        }
    }

    #[test]
    fn a_capture_reports_only_the_sets_shown_in_the_views_it_drew() {
        let mut overlays = Overlays::new();
        overlays.set("everywhere", &set());
        overlays.set("side-only", &set());
        overlays.shown.all_sets = true;
        overlays
            .shown
            .only_in
            .insert("side-only".to_owned(), "side".to_owned());
        let names = |views: &[&str]| -> Vec<String> {
            overlays.drawn_revisions(views).into_keys().collect()
        };
        assert_eq!(names(&["side"]), ["everywhere", "side-only"]);
        assert_eq!(names(&["grip"]), ["everywhere"]);
        assert_eq!(names(&[PLAYER_VIEW]), ["everywhere"]);
        assert_eq!(names(&[PLAYER_VIEW, "side"]), ["everywhere", "side-only"]);
    }

    #[test]
    fn a_set_hidden_by_name_stays_hidden_from_every_set_until_shown() {
        let mut overlays = Overlays::new();
        overlays.set("guide", &set());
        overlays.set("other", &set());
        overlays.shown.all_sets = true;
        overlays.shown.sets_in.insert("grip".to_owned());
        overlays.shown.hidden.insert("guide".to_owned());
        assert!(overlays.shown.set_scopes("guide").is_empty());
        assert_eq!(overlays.shown.set_scopes("other"), [None]);
        let names = |overlays: &Overlays, views: &[&str]| -> Vec<String> {
            overlays.drawn_revisions(views).into_keys().collect()
        };
        assert_eq!(names(&overlays, &[PLAYER_VIEW, "grip"]), ["other"]);
        // Shown by name again, it draws whatever else is hidden.
        overlays.shown.sets.insert("guide".to_owned());
        assert_eq!(names(&overlays, &[PLAYER_VIEW]), ["guide", "other"]);
    }

    #[test]
    fn a_view_showing_every_set_draws_them_there_only() {
        let mut overlays = Overlays::new();
        overlays.set("guide", &set());
        overlays.shown.sets_in.insert("grip".to_owned());
        assert!(overlays.shown.any());
        assert_eq!(overlays.shown.set_scopes("guide"), [Some("grip")]);
        assert_eq!(overlays.drawn_revisions(&[PLAYER_VIEW]).len(), 0);
        assert_eq!(overlays.drawn_revisions(&["grip"]).len(), 1);
        // Shown everywhere already: the view adds nothing.
        overlays.shown.all_sets = true;
        assert_eq!(overlays.shown.set_scopes("guide"), [None]);
        // Shown only in another view: both views draw it.
        overlays
            .shown
            .only_in
            .insert("guide".to_owned(), "side".to_owned());
        assert_eq!(
            overlays.shown.set_scopes("guide"),
            [Some("side"), Some("grip")]
        );
    }
}
