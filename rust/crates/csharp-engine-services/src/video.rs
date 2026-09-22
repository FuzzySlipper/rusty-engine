use std::{
    collections::{BTreeMap, VecDeque},
    ffi::c_void,
    sync::Arc,
};

use csharp_engine_abi::*;
use render_presentation::{video_frame, VideoClipRef, VideoPlaybackHandle, VideoProjector};

use crate::{
    appearance::CsharpRenderResource,
    composition::{borrowed_utf8, ABI_OK},
    content::RuntimeContentBridge,
    CsharpEngineServicesError,
};

const MAX_VIDEO_FACTS: usize = 128;

#[derive(Clone)]
struct VideoState {
    projector: VideoProjector,
    active: Option<(u64, CsharpRenderResource)>,
    next_handle: u64,
}

pub(crate) struct RuntimeVideoCall {
    state: VideoState,
    pub(crate) frame: Option<render_presentation::PresentationFrameDiff>,
}

/// Browser realization observations are copied into this Engine owner between
/// admitted product calls. Browser timing never mutates product policy.
#[derive(Clone, Copy)]
pub enum VideoRealizationFact {
    Completed {
        fact_id: u64,
        handle: u64,
    },
    Skipped {
        fact_id: u64,
        handle: u64,
    },
    Failed {
        fact_id: u64,
        handle: u64,
        failure: NativeVideoFailureCode,
    },
}

impl VideoRealizationFact {
    fn id(self) -> u64 {
        match self {
            Self::Completed { fact_id, .. }
            | Self::Skipped { fact_id, .. }
            | Self::Failed { fact_id, .. } => fact_id,
        }
    }
}

pub(crate) struct RuntimeVideoBridge {
    state: VideoState,
    content_resources: BTreeMap<String, Arc<[u8]>>,
    staged: Option<RuntimeVideoCall>,
    callback_error: Option<CsharpEngineServicesError>,
    facts: VecDeque<VideoRealizationFact>,
    evicted: u64,
    accepted_through: Option<u64>,
    content: Option<*const RuntimeContentBridge>,
}

