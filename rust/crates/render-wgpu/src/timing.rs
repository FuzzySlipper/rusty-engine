//! GPU time of one pass through the device's timestamp queries, so every
//! pass the renderer times reports its cost the same way (`GpuReadout`).
//!
//! A timer writes a begin and an end stamp on its pass, resolves them into a
//! mapped buffer after the pass, and reads them on a later frame without
//! waiting for the GPU: one readback in flight at a time, so a frame whose
//! previous readback is still mapping goes untimed. The readout is the
//! median over the recent timed frames. Without `TIMESTAMP_QUERY` there is
//! no timer and the pass runs untimed.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, TryRecvError};

use crate::Gpu;

/// Begin and end of the pass.
const STAMPS: u32 = 2;
const STAMP_BYTES: u64 = 8 * STAMPS as u64;
/// Timed frames the median is taken over.
const WINDOW: usize = 120;

/// The GPU time of one timed pass over the recent frames.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuPassTiming {
    pub pass: &'static str,
    /// Recent frames whose GPU time was read back.
    pub timed_frames: usize,
    /// Median GPU milliseconds of the pass over those frames; 0 with none.
    pub median_gpu_ms: f64,
}

pub(crate) struct PassTimer {
    pass: &'static str,
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    /// Nanoseconds per timestamp tick.
    period: f32,
    /// The readback of a timed frame is mapping; its result arrives here.
    pending: Option<Receiver<Result<(), wgpu::BufferAsyncError>>>,
    /// This frame's stamps were resolved and wait for the submit.
    resolved: bool,
    recent: VecDeque<f64>,
}

impl PassTimer {
    /// A timer for `pass`, or `None` on a device without timestamp queries.
    pub fn new(gpu: &Gpu, pass: &'static str) -> Option<Self> {
        let device = &gpu.device;
        device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| Self {
                pass,
                queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some(pass),
                    ty: wgpu::QueryType::Timestamp,
                    count: STAMPS,
                }),
                resolve: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(pass),
                    size: STAMP_BYTES,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(pass),
                    size: STAMP_BYTES,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                period: gpu.queue.get_timestamp_period(),
                pending: None,
                resolved: false,
                recent: VecDeque::with_capacity(WINDOW),
            })
    }

    /// Take the time of the pending timed frame if its readback has mapped,
    /// without waiting. Call once per frame before the pass.
    pub fn collect(&mut self, gpu: &Gpu) {
        let Some(receiver) = self.pending.as_ref() else {
            return;
        };
        // Mapping completes in a poll; this one never blocks.
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        match receiver.try_recv() {
            Err(TryRecvError::Empty) => return,
            Ok(Ok(())) => {
                if let Ok(mapped) = self.readback.slice(..).get_mapped_range() {
                    let stamp = |index: usize| {
                        u64::from_le_bytes(mapped[index * 8..index * 8 + 8].try_into().unwrap())
                    };
                    let ticks = stamp(1).saturating_sub(stamp(0));
                    let ms = ticks as f64 * f64::from(self.period) / 1_000_000.0;
                    if self.recent.len() == WINDOW {
                        self.recent.pop_front();
                    }
                    self.recent.push_back(ms);
                }
                self.readback.unmap();
            }
            // A failed or abandoned map leaves nothing mapped; the next
            // frame times again.
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {}
        }
        self.pending = None;
    }

    /// Whether this frame's pass is timed: no earlier readback is mapping.
    fn armed(&self) -> bool {
        self.pending.is_none()
    }

    pub fn render_writes(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.render_writes_between(true, true)
    }

    /// Stamps for a stage of several render passes: `begin` on its first
    /// pass, `end` on its last; none for a pass between them.
    pub fn render_writes_between(
        &self,
        begin: bool,
        end: bool,
    ) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        (self.armed() && (begin || end)).then_some(wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: begin.then_some(0),
            end_of_pass_write_index: end.then_some(1),
        })
    }

    pub fn compute_writes(&self) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        self.compute_writes_between(true, true)
    }

    /// Stamps for a stage of several compute passes: `begin` on its first
    /// pass, `end` on its last; none for a pass between them.
    pub fn compute_writes_between(
        &self,
        begin: bool,
        end: bool,
    ) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        (self.armed() && (begin || end)).then_some(wgpu::ComputePassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: begin.then_some(0),
            end_of_pass_write_index: end.then_some(1),
        })
    }

    /// After the pass, in the same encoder: bring the stamps to the
    /// readback buffer.
    pub fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if self.armed() {
            encoder.resolve_query_set(&self.queries, 0..STAMPS, &self.resolve, 0);
            encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, None);
            self.resolved = true;
        }
    }

    /// After the encoder was submitted: start reading the stamps back.
    pub fn submitted(&mut self) {
        if !std::mem::take(&mut self.resolved) {
            return;
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.pending = Some(receiver);
    }

    /// Forget the recent frames, as when the pass changes what it does.
    pub fn restart(&mut self) {
        self.recent.clear();
    }

    pub fn readout(&self) -> GpuPassTiming {
        let mut values: Vec<f64> = self.recent.iter().copied().collect();
        values.sort_by(f64::total_cmp);
        GpuPassTiming {
            pass: self.pass,
            timed_frames: values.len(),
            median_gpu_ms: values.get(values.len() / 2).copied().unwrap_or(0.0),
        }
    }
}

/// The readout of a pass on a device without timestamp queries.
pub(crate) fn untimed(pass: &'static str) -> GpuPassTiming {
    GpuPassTiming {
        pass,
        timed_frames: 0,
        median_gpu_ms: 0.0,
    }
}
