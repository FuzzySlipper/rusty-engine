use serde::{Deserialize, Serialize};

use crate::{AdmittedVoxelObject, VoxelObjectRuntimeClip};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelObjectLoopMode {
    Once,
    Repeat,
    PingPong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectPlaybackRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl VoxelObjectPlaybackRate {
    pub const NORMAL: Self = Self {
        numerator: 1,
        denominator: 1,
    };

    pub fn new(numerator: u32, denominator: u32) -> Result<Self, VoxelObjectPlayerError> {
        if numerator == 0 || denominator == 0 {
            return Err(VoxelObjectPlayerError::InvalidRate);
        }
        let divisor = greatest_common_divisor(numerator, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }
}

impl Default for VoxelObjectPlaybackRate {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoxelObjectPlaybackStatus {
    Stopped,
    Playing,
    Paused,
}

/// Caller-owned playback posture. A caller may persist it or keep it transient;
/// no host or renderer clock is stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VoxelObjectPlaybackPosture {
    pub status: VoxelObjectPlaybackStatus,
    pub clip: Option<String>,
    pub loop_mode: VoxelObjectLoopMode,
    pub rate: VoxelObjectPlaybackRate,
    pub elapsed_micros: u64,
}

impl Default for VoxelObjectPlaybackPosture {
    fn default() -> Self {
        Self {
            status: VoxelObjectPlaybackStatus::Stopped,
            clip: None,
            loop_mode: VoxelObjectLoopMode::Once,
            rate: VoxelObjectPlaybackRate::NORMAL,
            elapsed_micros: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelObjectPlaybackReadout<'a> {
    pub status: VoxelObjectPlaybackStatus,
    pub clip: Option<&'a str>,
    pub loop_mode: VoxelObjectLoopMode,
    pub rate: VoxelObjectPlaybackRate,
    pub elapsed_micros: u64,
    pub frame: u32,
    pub clip_frame: Option<u32>,
    pub ended: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoxelObjectPlayerError {
    UnknownClip {
        clip: String,
    },
    ClipFrameOutOfRange {
        clip: String,
        frame: u32,
        frame_count: u32,
    },
    InvalidRate,
    InvalidPosture,
    NotPlaying,
    NotPaused,
    TimeMovedBackwards {
        previous: u64,
        current: u64,
    },
    TimeOverflow,
}

impl std::fmt::Display for VoxelObjectPlayerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownClip { clip } => write!(formatter, "unknown voxel-object clip `{clip}`"),
            Self::ClipFrameOutOfRange {
                clip,
                frame,
                frame_count,
            } => write!(
                formatter,
                "voxel-object clip `{clip}` frame {frame} is outside 0..{frame_count}"
            ),
            Self::InvalidRate => write!(formatter, "playback rate terms must be non-zero"),
            Self::InvalidPosture => {
                write!(formatter, "playback posture is internally inconsistent")
            }
            Self::NotPlaying => write!(formatter, "voxel-object player is not playing"),
            Self::NotPaused => write!(formatter, "voxel-object player is not paused"),
            Self::TimeMovedBackwards { previous, current } => write!(
                formatter,
                "explicit playback time moved backwards from {previous} to {current} microseconds"
            ),
            Self::TimeOverflow => write!(formatter, "voxel-object playback time overflowed"),
        }
    }
}

impl std::error::Error for VoxelObjectPlayerError {}

#[derive(Debug, Clone)]
pub struct VoxelObjectPlayer {
    posture: VoxelObjectPlaybackPosture,
    playing_anchor_micros: Option<u64>,
}

impl Default for VoxelObjectPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxelObjectPlayer {
    pub fn new() -> Self {
        Self {
            posture: VoxelObjectPlaybackPosture::default(),
            playing_anchor_micros: None,
        }
    }

    pub fn restore(
        object: &AdmittedVoxelObject,
        posture: VoxelObjectPlaybackPosture,
        now_micros: u64,
    ) -> Result<Self, VoxelObjectPlayerError> {
        validate_posture(object, &posture)?;
        let playing_anchor_micros =
            (posture.status == VoxelObjectPlaybackStatus::Playing).then_some(now_micros);
        Ok(Self {
            posture,
            playing_anchor_micros,
        })
    }

    pub fn play(
        &mut self,
        object: &AdmittedVoxelObject,
        clip: &str,
        loop_mode: VoxelObjectLoopMode,
        rate: VoxelObjectPlaybackRate,
        now_micros: u64,
    ) -> Result<(), VoxelObjectPlayerError> {
        require_clip(object, clip)?;
        if rate.numerator == 0 || rate.denominator == 0 {
            return Err(VoxelObjectPlayerError::InvalidRate);
        }
        self.posture = VoxelObjectPlaybackPosture {
            status: VoxelObjectPlaybackStatus::Playing,
            clip: Some(clip.to_string()),
            loop_mode,
            rate,
            elapsed_micros: 0,
        };
        self.playing_anchor_micros = Some(now_micros);
        Ok(())
    }

    /// Selects an exact stored clip frame as a paused, normal-rate posture.
    ///
    /// This is the host-neutral scrub operation. The caller chooses the clip,
    /// frame, and loop policy while the runtime derives the canonical elapsed
    /// time from admitted frame durations. A later [`Self::resume`] continues
    /// from the selected pose using the caller's explicit clock.
    pub fn scrub(
        &mut self,
        object: &AdmittedVoxelObject,
        clip: &str,
        clip_frame: u32,
        loop_mode: VoxelObjectLoopMode,
    ) -> Result<(), VoxelObjectPlayerError> {
        let clip_readout = require_clip(object, clip)?;
        let frame_count = clip_readout.frame_indices.len() as u32;
        if clip_frame >= frame_count {
            return Err(VoxelObjectPlayerError::ClipFrameOutOfRange {
                clip: clip.to_string(),
                frame: clip_frame,
                frame_count,
            });
        }
        let elapsed_micros = clip_readout.frame_durations_micros[..clip_frame as usize]
            .iter()
            .try_fold(0_u64, |elapsed, duration| elapsed.checked_add(*duration))
            .ok_or(VoxelObjectPlayerError::TimeOverflow)?;
        self.posture = VoxelObjectPlaybackPosture {
            status: VoxelObjectPlaybackStatus::Paused,
            clip: Some(clip.to_string()),
            loop_mode,
            rate: VoxelObjectPlaybackRate::NORMAL,
            elapsed_micros,
        };
        self.playing_anchor_micros = None;
        Ok(())
    }

    pub fn pause(&mut self, now_micros: u64) -> Result<(), VoxelObjectPlayerError> {
        if self.posture.status != VoxelObjectPlaybackStatus::Playing {
            return Err(VoxelObjectPlayerError::NotPlaying);
        }
        self.posture.elapsed_micros = self.elapsed_at(now_micros)?;
        self.posture.status = VoxelObjectPlaybackStatus::Paused;
        self.playing_anchor_micros = None;
        Ok(())
    }

    pub fn resume(&mut self, now_micros: u64) -> Result<(), VoxelObjectPlayerError> {
        if self.posture.status != VoxelObjectPlaybackStatus::Paused {
            return Err(VoxelObjectPlayerError::NotPaused);
        }
        self.posture.status = VoxelObjectPlaybackStatus::Playing;
        self.playing_anchor_micros = Some(now_micros);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.posture = VoxelObjectPlaybackPosture::default();
        self.playing_anchor_micros = None;
    }

    pub fn posture_at(
        &self,
        now_micros: u64,
    ) -> Result<VoxelObjectPlaybackPosture, VoxelObjectPlayerError> {
        let mut posture = self.posture.clone();
        posture.elapsed_micros = self.elapsed_at(now_micros)?;
        Ok(posture)
    }

    pub fn sample_at<'a>(
        &'a self,
        object: &'a AdmittedVoxelObject,
        now_micros: u64,
    ) -> Result<VoxelObjectPlaybackReadout<'a>, VoxelObjectPlayerError> {
        validate_posture(object, &self.posture)?;
        let elapsed_micros = self.elapsed_at(now_micros)?;
        let Some(clip_id) = self.posture.clip.as_deref() else {
            return Ok(VoxelObjectPlaybackReadout {
                status: self.posture.status,
                clip: None,
                loop_mode: self.posture.loop_mode,
                rate: self.posture.rate,
                elapsed_micros,
                frame: 0,
                clip_frame: None,
                ended: false,
            });
        };
        let clip = require_clip(object, clip_id)?;
        let scaled_elapsed = scale_elapsed(elapsed_micros, self.posture.rate)?;
        let (clip_frame, ended) = select_frame(clip, scaled_elapsed, self.posture.loop_mode);
        Ok(VoxelObjectPlaybackReadout {
            status: self.posture.status,
            clip: Some(clip_id),
            loop_mode: self.posture.loop_mode,
            rate: self.posture.rate,
            elapsed_micros,
            frame: clip.frame_indices[clip_frame as usize],
            clip_frame: Some(clip_frame),
            ended,
        })
    }

    fn elapsed_at(&self, now_micros: u64) -> Result<u64, VoxelObjectPlayerError> {
        let Some(anchor) = self.playing_anchor_micros else {
            return Ok(self.posture.elapsed_micros);
        };
        let delta =
            now_micros
                .checked_sub(anchor)
                .ok_or(VoxelObjectPlayerError::TimeMovedBackwards {
                    previous: anchor,
                    current: now_micros,
                })?;
        self.posture
            .elapsed_micros
            .checked_add(delta)
            .ok_or(VoxelObjectPlayerError::TimeOverflow)
    }
}

