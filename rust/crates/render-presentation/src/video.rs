use serde::{Deserialize, Serialize};

use crate::{PresentationFrameDiff, PresentationOp, PresentationOpMeta};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VideoPlaybackHandle(u64);

impl VideoPlaybackHandle {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoClipRef {
    pub asset: String,
    pub content_hash: String,
    pub media_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum VideoProjectionOp {
    Play {
        handle: VideoPlaybackHandle,
        clip: VideoClipRef,
    },
    Stop {
        handle: VideoPlaybackHandle,
    },
    Skip {
        handle: VideoPlaybackHandle,
    },
}

#[derive(Debug, Clone, Default)]
pub struct VideoProjector {
    active: Option<(VideoPlaybackHandle, VideoClipRef)>,
}

impl VideoProjector {
    pub fn project(
        &mut self,
        meta: PresentationOpMeta,
        op: VideoProjectionOp,
    ) -> Result<PresentationOp, &'static str> {
        let op = match op {
            VideoProjectionOp::Play { handle, clip } => self.play(handle, clip)?,
            VideoProjectionOp::Stop { handle } => self
                .stop(handle)
                .ok_or("video stop does not name the active playback")?,
            VideoProjectionOp::Skip { handle } => self
                .skip(handle)
                .ok_or("video skip does not name the active playback")?,
        };
        Ok(PresentationOp::Video { meta, op })
    }

    pub fn play(
        &mut self,
        handle: VideoPlaybackHandle,
        clip: VideoClipRef,
    ) -> Result<VideoProjectionOp, &'static str> {
        if handle.raw() == 0
            || clip.asset.is_empty()
            || clip.content_hash.is_empty()
            || clip.media_type != "video/webm"
        {
            return Err("video playback requires a nonzero handle and admitted video/webm clip");
        }
        self.active = Some((handle, clip.clone()));
        Ok(VideoProjectionOp::Play { handle, clip })
    }

    pub fn stop(&mut self, handle: VideoPlaybackHandle) -> Option<VideoProjectionOp> {
        if self
            .active
            .as_ref()
            .is_some_and(|(active, _)| *active == handle)
        {
            self.active = None;
            Some(VideoProjectionOp::Stop { handle })
        } else {
            None
        }
    }

    pub fn skip(&mut self, handle: VideoPlaybackHandle) -> Option<VideoProjectionOp> {
        if self
            .active
            .as_ref()
            .is_some_and(|(active, _)| *active == handle)
        {
            self.active = None;
            Some(VideoProjectionOp::Skip { handle })
        } else {
            None
        }
    }

    pub fn snapshot(&self) -> Vec<VideoProjectionOp> {
        self.active
            .as_ref()
            .map(|(handle, clip)| VideoProjectionOp::Play {
                handle: *handle,
                clip: clip.clone(),
            })
            .into_iter()
            .collect()
    }
}

pub fn video_frame(ops: impl IntoIterator<Item = VideoProjectionOp>) -> PresentationFrameDiff {
    let ops = ops
        .into_iter()
        .enumerate()
        .map(|(sequence, op)| PresentationOp::Video {
            meta: PresentationOpMeta::new(sequence as u32),
            op,
        })
        .collect();
    PresentationFrameDiff::try_from_ops(ops)
        .expect("video projector emits bounded contiguous operations")
}
