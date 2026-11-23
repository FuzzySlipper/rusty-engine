//! A kira backend that mixes only when asked, for measuring exactly what the
//! realizer plays: levels, filtering and reverb, with no device and no
//! wall-clock pacing.

use kira::backend::{Backend, Renderer};

/// Frames per mixed block, as the stream mixes them.
const BLOCK_FRAMES: usize = 480;
const CHANNELS: u16 = 2;

pub struct OfflineBackendSettings {
    pub sample_rate: u32,
}

pub struct OfflineBackend {
    renderer: Option<Renderer>,
}

impl Backend for OfflineBackend {
    type Settings = OfflineBackendSettings;
    type Error = std::convert::Infallible;

    fn setup(
        settings: Self::Settings,
        _internal_buffer_size: usize,
    ) -> Result<(Self, u32), Self::Error> {
        Ok((Self { renderer: None }, settings.sample_rate))
    }

    fn start(&mut self, renderer: Renderer) -> Result<(), Self::Error> {
        self.renderer = Some(renderer);
        Ok(())
    }
}

impl OfflineBackend {
    /// Mixes the next `frames` frames (rounded up to whole blocks) as
    /// interleaved stereo samples.
    pub fn render(&mut self, frames: usize) -> Vec<f32> {
        let blocks = frames.div_ceil(BLOCK_FRAMES);
        let mut out = vec![0.0_f32; blocks * BLOCK_FRAMES * usize::from(CHANNELS)];
        if let Some(renderer) = self.renderer.as_mut() {
            for block in out.chunks_mut(BLOCK_FRAMES * usize::from(CHANNELS)) {
                renderer.on_start_processing();
                renderer.process(block, CHANNELS);
            }
        }
        out
    }
}