fn validate_posture(
    object: &AdmittedVoxelObject,
    posture: &VoxelObjectPlaybackPosture,
) -> Result<(), VoxelObjectPlayerError> {
    if posture.rate.numerator == 0 || posture.rate.denominator == 0 {
        return Err(VoxelObjectPlayerError::InvalidRate);
    }
    match (posture.status, posture.clip.as_deref()) {
        (VoxelObjectPlaybackStatus::Stopped, None) => Ok(()),
        (VoxelObjectPlaybackStatus::Playing | VoxelObjectPlaybackStatus::Paused, Some(clip)) => {
            require_clip(object, clip).map(|_| ())
        }
        _ => Err(VoxelObjectPlayerError::InvalidPosture),
    }
}

fn require_clip<'a>(
    object: &'a AdmittedVoxelObject,
    clip: &str,
) -> Result<&'a VoxelObjectRuntimeClip, VoxelObjectPlayerError> {
    object
        .clip(clip)
        .ok_or_else(|| VoxelObjectPlayerError::UnknownClip {
            clip: clip.to_string(),
        })
}

fn scale_elapsed(
    elapsed_micros: u64,
    rate: VoxelObjectPlaybackRate,
) -> Result<u64, VoxelObjectPlayerError> {
    let scaled =
        u128::from(elapsed_micros) * u128::from(rate.numerator) / u128::from(rate.denominator);
    u64::try_from(scaled).map_err(|_| VoxelObjectPlayerError::TimeOverflow)
}

