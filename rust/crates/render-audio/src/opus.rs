//! Ogg Opus streaming for kira. symphonia demuxes the Ogg stream, trims end
//! padding and seeks; `opus-decoder` (pure Rust) decodes the packets, which
//! symphonia has no codec for. The header's pre-skip is dropped here:
//! symphonia reports it as the track delay and leaves it in the packets.

use std::io::Cursor;
use std::sync::Arc;

use kira::sound::streaming::Decoder;
use kira::sound::FromFileError;
use kira::Frame;
use opus_decoder::OpusDecoder;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::{FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::units::Timestamp;
use symphonia::default::formats::OggReader;

/// Opus always decodes at 48 kHz.
const OPUS_SAMPLE_RATE: u32 = 48_000;
/// RFC 7845 §4.6: decode at least 80 ms before a seek target so the decoder
/// state has converged by the first kept sample.
const SEEK_PREROLL_FRAMES: i64 = 3_840;

pub(crate) struct OggOpusDecoder {
    reader: OggReader<'static>,
    decoder: OpusDecoder,
    track_id: u32,
    channels: usize,
    /// Pre-skip frames at the start of the stream, never output.
    delay: i64,
    num_frames: usize,
    pcm: Vec<f32>,
    /// Stream timestamp of the next frame to output. Earlier frames are
    /// pre-skip or seek pre-roll.
    keep_from: i64,
}

impl OggOpusDecoder {
    pub(crate) fn new(bytes: Arc<[u8]>) -> Result<Self, FromFileError> {
        let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
        let reader = OggReader::try_new(stream, Default::default())?;
        let track = reader
            .default_track(TrackType::Audio)
            .ok_or(FromFileError::NoDefaultTrack)?;
        let Some(CodecParameters::Audio(params)) = track.codec_params.as_ref() else {
            return Err(FromFileError::NoDefaultTrack);
        };
        let channels = params
            .channels
            .as_ref()
            .map(|channels| channels.count())
            .filter(|count| (1..=2).contains(count))
            .ok_or(FromFileError::UnsupportedChannelConfiguration)?;
        let delay = u64::from(track.delay.unwrap_or(0));
        let num_frames = track
            .num_frames
            .ok_or(FromFileError::UnknownDuration)?
            .saturating_sub(delay)
            .try_into()
            .map_err(|_| FromFileError::UnknownDuration)?;
        let delay = i64::try_from(delay).map_err(|_| FromFileError::UnknownDuration)?;
        let track_id = track.id;
        let decoder = OpusDecoder::new(OPUS_SAMPLE_RATE, channels).map_err(opus_error)?;
        let pcm = vec![0.0; decoder.max_frame_size_per_channel() * channels];
        Ok(Self {
            reader,
            decoder,
            track_id,
            channels,
            delay,
            num_frames,
            pcm,
            keep_from: delay,
        })
    }
}

impl Decoder for OggOpusDecoder {
    type Error = FromFileError;

    fn sample_rate(&self) -> u32 {
        OPUS_SAMPLE_RATE
    }

    fn num_frames(&self) -> usize {
        self.num_frames
    }

    fn decode(&mut self) -> Result<Vec<Frame>, Self::Error> {
        loop {
            let Some(packet) = self.reader.next_packet()? else {
                return Ok(Vec::new());
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let decoded = self
                .decoder
                .decode_float(&packet.data, &mut self.pcm, false)
                .map_err(opus_error)?;
            let trim_start = to_frames(packet.trim_start.get());
            let trim_end = to_frames(packet.trim_end.get());
            let end = decoded.saturating_sub(trim_end);
            let mut start = trim_start.min(end);
            let first = packet.pts.get() + start as i64;
            let skip = usize::try_from(self.keep_from - first).unwrap_or(0);
            start = (start + skip).min(end);
            if start == end {
                continue;
            }
            let samples = &self.pcm[start * self.channels..end * self.channels];
            let frames = if self.channels == 1 {
                samples.iter().copied().map(Frame::from_mono).collect()
            } else {
                samples
                    .chunks_exact(2)
                    .map(|pair| Frame::new(pair[0], pair[1]))
                    .collect()
            };
            return Ok(frames);
        }
    }

    fn seek(&mut self, index: usize) -> Result<usize, Self::Error> {
        let index = index.min(self.num_frames);
        let target = self.delay + i64::try_from(index).unwrap_or(i64::MAX);
        self.reader.seek(
            SeekMode::Accurate,
            SeekTo::Timestamp {
                ts: Timestamp::new((target - SEEK_PREROLL_FRAMES).max(0)),
                track_id: self.track_id,
            },
        )?;
        self.decoder.reset();
        self.keep_from = target;
        Ok(index)
    }
}

fn to_frames(duration: u64) -> usize {
    usize::try_from(duration).unwrap_or(usize::MAX)
}

fn opus_error(error: opus_decoder::OpusError) -> FromFileError {
    FromFileError::IoError(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("opus: {error}"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TONE: &[u8] = include_bytes!("../../../../fixtures/audio-containers/tone.opus");

    fn decode_all(decoder: &mut OggOpusDecoder) -> Vec<f32> {
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
    fn pre_skip_and_padding_are_trimmed() {
        let mut decoder = OggOpusDecoder::new(Arc::from(TONE)).expect("opens");
        // One second at 48 kHz, as libopus (ffmpeg) decodes this fixture.
        assert_eq!(decoder.num_frames(), 48_000);
        assert_eq!(decode_all(&mut decoder).len(), 48_000);
    }

    #[test]
    fn a_seek_continues_the_continuous_decode() {
        let mut decoder = OggOpusDecoder::new(Arc::from(TONE)).expect("opens");
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
