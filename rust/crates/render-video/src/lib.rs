//! Video clips for the Engine's renderers: WebM demuxed once, VP9 decoded to
//! YUV planes as playback advances.
//!
//! The Engine admits WebM with one VP9 profile 0 video track (8-bit, 4:2:0),
//! the format the products ship, and an optional Opus audio track. Anything
//! else fails to open, which the renderer reports as a failed playback.
//! Decoding is pure Rust (`rusty_vp9`, `matroska-demuxer`); no codec library
//! is loaded.

#![forbid(unsafe_code)]

use std::io::Cursor;
use std::sync::Arc;

use matroska_demuxer::{Frame, MatroskaFile, TrackType};

const VP9_CODEC_ID: &str = "V_VP9";
const OPUS_CODEC_ID: &str = "A_OPUS";
const NANOSECONDS_PER_SECOND: f64 = 1e9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoError(pub String);

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for VideoError {}

fn error(detail: impl std::fmt::Display) -> VideoError {
    VideoError(detail.to_string())
}

/// One demuxed clip: its video packets with their presentation times, and
/// its Opus packets if it has sound.
#[derive(Debug)]
pub struct VideoClip {
    pub width: u32,
    pub height: u32,
    /// Seconds from the first frame to the end of the last.
    pub duration: f64,
    packets: Vec<(f64, Vec<u8>)>,
    pub audio: Option<OpusTrack>,
}

/// The clip's Opus soundtrack, for the audio realizer.
#[derive(Debug)]
pub struct OpusTrack {
    pub channels: u32,
    pub sample_rate: u32,
    /// Samples at 48 kHz to drop from the start of the decoded stream.
    pub pre_skip: u64,
    /// Packets with their presentation time in seconds.
    pub packets: Vec<(f64, Vec<u8>)>,
}

impl VideoClip {
    /// Demux a WebM clip. Fails unless it has one VP9 video track.
    pub fn open(bytes: &[u8]) -> Result<Arc<Self>, VideoError> {
        let mut file = MatroskaFile::open(Cursor::new(bytes)).map_err(error)?;
        let scale = file.info().timestamp_scale().get() as f64 / NANOSECONDS_PER_SECOND;
        let tracks = file.tracks();
        let video = tracks
            .iter()
            .find(|track| track.track_type() == TrackType::Video)
            .ok_or_else(|| error("the clip has no video track"))?;
        if video.codec_id() != VP9_CODEC_ID {
            return Err(error(format!(
                "the video track is {}, not VP9",
                video.codec_id()
            )));
        }
        let video_track = video.track_number().get();
        let (width, height) = video
            .video()
            .map(|video| (video.pixel_width().get(), video.pixel_height().get()))
            .ok_or_else(|| error("the video track has no dimensions"))?;
        let frame_duration = video
            .default_duration()
            .map(|nanoseconds| nanoseconds.get() as f64 / NANOSECONDS_PER_SECOND);
        let opus = tracks
            .iter()
            .find(|track| {
                track.track_type() == TrackType::Audio && track.codec_id() == OPUS_CODEC_ID
            })
            .map(|track| {
                let audio = track.audio();
                (
                    track.track_number().get(),
                    OpusTrack {
                        channels: audio.map_or(1, |audio| audio.channels().get() as u32),
                        sample_rate: audio
                            .map_or(48_000, |audio| audio.sampling_frequency() as u32),
                        pre_skip: track.codec_delay().map_or(0, |nanoseconds| {
                            (nanoseconds as f64 * 48_000.0 / NANOSECONDS_PER_SECOND).round() as u64
                        }),
                        packets: Vec::new(),
                    },
                )
            });
        let declared = file.info().duration().map(|duration| duration * scale);
        let (audio_track, mut audio) = match opus {
            Some((number, track)) => (Some(number), Some(track)),
            None => (None, None),
        };
        let mut packets = Vec::new();
        let mut frame = Frame::default();
        while file.next_frame(&mut frame).map_err(error)? {
            let at = frame.timestamp as f64 * scale;
            if frame.track == video_track {
                packets.push((at, std::mem::take(&mut frame.data)));
            } else if Some(frame.track) == audio_track {
                if let Some(audio) = &mut audio {
                    audio.packets.push((at, std::mem::take(&mut frame.data)));
                }
            }
        }
        let first = packets
            .first()
            .map(|(at, _)| *at)
            .ok_or_else(|| error("the clip has no video frames"))?;
        for packet in &mut packets {
            packet.0 -= first;
        }
        let last = packets.last().map_or(0.0, |(at, _)| *at);
        // The last frame shows for one frame interval: the declared one, else
        // the clip's average.
        let interval = frame_duration.unwrap_or_else(|| {
            if packets.len() > 1 {
                last / (packets.len() - 1) as f64
            } else {
                0.0
            }
        });
        let duration = declared.map_or(last + interval, |declared| declared.max(last));
        Ok(Arc::new(Self {
            width: width as u32,
            height: height as u32,
            duration,
            packets,
            audio,
        }))
    }