fn select_frame(
    clip: &VoxelObjectRuntimeClip,
    elapsed_micros: u64,
    loop_mode: VoxelObjectLoopMode,
) -> (u32, bool) {
    let frame_count = clip.frame_indices.len();
    if frame_count == 1 {
        return (
            0,
            loop_mode == VoxelObjectLoopMode::Once && elapsed_micros >= clip.duration_micros,
        );
    }
    match loop_mode {
        VoxelObjectLoopMode::Once => {
            if elapsed_micros >= clip.duration_micros {
                return (frame_count as u32 - 1, true);
            }
            (locate_forward(clip, elapsed_micros), false)
        }
        VoxelObjectLoopMode::Repeat => (
            locate_forward(clip, elapsed_micros % clip.duration_micros),
            false,
        ),
        VoxelObjectLoopMode::PingPong => {
            let reverse_duration = clip.frame_durations_micros[1..frame_count - 1]
                .iter()
                .copied()
                .sum::<u64>();
            let cycle = clip.duration_micros + reverse_duration;
            let offset = elapsed_micros % cycle;
            if offset < clip.duration_micros {
                return (locate_forward(clip, offset), false);
            }
            let mut reverse_offset = offset - clip.duration_micros;
            for frame in (1..frame_count - 1).rev() {
                let duration = clip.frame_durations_micros[frame];
                if reverse_offset < duration {
                    return (frame as u32, false);
                }
                reverse_offset -= duration;
            }
            (0, false)
        }
    }
}

fn locate_forward(clip: &VoxelObjectRuntimeClip, mut offset: u64) -> u32 {
    for (index, duration) in clip.frame_durations_micros.iter().copied().enumerate() {
        if offset < duration {
            return index as u32;
        }
        offset -= duration;
    }
    clip.frame_indices.len() as u32 - 1
}

const fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    left
}
