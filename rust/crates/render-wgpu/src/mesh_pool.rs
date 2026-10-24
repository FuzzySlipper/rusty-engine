//! The meshes the selected scatter copies draw, in one vertex buffer and one
//! index buffer (#10033). Each mesh is copied in once (again when it is
//! defined anew), and a selected draw's indirect arguments name its place
//! by first index and base vertex, so every draw of one pass, pipeline and
//! material binds the same buffers and the run of them is one multi-draw
//! (`scatter_select.rs`, `frame.rs` `draw_selected`). A mesh with a second
//! vertex stream, or one that would grow the pool past the device's buffer
//! size, is not pooled; its draws keep its own buffers.

use std::collections::HashMap;

use crate::pipelines::VERTEX_FLOATS;
use crate::tables::{GpuMesh, MeshRef};
use crate::Gpu;

const VERTEX_BYTES: u64 = (VERTEX_FLOATS * 4) as u64;
const INDEX_BYTES: u64 = 4;
const INITIAL_VERTICES: u32 = 1 << 16;
const INITIAL_INDICES: u32 = 1 << 18;

/// A pooled mesh: where its vertices and indices start, and the buffer it
/// was copied from (a mesh defined anew has another).
struct Entry {
    source: wgpu::Buffer,
    base_vertex: u32,
    first_index: u32,
    vertices: u32,
    used: bool,
}

pub(crate) struct MeshPool {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    vertex_capacity: u32,
    index_capacity: u32,
    /// The most vertices and indices a buffer holds on this device.
    vertex_ceiling: u32,
    index_ceiling: u32,
    vertex_end: u32,
    index_end: u32,
    entries: HashMap<MeshRef, Entry>,
}

impl MeshPool {
    pub fn new(gpu: &Gpu) -> Self {
        let ceiling = |bytes: u64| (gpu.max_buffer_size() / bytes).min(u64::from(u32::MAX)) as u32;
        let (vertex_ceiling, index_ceiling) = (ceiling(VERTEX_BYTES), ceiling(INDEX_BYTES));
        let (vertex_capacity, index_capacity) = (
            INITIAL_VERTICES.min(vertex_ceiling),
            INITIAL_INDICES.min(index_ceiling),
        );
        Self {
            vertices: vertex_buffer(gpu, vertex_capacity),
            indices: index_buffer(gpu, index_capacity),
            vertex_capacity,
            index_capacity,
            vertex_ceiling,
            index_ceiling,
            vertex_end: 0,
            index_end: 0,
            entries: HashMap::new(),
        }
    }

    /// Bytes the pool's buffers hold.
    pub fn bytes(&self) -> u64 {
        u64::from(self.vertex_capacity) * VERTEX_BYTES
            + u64::from(self.index_capacity) * INDEX_BYTES
    }

    /// Start placing a selection's meshes: those it does not place again are
    /// dropped at [`Self::finish`]; when they held more than the rest, the
    /// pool is laid out anew.
    pub fn begin(&mut self) {
        for entry in self.entries.values_mut() {
            entry.used = false;
        }
    }

    /// Where `mesh` (`at`) is in the pool, copying it in when it is not, or
    /// was defined anew: its base vertex and first index. None for a mesh
    /// with a second vertex stream, or one the pool has no room for.
    pub fn place(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        at: &MeshRef,
        mesh: &GpuMesh,
    ) -> Option<(u32, u32)> {
        if mesh.second_stream().is_some() {
            return None;
        }
        if let Some(entry) = self.entries.get_mut(at) {
            if entry.source == mesh.vertices {
                entry.used = true;
                return Some((entry.base_vertex, entry.first_index));
            }
        }
        let vertices = (mesh.vertices.size() / VERTEX_BYTES) as u32;
        let indices = (mesh.indices.size() / INDEX_BYTES) as u32;
        if !self.reserve(gpu, encoder, vertices, indices) {
            return None;
        }
        let (base_vertex, first_index) = (self.vertex_end, self.index_end);
        encoder.copy_buffer_to_buffer(
            &mesh.vertices,
            0,
            &self.vertices,
            u64::from(base_vertex) * VERTEX_BYTES,
            Some(u64::from(vertices) * VERTEX_BYTES),
        );
        encoder.copy_buffer_to_buffer(
            &mesh.indices,
            0,
            &self.indices,
            u64::from(first_index) * INDEX_BYTES,
            Some(u64::from(indices) * INDEX_BYTES),
        );
        self.vertex_end += vertices;
        self.index_end += indices;
        self.entries.insert(
            at.clone(),
            Entry {
                source: mesh.vertices.clone(),
                base_vertex,
                first_index,
                vertices,
                used: true,
            },
        );
        Some((base_vertex, first_index))
    }

    /// Drop the meshes the selection no longer draws. Their space is given
    /// back when the pool is laid out anew: at the next selection, once the
    /// dropped held more than the kept.
    pub fn finish(&mut self) {
        self.entries.retain(|_, entry| entry.used);
        let kept: u64 = self
            .entries
            .values()
            .map(|entry| u64::from(entry.vertices))
            .sum();
        if u64::from(self.vertex_end) > 2 * kept.max(1) {
            // Laid out anew as the next selection places its meshes.
            self.entries.clear();
            self.vertex_end = 0;
            self.index_end = 0;
        }
    }

    /// Room for `vertices` and `indices` more, the buffers grown (their
    /// contents copied over) when they would not fit; false when they would
    /// pass the device's buffer size or it has no memory for them.
    fn reserve(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        vertices: u32,
        indices: u32,
    ) -> bool {
        let vertices_needed = u64::from(self.vertex_end) + u64::from(vertices);
        let indices_needed = u64::from(self.index_end) + u64::from(indices);
        if vertices_needed > u64::from(self.vertex_ceiling)
            || indices_needed > u64::from(self.index_ceiling)
        {
            return false;
        }
        let needed = self.vertex_end + vertices;
        if needed > self.vertex_capacity {
            let capacity = needed
                .checked_next_power_of_two()
                .unwrap_or(u32::MAX)
                .min(self.vertex_ceiling);
            let Ok(grown) = crate::gpu::allocating(&gpu.device, || vertex_buffer(gpu, capacity))
            else {
                return false;
            };
            encoder.copy_buffer_to_buffer(
                &self.vertices,
                0,
                &grown,
                0,
                Some(u64::from(self.vertex_end) * VERTEX_BYTES),
            );
            self.vertices = grown;
            self.vertex_capacity = capacity;
        }
        let needed = self.index_end + indices;
        if needed > self.index_capacity {
            let capacity = needed
                .checked_next_power_of_two()
                .unwrap_or(u32::MAX)
                .min(self.index_ceiling);
            let Ok(grown) = crate::gpu::allocating(&gpu.device, || index_buffer(gpu, capacity))
            else {
                return false;
            };
            encoder.copy_buffer_to_buffer(
                &self.indices,
                0,
                &grown,
                0,
                Some(u64::from(self.index_end) * INDEX_BYTES),
            );
            self.indices = grown;
            self.index_capacity = capacity;
        }
        true
    }
}

fn vertex_buffer(gpu: &Gpu, vertices: u32) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu scatter mesh pool vertices"),
        size: u64::from(vertices) * VERTEX_BYTES,
        usage: wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn index_buffer(gpu: &Gpu, indices: u32) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu scatter mesh pool indices"),
        size: u64::from(indices) * INDEX_BYTES,
        usage: wgpu::BufferUsages::INDEX
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}
