//! Overlay sets (`RenderDiff::SetOverlaySet`) and the renderer's own
//! inspection drawing, as coloured lines over primary views while inspection
//! shows them.
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
    OverlayItem, OverlaySetDescriptor, OverlayShape, OverlaySpace, RenderHandle, RenderLayer,
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
}

impl OverlayShown {
    pub fn any(&self) -> bool {
        self.all_sets
            || !self.sets.is_empty()
            || self.debug_layer
            || self.camera
            || !self.joints.is_empty()
            || !self.bounds.is_empty()
            || !self.ik.is_empty()
    }

    fn shows_set(&self, name: &str) -> bool {
        self.all_sets || self.sets.contains(name)
    }
}

/// One product set as the renderer holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlaySetReadout {
    pub name: String,
    pub items: usize,
    /// Counts this renderer's changes to the set.
    pub revision: u64,
    pub shown: bool,
}

/// The primary camera a frame's overlays are resolved for: the camera of
/// its first primary view, never the observer.
#[derive(Clone, Copy)]
pub(crate) struct OverlayCamera {
    pub pose: CameraPose,
    pub projection: RendererCameraProjection,
    pub aspect: f32,
    pub viewmodel_fov_y_degrees: f64,
}

struct RetainedSet {
    set: OverlaySetDescriptor,
    revision: u64,
}

pub(crate) struct Overlays {
    sets: BTreeMap<String, RetainedSet>,
    revisions: u64,
    pub shown: OverlayShown,
    /// This frame's vertices, by bucket (see [`bucket`]).
    ranges: [Range<u32>; 4],
    vertices: Vec<LineVertex>,
    buffer: Option<wgpu::Buffer>,
    shader: Option<(wgpu::ShaderModule, wgpu::PipelineLayout)>,
    /// Per target: depth-tested and on-top pipelines.
    pipelines: Vec<(ColorTarget, [wgpu::RenderPipeline; 2])>,
    /// The billboard labels the last resolve made.
    labels: usize,
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
            vertices: Vec::new(),
            buffer: None,
            shader: None,
            pipelines: Vec::new(),
            labels: 0,
        }
    }

    /// Replaces a product set; an empty one removes it.
    pub fn set(&mut self, name: &str, set: &OverlaySetDescriptor) {
        if set.items.is_empty() {
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
                revision: retained.revision,
                shown: self.shown.shows_set(name),
            })
            .collect()
    }

    /// The shown sets' revisions, as a capture that drew them reports.
    pub fn drawn_revisions(&self) -> BTreeMap<String, u64> {
        self.sets
            .iter()
            .filter(|(name, _)| self.shown.shows_set(name))
            .map(|(name, retained)| (name.clone(), retained.revision))
            .collect()
    }

    /// Lines this frame draws in a pass of `layer`.
    pub fn lines(&self, layer: ViewLayer) -> u32 {
        [false, true]
            .into_iter()
            .map(|on_top| self.ranges[bucket(layer, on_top)].len() as u32 / 2)
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
        for (on_top, pipeline) in [false, true].into_iter().zip(pipelines) {
            let range = self.ranges[bucket(layer, on_top)].clone();
            if range.is_empty() {
                continue;
            }
            pass.set_pipeline(pipeline);
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(range, 0..1);
            draws += 1;
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

    /// The shown overlay sets' revisions.
    pub(crate) fn overlay_revisions(&self) -> BTreeMap<String, u64> {
        self.overlays.drawn_revisions()
    }

    /// Shows or hides what inspection draws. The Debug layer's parts follow.
    pub fn set_overlay_shown(&mut self, shown: OverlayShown) {
        let debug_changed = shown.debug_layer != self.overlays.shown.debug_layer;
        let ik_added: Vec<u64> = shown
            .ik
            .difference(&self.overlays.shown.ik)
            .copied()
            .collect();
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
        // An instance newly drawing its IK is posed again to record it.
        for handle in ik_added {
            self.repose_animated(RenderHandle::new(handle));
        }
    }

    /// Resolves the frame's overlay lines for `camera` and uploads them;
    /// clears them when nothing is shown.
    pub(crate) fn resolve_overlays(&mut self, camera: Option<OverlayCamera>) {
        let mut vertices = std::mem::take(&mut self.overlays.vertices);
        vertices.clear();
        self.overlays.ranges = Default::default();
        let mut labels: Vec<(Vec3, String, [f32; 4], bool)> = Vec::new();
        if !self.overlays.shown.any() {
            self.overlays.vertices = vertices;
            self.realize_overlay_labels(labels);
            return;
        }
        // A viewmodel point drawn with the viewmodel's projection, as the
        // world point the world projection puts at the same pixel.
        let viewmodel_to_world = camera.map(|camera| {
            let world = camera::camera_matrices(camera.pose, &camera.projection, camera.aspect);
            let viewmodel = camera::viewmodel_matrices(
                &camera.projection,
                camera.viewmodel_fov_y_degrees,
                camera.aspect,
            );
            world.view_proj.inverse() * viewmodel.view_proj
        });
        let mut buckets: [Vec<LineVertex>; 4] = Default::default();
        let entities = self.entity_handles();
        let shown = &self.overlays.shown;
        for (name, retained) in &self.overlays.sets {
            if !shown.shows_set(name) {
                continue;
            }
            for item in &retained.set.items {
                if let Some((layer, to_pass)) = self.item_space(item, &entities) {
                    if let OverlayShape::Label { at, text } = &item.shape {
                        let at = to_pass.transform_point3(Vec3::from_array(*at));
                        let anchor = match (layer, viewmodel_to_world) {
                            (ViewLayer::Viewmodel, Some(to_world)) => to_world.project_point3(at),
                            (ViewLayer::Viewmodel, None) => continue,
                            _ => at,
                        };
                        // A viewmodel label is over the world it is drawn on.
                        let on_top = item.on_top || layer == ViewLayer::Viewmodel;
                        labels.push((anchor, text.clone(), item.color, on_top));
                        continue;
                    }
                    let mut pen = Pen {
                        out: &mut buckets[bucket(layer, item.on_top)],
                        to_pass,
                        color: item.color,
                    };
                    pen.shape(&item.shape);
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
        for (index, lines) in buckets.iter().enumerate() {
            let start = vertices.len() as u32;
            vertices.extend_from_slice(lines);
            self.overlays.ranges[index] = start..vertices.len() as u32;
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
    /// handles, and removes the ones the last frame had beyond them. A label
    /// whose text and look are unchanged keeps its raster.
    fn realize_overlay_labels(&mut self, labels: Vec<(Vec3, String, [f32; 4], bool)>) {
        use render_presentation::{
            BillboardAnchor, BillboardContent, BillboardDescriptor, BillboardFontRef,
            BillboardHandle, BillboardLayer, BillboardProjectionOp,
        };
        let count = labels.len();
        for (index, (anchor, text, color, on_top)) in labels.into_iter().enumerate() {
            let descriptor = BillboardDescriptor {
                anchor: BillboardAnchor::World {
                    position: anchor.to_array(),
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
        }
        for index in count..self.overlays.labels {
            let handle = BillboardHandle::new(FIRST_LABEL_HANDLE - index as u64);
            let _ = self.labels.apply(
                &BillboardProjectionOp::Destroy { handle },
                &crate::resources::NoResources,
                &|_| None,
            );
        }
        self.overlays.labels = count;
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
        item: &OverlayItem,
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
        match item.space {
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
