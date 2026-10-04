//! The streamed-audio route: `GET /__rusty/product/runtime/audio`.
//!
//! When the runtime's audio output is `stream`, it mixes its committed audio
//! in real time and publishes each mixed block here instead of to a device.
//! A watching page opens one long-lived request and plays what arrives: raw
//! interleaved stereo signed 16-bit little-endian PCM at
//! [`PRODUCT_HOST_AUDIO_SAMPLE_RATE`], with no header or framing
//! (`audio/L16; rate=48000; channels=2`, little-endian). A response starts at
//! the next published block, so a page hears the present, not a backlog.
//!
//! The stream keeps only a few recent blocks. A viewer that falls behind
//! skips to the newest of them rather than queueing sound in socket buffers,
//! as a slow frame viewer skips frames. Every watching page receives the same
//! mix.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

pub const PRODUCT_HOST_AUDIO_PATH: &str = "/__rusty/product/runtime/audio";
pub const PRODUCT_HOST_AUDIO_SAMPLE_RATE: u32 = 48_000;
pub const PRODUCT_HOST_AUDIO_CHANNELS: u16 = 2;
pub(crate) const PRODUCT_HOST_AUDIO_CONTENT_TYPE: &str =
    "audio/L16; rate=48000; channels=2; endianness=little-endian";

/// Blocks kept for a viewer that is momentarily behind; at 10 ms blocks this
/// is a fifth of a second.
const RETAINED_BLOCKS: usize = 20;

/// The most recent mixed blocks and the sequence of the next one.
#[derive(Default)]
pub struct ProductHostAudioStream {
    state: Mutex<AudioState>,
    published: Condvar,
}

#[derive(Default)]
struct AudioState {
    /// Sequence of the next block published; the oldest retained block is
    /// `next - blocks.len()`.
    next: u64,
    blocks: VecDeque<Arc<[u8]>>,
}

impl ProductHostAudioStream {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    /// Publishes one block of interleaved stereo samples in `[-1, 1]`.
    pub fn publish(&self, samples: &[f32]) {
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let mut state = self.state();
        if state.blocks.len() == RETAINED_BLOCKS {
            state.blocks.pop_front();
        }
        state.blocks.push_back(bytes.into());
        state.next += 1;
        drop(state);
        self.published.notify_all();
    }

    /// The sequence a new viewer starts from: the next block published.
    pub(crate) fn start(&self) -> u64 {
        self.state().next
    }

    /// The blocks from `from` on, waiting up to `timeout` for one, and the
    /// sequence to ask from next. A viewer behind the retained blocks skips
    /// to the oldest one kept.
    pub(crate) fn blocks_from(&self, from: u64, timeout: Duration) -> (Vec<Arc<[u8]>>, u64) {
        let state = self.state();
        let (state, _) = self
            .published
            .wait_timeout_while(state, timeout, |state| state.next <= from)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let oldest = state.next - state.blocks.len() as u64;
        let first = from.max(oldest);
        let skip = (first - oldest) as usize;
        (
            state.blocks.iter().skip(skip).cloned().collect(),
            state.next,
        )
    }

    fn state(&self) -> MutexGuard<'_, AudioState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_viewer_starts_at_the_next_block_and_a_slow_one_skips_ahead() {
        let stream = ProductHostAudioStream::new();
        stream.publish(&[0.5, -0.5]);
        let from = stream.start();
        stream.publish(&[1.0, -1.0, 2.0, 0.0]);
        let (blocks, next) = stream.blocks_from(from, Duration::ZERO);
        assert_eq!(next, from + 1);
        assert_eq!(
            &*blocks[0],
            &[0xff, 0x7f, 0x01, 0x80, 0xff, 0x7f, 0x00, 0x00][..]
        );
        for _ in 0..RETAINED_BLOCKS + 5 {
            stream.publish(&[0.0, 0.0]);
        }
        let (blocks, next) = stream.blocks_from(from, Duration::ZERO);
        assert_eq!(blocks.len(), RETAINED_BLOCKS);
        assert_eq!(next, from + 1 + RETAINED_BLOCKS as u64 + 5);
        let (blocks, _) = stream.blocks_from(next, Duration::from_millis(1));
        assert!(blocks.is_empty());
    }
}