impl RuntimeVideoBridge {
    pub(crate) fn new(content_resources: BTreeMap<String, Arc<[u8]>>) -> Self {
        Self {
            state: VideoState {
                projector: VideoProjector::default(),
                active: None,
                next_handle: 1,
            },
            content_resources,
            staged: None,
            callback_error: None,
            facts: VecDeque::new(),
            evicted: 0,
            accepted_through: None,
            content: None,
        }
    }
    pub(crate) fn bind_content(&mut self, content: &RuntimeContentBridge) {
        self.content = Some(content as *const RuntimeContentBridge);
    }
    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(RuntimeVideoCall {
            state: self.state.clone(),
            frame: None,
        });
        self.callback_error = None;
    }
    pub(crate) fn discard_call(&mut self) {
        self.staged = None;
        self.callback_error = None;
    }
    pub(crate) fn take_staged_call(
        &mut self,
    ) -> Result<RuntimeVideoCall, CsharpEngineServicesError> {
        self.staged.take().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VIDEO_CALL",
                "video service was called outside a product call",
            )
        })
    }
    pub(crate) fn commit(&mut self, call: RuntimeVideoCall) {
        self.state = call.state;
    }
    pub(crate) fn render_resources(&self) -> impl Iterator<Item = &CsharpRenderResource> {
        self.state.active.iter().map(|(_, resource)| resource)
    }
    pub(crate) fn snapshot_call_frame(
        call: &RuntimeVideoCall,
    ) -> render_presentation::PresentationFrameDiff {
        video_frame(call.state.projector.snapshot())
    }
    pub(crate) fn ingest_realized_feedback(
        &mut self,
        replace: bool,
        evicted_fact_count: u64,
        facts: impl IntoIterator<Item = VideoRealizationFact>,
    ) -> Result<(), CsharpEngineServicesError> {
        if replace {
            self.facts.clear();
            self.evicted = evicted_fact_count;
            self.accepted_through = None;
        } else if evicted_fact_count > self.evicted {
            self.evicted = evicted_fact_count;
        }
        for fact in facts {
            if self.accepted_through.is_some_and(|id| fact.id() <= id) {
                continue;
            }
            if self.facts.len() == MAX_VIDEO_FACTS {
                self.facts.pop_front();
                self.evicted += 1;
            }
            self.accepted_through = Some(fact.id());
            self.facts.push_back(fact);
        }
        Ok(())
    }
    pub(crate) fn reset_realized_feedback(&mut self) {
        let _ = self.ingest_realized_feedback(true, 0, []);
    }
    fn staged(&mut self) -> Result<&mut RuntimeVideoCall, CsharpEngineServicesError> {
        self.staged.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VIDEO_CALL",
                "video service was called outside a product call",
            )
        })
    }
    fn play_selected(
        &mut self,
        path: &str,
        bytes: Arc<[u8]>,
    ) -> Result<NativeVideoPlaybackHandle, CsharpEngineServicesError> {
        let path = path.strip_prefix("content/").unwrap_or(path);
        let resource =
            CsharpRenderResource::admit_video(format!("content/{path}"), bytes.to_vec())?;
        let staged = self.staged()?;
        let handle = staged.state.next_handle;
        staged.state.next_handle = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VIDEO_HANDLE",
                "video playback handles exhausted",
            )
        })?;
        let clip = VideoClipRef {
            asset: resource.identity().to_owned(),
            content_hash: resource.content_hash().to_owned(),
            media_type: "video/webm".to_owned(),
        };
        let op = staged
            .state
            .projector
            .play(VideoPlaybackHandle::new(handle), clip)
            .map_err(|detail| CsharpEngineServicesError::new("CSHARP_VIDEO_PLAY", detail))?;
        staged.state.active = Some((handle, resource));
        staged.frame = Some(video_frame([op]));
        Ok(NativeVideoPlaybackHandle { value: handle })
    }
    fn play(
        &mut self,
        request: &NativePlayVideoRequest,
    ) -> Result<NativeVideoPlaybackHandle, CsharpEngineServicesError> {
        let path =
            unsafe { borrowed_utf8(request.path.bytes, request.path.len, "video resource path")? }
                .strip_prefix("content/")
                .unwrap_or(unsafe {
                    borrowed_utf8(request.path.bytes, request.path.len, "video resource path")?
                });
        let bytes = self.content_resources.get(path).cloned().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VIDEO_RESOURCE_UNKNOWN",
                format!("product content has no video resource `{path}`"),
            )
        })?;
        self.play_selected(path, bytes)
    }
    fn play_from_content(
        &mut self,
        request: NativePlayVideoFromContentRequest,
    ) -> Result<NativeVideoPlaybackHandle, CsharpEngineServicesError> {
        let retained = self
            .content
            .and_then(|pointer| unsafe { pointer.as_ref() })
            .and_then(|content| content.retained_content(request.content))
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_VIDEO_CONTENT",
                    "video content reference is not retained",
                )
            })?;
        self.play_selected(&retained.path, retained.bytes)
    }
    fn stop(
        &mut self,
        handle: NativeVideoPlaybackHandle,
        skip: bool,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged()?;
        let op = if skip {
            staged
                .state
                .projector
                .skip(VideoPlaybackHandle::new(handle.value))
        } else {
            staged
                .state
                .projector
                .stop(VideoPlaybackHandle::new(handle.value))
        }
        .ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_VIDEO_HANDLE",
                "video operation does not name active playback",
            )
        })?;
        staged.state.active = None;
        staged.frame = Some(video_frame([op]));
        Ok(())
    }
    fn read(&mut self) -> Result<NativeVideoReadout, CsharpEngineServicesError> {
        let active = self
            .staged()?
            .state
            .active
            .as_ref()
            .map(|(handle, _)| *handle);
        Ok(NativeVideoReadout {
            active: active.is_some(),
            active_handle: NativeVideoPlaybackHandle {
                value: active.unwrap_or(0),
            },
        })
    }
    fn read_realization(
        &mut self,
    ) -> Result<NativeVideoRealizationReadout, CsharpEngineServicesError> {
        let _ = self.staged()?;
        Ok(NativeVideoRealizationReadout {
            retained_fact_count: self.facts.len() as u32,
            evicted_fact_count: self.evicted,
        })
    }
    fn read_fact(
        &mut self,
        request: NativeVideoRealizationFactAtRequest,
    ) -> Result<NativeVideoRealizationFactAtReceipt, CsharpEngineServicesError> {
        let _ = self.staged()?;
        let Some(fact) = self.facts.get(request.index as usize).copied() else {
            return Ok(NativeVideoRealizationFactAtReceipt::default());
        };
        Ok(match fact {
            VideoRealizationFact::Completed { fact_id, handle } => {
                NativeVideoRealizationFactAtReceipt {
                    present: true,
                    kind: NativeVideoRealizationFactKind::Completed,
                    fact_id,
                    handle: NativeVideoPlaybackHandle { value: handle },
                    failure: NativeVideoFailureCode::None,
                }
            }
            VideoRealizationFact::Skipped { fact_id, handle } => {
                NativeVideoRealizationFactAtReceipt {
                    present: true,
                    kind: NativeVideoRealizationFactKind::Skipped,
                    fact_id,
                    handle: NativeVideoPlaybackHandle { value: handle },
                    failure: NativeVideoFailureCode::None,
                }
            }
            VideoRealizationFact::Failed {
                fact_id,
                handle,
                failure,
            } => NativeVideoRealizationFactAtReceipt {
                present: true,
                kind: NativeVideoRealizationFactKind::Failed,
                fact_id,
                handle: NativeVideoPlaybackHandle { value: handle },
                failure,
            },
        })
    }
}