    pub fn frame_count(&self) -> usize {
        self.packets.len()
    }
}

/// A decoded picture: 8-bit Y, U and V planes, chroma at half resolution.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub planes: [Vec<u8>; 3],
    pub strides: [usize; 3],
    /// Presentation time in seconds from the clip's start.
    pub at: f64,
}

/// One playback of a clip, from its start.
pub struct VideoPlayback {
    clip: Arc<VideoClip>,
    decoder: rusty_vp9::Vp9Decoder,
    next: usize,
    current: Option<VideoFrame>,
    changed: bool,
}

impl VideoPlayback {
    pub fn new(clip: Arc<VideoClip>) -> Self {
        Self {
            clip,
            decoder: rusty_vp9::Vp9Decoder::new(),
            next: 0,
            current: None,
            changed: false,
        }
    }

    pub fn clip(&self) -> &VideoClip {
        &self.clip
    }

    /// Decode up to the frame shown at `position` seconds. Frames between
    /// the previous position and this one are decoded (VP9 frames depend on
    /// their predecessors) but only the last is kept.
    pub fn advance(&mut self, position: f64) -> Result<(), VideoError> {
        while let Some((at, packet)) = self.clip.packets.get(self.next) {
            if *at > position {
                break;
            }
            self.decoder
                .push(packet, Some(self.next as i64))
                .map_err(|e| error(format!("{e:?}")))?;
            self.next += 1;
            loop {
                match self.decoder.next_frame() {
                    Ok(frame) => {
                        if frame.bit_depth != 8
                            || frame.subsampling_x != 1
                            || frame.subsampling_y != 1
                        {
                            return Err(error("only 8-bit 4:2:0 VP9 is admitted"));
                        }
                        let at = frame
                            .pts
                            .and_then(|index| self.clip.packets.get(index as usize))
                            .map_or(*at, |(at, _)| *at);
                        let [y, u, v]: [Vec<u8>; 3] = frame
                            .planes
                            .try_into()
                            .map_err(|_| error("a decoded frame has no three planes"))?;
                        self.current = Some(VideoFrame {
                            width: frame.width,
                            height: frame.height,
                            strides: [frame.strides[0], frame.strides[1], frame.strides[2]],
                            planes: [y, u, v],
                            at,
                        });
                        self.changed = true;
                    }
                    Err(rusty_vp9::Error::Again) | Err(rusty_vp9::Error::Eof) => break,
                    Err(e) => return Err(error(format!("{e:?}"))),
                }
            }
        }
        Ok(())
    }

    /// The frame shown now, and whether it changed since the last call.
    pub fn take_frame(&mut self) -> Option<(&VideoFrame, bool)> {
        let changed = std::mem::take(&mut self.changed);
        self.current.as_ref().map(|frame| (frame, changed))
    }

    pub fn finished(&self, position: f64) -> bool {
        position >= self.clip.duration
    }
}
