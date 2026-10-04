//! A kira backend that mixes in real time into a sink instead of a device.
//!
//! A pacing thread renders whole blocks as wall-clock time makes them due,
//! so voices advance, finish and report completion as they would on a
//! device, and hands each block to the sink: a streaming route sends it to
//! the pages that watch the runtime.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kira::backend::{Backend, Renderer};

/// Frames per mixed block: 10 ms at 48 kHz.
const BLOCK_FRAMES: usize = 480;
const CHANNELS: u16 = 2;
/// How often the pacing thread looks for due blocks.
const TICK: Duration = Duration::from_millis(4);
/// A thread starved for longer than this resumes at the present rather than
/// rendering the backlog in a burst.
const MAX_BACKLOG: Duration = Duration::from_millis(200);

/// Receives each mixed block: interleaved stereo samples in `[-1, 1]`.
pub type AudioBlockSink = Arc<dyn Fn(&[f32]) + Send + Sync>;

pub struct StreamBackendSettings {
    pub sample_rate: u32,
    pub sink: AudioBlockSink,
}

pub struct StreamBackend {
    sample_rate: u32,
    sink: AudioBlockSink,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Backend for StreamBackend {
    type Settings = StreamBackendSettings;
    type Error = std::io::Error;

    fn setup(
        settings: Self::Settings,
        _internal_buffer_size: usize,
    ) -> Result<(Self, u32), Self::Error> {
        Ok((
            Self {
                sample_rate: settings.sample_rate,
                sink: settings.sink,
                running: Arc::new(AtomicBool::new(true)),
                thread: None,
            },
            settings.sample_rate,
        ))
    }

    fn start(&mut self, mut renderer: Renderer) -> Result<(), Self::Error> {
        let (sink, running, rate) = (
            Arc::clone(&self.sink),
            Arc::clone(&self.running),
            u64::from(self.sample_rate),
        );
        let thread = std::thread::Builder::new()
            .name("rusty-audio-stream".to_owned())
            .spawn(move || {
                let mut block = vec![0.0_f32; BLOCK_FRAMES * usize::from(CHANNELS)];
                let started = Instant::now();
                let mut rendered = 0_u64;
                let backlog = MAX_BACKLOG.as_secs_f64() * rate as f64;
                while running.load(Ordering::Acquire) {
                    let due = (started.elapsed().as_secs_f64() * rate as f64) as u64;
                    if (due.saturating_sub(rendered)) as f64 > backlog {
                        rendered = due - BLOCK_FRAMES as u64;
                    }
                    while rendered + BLOCK_FRAMES as u64 <= due {
                        renderer.on_start_processing();
                        renderer.process(&mut block, CHANNELS);
                        sink(&block);
                        rendered += BLOCK_FRAMES as u64;
                    }
                    std::thread::sleep(TICK);
                }
            })?;
        self.thread = Some(thread);
        Ok(())
    }
}

impl Drop for StreamBackend {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
