//! Indirect arguments for runs of pooled batches (#10697): a draw list's
//! batches of one pipeline and material whose meshes are in the static pool
//! (`mesh_pool.rs`) draw as one `multi_draw_indexed_indirect` over arguments
//! the CPU writes, rather than a `draw_indexed` each, so a list's draw calls
//! follow its materials rather than its meshes.
//!
//! Each list takes a region of one buffer for the frame, written once when
//! the list is encoded and read by the GPU when its encoder is submitted. A
//! view submits its encoders before the next frame starts, and a queue
//! write is ordered after the work already submitted, so the next frame
//! reuses the buffer from its start. A list that does not fit takes a
//! larger buffer, which later lists of the frame use; passes already encoded
//! keep the one they drew from.
//!
//! A device without indirect execution, or without a first instance in
//! indirect arguments, has no arena: its pooled batches draw one by one.

use crate::Gpu;

/// `wgpu::util::DrawIndexedIndirectArgs`.
pub(crate) const ARGS_BYTES: u64 = 20;
const INITIAL_ARGS: u64 = 4096;

pub(crate) struct DrawArgs {
    buffer: wgpu::Buffer,
    /// Arguments the buffer holds, and those the frame's lists took.
    capacity: u64,
    used: u64,
}

/// A list's region: the buffer and the byte offset of its first argument.
pub(crate) struct Region {
    pub buffer: wgpu::Buffer,
    pub offset: u64,
}

impl DrawArgs {
    /// None where indirect draws cannot name their first instance.
    pub fn new(gpu: &Gpu) -> Option<Self> {
        let indirect = gpu
            .adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::INDIRECT_EXECUTION);
        let first_instance = gpu
            .device
            .features()
            .contains(wgpu::Features::INDIRECT_FIRST_INSTANCE);
        (indirect && first_instance).then(|| Self {
            buffer: args_buffer(gpu, INITIAL_ARGS),
            capacity: INITIAL_ARGS,
            used: 0,
        })
    }

    /// A frame starts: its lists take the buffer from its start.
    pub fn begin_frame(&mut self) {
        self.used = 0;
    }

    /// Room for `count` arguments, in a larger buffer when this one is full.
    pub fn reserve(&mut self, gpu: &Gpu, count: u64) -> Region {
        if self.used + count > self.capacity {
            self.capacity = (self.capacity * 2).max(count.next_power_of_two());
            self.buffer = args_buffer(gpu, self.capacity);
            self.used = 0;
        }
        let offset = self.used * ARGS_BYTES;
        self.used += count;
        Region {
            buffer: self.buffer.clone(),
            offset,
        }
    }
}

fn args_buffer(gpu: &Gpu, count: u64) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu pooled draw arguments"),
        size: count * ARGS_BYTES,
        usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