macro_rules! call { ($name:ident, $method:ident $(, $arg:ident : $ty:ty )* => $out:ty) => { pub(crate) unsafe extern "C" fn $name(context: *mut c_void, $($arg: $ty,)* result: *mut $out) -> i32 { if context.is_null() || result.is_null() { return 0; } let bridge = unsafe { &mut *context.cast::<RuntimeVideoBridge>() }; match bridge.$method($($arg),*) { Ok(value) => { unsafe { *result = value; } ABI_OK }, Err(error) => { bridge.callback_error = Some(error); 0 } } } }; }
pub(crate) unsafe extern "C" fn play_video(
    context: *mut c_void,
    request: *const NativePlayVideoRequest,
    result: *mut NativeVideoPlaybackHandle,
) -> i32 {
    if request.is_null() {
        return 0;
    }
    unsafe { play_video_call(context, &*request, result) }
}
pub(crate) unsafe extern "C" fn play_video_from_content(
    context: *mut c_void,
    request: *const NativePlayVideoFromContentRequest,
    result: *mut NativeVideoPlaybackHandle,
) -> i32 {
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVideoBridge>() };
    match bridge.play_from_content(unsafe { *request }) {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}
call!(play_video_call, play, request: &NativePlayVideoRequest => NativeVideoPlaybackHandle);
pub(crate) unsafe extern "C" fn stop_video(
    context: *mut c_void,
    handle: NativeVideoPlaybackHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVideoBridge>() };
    match bridge.stop(handle, false) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn skip_video(
    context: *mut c_void,
    handle: NativeVideoPlaybackHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeVideoBridge>() };
    match bridge.stop(handle, true) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}
call!(read_video, read => NativeVideoReadout);
call!(read_video_realization, read_realization => NativeVideoRealizationReadout);
call!(read_video_fact, read_fact, request: NativeVideoRealizationFactAtRequest => NativeVideoRealizationFactAtReceipt);

pub(crate) fn api(bridge: &mut RuntimeVideoBridge) -> NativeVideoApi {
    NativeVideoApi {
        context: (bridge as *mut RuntimeVideoBridge).cast(),
        play: play_video,
        play_from_content: play_video_from_content,
        stop: stop_video,
        skip: skip_video,
        read: read_video,
        read_realization: read_video_realization,
        read_realization_fact_at: read_video_fact,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn plays_a_retained_content_reference_without_a_path_resource_map() {
        let mut files = BTreeMap::new();
        files.insert(
            "cinematic.webm".to_owned(),
            Arc::from(&[0x1a, 0x45, 0xdf, 0xa3, b'w', b'e', b'b', b'm'][..]),
        );
        let mut content = RuntimeContentBridge::new(files);
        let content_api = crate::content::api(&mut content);
        let path = b"cinematic.webm";
        let mut reference = NativeContentReferenceHandle::default();
        assert_eq!(
            unsafe {
                (content_api.open_reference)(
                    content_api.context,
                    &NativeContentOpenRequest {
                        path: NativeUtf8Slice {
                            bytes: path.as_ptr(),
                            len: path.len(),
                        },
                    },
                    &mut reference,
                )
            },
            ABI_OK
        );
        let mut bridge = RuntimeVideoBridge::new(BTreeMap::new());
        bridge.bind_content(&content);
        bridge.begin_call();
        let handle = bridge
            .play_from_content(NativePlayVideoFromContentRequest { content: reference })
            .expect("retained video content plays");
        assert_ne!(handle.value, 0);
        assert_eq!(
            bridge.render_resources().count(),
            0,
            "uncommitted calls do not publish resources"
        );
        let call = bridge.take_staged_call().expect("staged video call");
        bridge.commit(call);
        assert_eq!(bridge.render_resources().count(), 1);
    }

    #[test]
    fn realization_readout_preserves_renderer_eviction_count_and_reset_clears_it() {
        let mut bridge = RuntimeVideoBridge::new(BTreeMap::new());
        bridge
            .ingest_realized_feedback(
                true,
                3,
                [VideoRealizationFact::Completed {
                    fact_id: 7,
                    handle: 2,
                }],
            )
            .expect("bounded renderer feedback");
        bridge.begin_call();
        let readout = bridge.read_realization().expect("read realization");
        assert_eq!(readout.retained_fact_count, 1);
        assert_eq!(readout.evicted_fact_count, 3);
        bridge.discard_call();

        bridge.reset_realized_feedback();
        bridge.begin_call();
        let reset = bridge.read_realization().expect("read reset realization");
        assert_eq!(reset.retained_fact_count, 0);
        assert_eq!(reset.evicted_fact_count, 0);
    }
}
