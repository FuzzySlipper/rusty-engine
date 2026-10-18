//! Scatter runs' copies (#10016): every copy of every scatter patch as a row
//! of one storage buffer (`rusty::view::copies`), a run's rows contiguous.
//! A run is one part (its node's model, its material's colour and fade); its
//! draw's instances index the rows, so the CPU lists, culling and shadow
//! casters see one part a run however many copies it holds, and a copy costs
//! its row and nothing on the CPU after it is written.
//!
//! The buffer holds as many rows as one storage binding can
//! (`Gpu::storage_binding_bytes`), at most `MAX_ROWS`; a run that does not
//! fit is refused, draws nothing and is counted
//! (`ScatterSelectionReadout::over_limit`).

use crate::Gpu;

/// `ScatterCopy` in types.wgsl: place and scale, turn, tint and part, rank
/// and the copy mesh's top.
pub(crate) const COPY_FLOATS: usize = 16;
/// Where in a row its part's id is.
pub(crate) const PART_LANE: usize = 11;
const COPY_BYTES: u64 = (COPY_FLOATS * 4) as u64;
const INITIAL_COPIES: u32 = 4096;
/// The most rows a selection entry can name (24 bits, `view.wgsl`).
pub(crate) const MAX_ROWS: u32 = 1 << 24;

/// A run's rows: the first and how many, or none and how many were
/// refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CopyRange {
    pub first: u32,
    pub count: u32,
    pub refused: u32,
}

pub(crate) struct ScatterCopies {
    buffer: wgpu::Buffer,
    /// Rows the buffer holds, and the most one binding holds.
    capacity: u32,
    ceiling: u32,
    /// Rows past the last allocation never used.
    end: u32,
    /// Freed ranges, by first row, none touching another.
    free: Vec<CopyRange>,
    /// Rows written since the last upload.
    pending: Vec<(u32, Vec<f32>)>,
    /// Rows held by runs, and refused to runs still made.
    pub held: u64,
    pub refused: u64,
}

impl ScatterCopies {
    pub fn new(gpu: &Gpu) -> Self {
        let ceiling = (gpu.storage_binding_bytes() / COPY_BYTES).min(u64::from(MAX_ROWS)) as u32;
        let capacity = INITIAL_COPIES.min(ceiling);
        Self {
            buffer: buffer(gpu, capacity),
            capacity,
            ceiling,
            end: 0,
            free: Vec::new(),
            pending: Vec::new(),
            held: 0,
            refused: 0,
        }
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Rows for a run of `rows` (each `COPY_FLOATS` long): the first free
    /// range that fits, or new ones past the end; none when they would
    /// pass the ceiling.
    pub fn insert(&mut self, rows: Vec<f32>) -> CopyRange {
        let count = (rows.len() / COPY_FLOATS) as u32;
        let fits = self.free.iter().position(|range| range.count >= count);
        if fits.is_none() && u64::from(self.end) + u64::from(count) > u64::from(self.ceiling) {
            return self.refuse(count);
        }
        let first = match fits {
            Some(index) => {
                let range = &mut self.free[index];
                let first = range.first;
                range.first += count;
                range.count -= count;
                if range.count == 0 {
                    self.free.remove(index);
                }
                first
            }
            None => {
                let first = self.end;
                self.end += count;
                first
            }
        };
        self.held += u64::from(count);
        self.pending.push((first, rows));
        CopyRange {
            first,
            count,
            refused: 0,
        }
    }

    /// No rows for a run of `count`, counted as refused.
    pub fn refuse(&mut self, count: u32) -> CopyRange {
        self.refused += u64::from(count);
        CopyRange {
            first: 0,
            count: 0,
            refused: count,
        }
    }

    /// Give a run's rows back, with their write if it has not been
    /// uploaded yet.
    pub fn remove(&mut self, range: CopyRange) {
        self.refused -= u64::from(range.refused);
        if range.count == 0 {
            return;
        }
        self.held -= u64::from(range.count);
        self.pending.retain(|(first, _)| *first != range.first);
        let at = self.free.partition_point(|free| free.first < range.first);
        self.free.insert(at, range);
        // Join it to its neighbours.
        if at + 1 < self.free.len() {
            let next = self.free[at + 1];
            if self.free[at].first + self.free[at].count == next.first {
                self.free[at].count += next.count;
                self.free.remove(at + 1);
            }
        }
        if at > 0 {
            let previous = self.free[at - 1];
            if previous.first + previous.count == self.free[at].first {
                self.free[at - 1].count += self.free[at].count;
                self.free.remove(at);
            }
        }
        // A free range at the end is the end again.
        if let Some(last) = self.free.last().copied() {
            if last.first + last.count == self.end {
                self.end = last.first;
                self.free.pop();
            }
        }
    }

    /// Write the rows inserted since the last upload, growing the buffer
    /// first (its rows copied over) when they reach past it. True when the
    /// buffer was replaced, so bind groups naming it are made again.
    pub fn upload(&mut self, gpu: &Gpu) -> bool {
        let mut replaced = false;
        if self.end > self.capacity {
            let capacity = self
                .end
                .checked_next_power_of_two()
                .unwrap_or(u32::MAX)
                .min(self.ceiling);
            let grown = buffer(gpu, capacity);
            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render-wgpu scatter copies grow"),
                });
            encoder.copy_buffer_to_buffer(
                &self.buffer,
                0,
                &grown,
                0,
                u64::from(self.capacity) * COPY_BYTES,
            );
            gpu.queue.submit([encoder.finish()]);
            self.buffer = grown;
            self.capacity = capacity;
            replaced = true;
        }
        for (first, rows) in self.pending.drain(..) {
            gpu.queue.write_buffer(
                &self.buffer,
                u64::from(first) * COPY_BYTES,
                bytemuck::cast_slice(&rows),
            );
        }
        replaced
    }
}

fn buffer(gpu: &Gpu, rows: u32) -> wgpu::Buffer {
    gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render-wgpu scatter copies"),
        size: u64::from(rows.max(1)) * COPY_BYTES,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

/// A copy's row: where it stands, its turn, its tint, its run's part, its
/// rank and its mesh's `top` (model y, before the copy's scale): where a copy
/// that sinks under snow looks up the open sky, above itself.
pub(crate) fn copy_row(
    instance: &render_model::ScatterInstance,
    part: u32,
    top: f32,
) -> [f32; COPY_FLOATS] {
    let [x, y, z] = instance.translation;
    let [r, g, b] = instance.tint;
    let [qx, qy, qz, qw] = instance.rotation;
    [
        x,
        y,
        z,
        instance.scale,
        qx,
        qy,
        qz,
        qw,
        r,
        g,
        b,
        f32::from_bits(part),
        instance.rank,
        top,
        0.0,
        0.0,
    ]
}
