//! Video soundtracks: the Opus track of a WebM clip, demuxed by
//! `render-video` with the picture and decoded here by `opus-decoder`, as
//! the browser's video element played its own sound.
//!
//! WebM carries no Ogg granule positions, so the stream length comes from
//! the packets' TOC bytes (RFC 6716 §3.1) less the track's codec delay.

use std::sync::Arc;

use kira::sound::streaming::Decoder;
use kira::sound::FromFileError;
use kira::Frame;
use opus_decoder::OpusDecoder;
use render_video::VideoClip;

const OPUS_SAMPLE_RATE: u32 = 48_000;
/// RFC 7845 §4.6: decode at least 80 ms before a seek target.
const SEEK_PREROLL_FRAMES: usize = 3_840;

pub(crate) struct SoundtrackDecoder {
    clip: Arc<VideoClip>,
    decoder: OpusDecoder,
    channels: usize,
    /// Frame offset of each packet's first sample, codec delay included.
    starts: Vec<usize>,
    delay: usize,
    num_frames: usize,
    next: usize,
    pcm: Vec<f32>,
    /// Stream frame (codec delay included) of the next frame to output.
    keep_from: usize,
}

impl SoundtrackDecoder {
    pub(crate) fn new(clip: Arc<VideoClip>) -> Result<Self, FromFileError> {
        let track = clip.audio.as_ref().ok_or(FromFileError::NoDefaultTrack)?;
        let channels = usize::try_from(track.channels)
            .ok()
            .filter(|count| (1..=2).contains(count))
            .ok_or(FromFileError::UnsupportedChannelConfiguration)?;
        let mut starts = Vec::with_capacity(track.packets.len());
        let mut total = 0;
        for (_, packet) in &track.packets {
            starts.push(total);
            total += packet_frames(packet).ok_or_else(|| opus_error("packet has no TOC byte"))?;
        }
        let delay = usize::try_from(track.pre_skip).unwrap_or(usize::MAX);
        let decoder = OpusDecoder::new(OPUS_SAMPLE_RATE, channels)
            .map_err(|error| opus_error(&error.to_string()))?;
        let pcm = vec![0.0; decoder.max_frame_size_per_channel() * channels];
        Ok(Self {
            clip,
            decoder,
            channels,
            starts,
            num_frames: total.saturating_sub(delay),
            delay,
            next: 0,
            pcm,
            keep_from: delay,
        })
    }
}

impl Decoder for SoundtrackDecoder {
    type Error = FromFileError;

    fn sample_rate(&self) -> u32 {
        OPUS_SAMPLE_RATE
    }

    fn num_frames(&self) -> usize {
        self.num_frames
    }

    fn decode(&mut self) -> Result<Vec<Frame>, Self::Error> {
        let track = self
            .clip
            .audio
            .as_ref()
            .ok_or(FromFileError::NoDefaultTrack)?;
        loop {
            let Some((_, packet)) = track.packets.get(self.next) else {
                return Ok(Vec::new());
            };
            let first = self.starts[self.next];
            self.next += 1;
            let decoded = self
                .decoder
                .decode_float(packet, &mut self.pcm, false)
                .map_err(|error| opus_error(&error.to_string()))?;
            let end = decoded.min((self.delay + self.num_frames).saturating_sub(first));
            let start = self.keep_from.saturating_sub(first).min(end);
            if start == end {
                continue;
            }
            let samples = &self.pcm[start * self.channels..end * self.channels];
            return Ok(if self.channels == 1 {
                samples
                    .iter()
                    .map(|&sample| Frame::from_mono(sample))
                    .collect()
            } else {
                samples
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| Frame::new(pair[0], pair[1]))
                    .collect()
            });
        }
    }

    fn seek(&mut self, index: usize) -> Result<usize, Self::Error> {
        let index = index.min(self.num_frames);
        let target = self.delay + index;
        let from = target.saturating_sub(SEEK_PREROLL_FRAMES);
        self.next = self
            .starts
            .partition_point(|start| *start <= from)
            .saturating_sub(1);
        self.decoder.reset();
        self.keep_from = target;
        Ok(index)
    }
}

/// Samples at 48 kHz in one Opus packet, from its TOC byte.
fn packet_frames(packet: &[u8]) -> Option<usize> {
    let toc = *packet.first()?;
    let config = toc >> 3;
    // Frame sizes in 48 kHz samples: SILK, hybrid, then CELT configurations.
    let frame = match config {
        0..=11 => [480, 960, 1_920, 2_880][usize::from(config % 4)],
        12..=15 => [480, 960][usize::from(config % 2)],
        _ => [120, 240, 480, 960][usize::from(config % 4)],
    };
    let count = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => usize::from(*packet.get(1)? & 0x3F),
    };
    Some(frame * count)
}

fn opus_error(detail: &str) -> FromFileError {
    FromFileError::IoError(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("opus: {detail}"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `render-video`'s fixture: 1.5 s of a 440 Hz mono tone in WebM Opus.
    const CLIP: &[u8] = include_bytes!("../../render-video/tests/fixtures/testsrc.webm");

    fn decode_all(decoder: &mut SoundtrackDecoder) -> Vec<f32> {
        let mut samples = Vec::new();
        loop {
            let frames = decoder.decode().expect("decodes");
            if frames.is_empty() {
                return samples;
            }
            samples.extend(frames.iter().map(|frame| frame.left));
        }
    }

    #[test]
    fn the_soundtrack_decodes_its_tone_without_the_codec_delay() {
        let clip = VideoClip::open(CLIP).expect("opens");
        let mut decoder = SoundtrackDecoder::new(clip).expect("has a soundtrack");
        let samples = decode_all(&mut decoder);
        assert_eq!(samples.len(), decoder.num_frames());
        // 1.5 s of tone; the encoder pads the last packet.
        assert!(
            (72_000..=72_000 + 960).contains(&samples.len()),
            "{}",
            samples.len()
        );
        // A 440 Hz sine crosses zero 880 times a second.
        let crossings = samples[4_800..52_800]
            .windows(2)
            .filter(|pair| (pair[0] < 0.0) != (pair[1] < 0.0))
            .count();
        assert!(
            (870..=890).contains(&crossings),
            "{crossings} crossings in 1 s"
        );
    }

    #[test]
    fn a_seek_continues_the_continuous_decode() {
        let clip = VideoClip::open(CLIP).expect("opens");
        let mut decoder = SoundtrackDecoder::new(clip).expect("has a soundtrack");
        let continuous = decode_all(&mut decoder);
        assert_eq!(decoder.seek(24_000).expect("seeks"), 24_000);
        let mut resumed = Vec::new();
        while resumed.len() < 4_800 {
            resumed.extend(
                decoder
                    .decode()
                    .expect("decodes")
                    .iter()
                    .map(|frame| frame.left),
            );
        }
        let worst = resumed[..4_800]
            .iter()
            .zip(&continuous[24_000..28_800])
            .map(|(left, right)| (left - right).abs())
            .fold(0.0_f32, f32::max);
        assert!(worst < 0.01, "seek diverges by {worst}");
    }
}
