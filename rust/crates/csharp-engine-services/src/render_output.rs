//! Engine-owned offline output lifetime. Product calls stage requests; the
//! renderer transfers completed bytes between calls, never through a callback.
use crate::{
    appearance::CsharpRenderResource,
    camera_view::RuntimeCameraViewCall,
    composition::{borrowed_utf8, ABI_OK},
    CsharpEngineServicesError,
};
use csharp_engine_abi::*;
use render_host_contracts::{
    RenderOutputChunk, RenderOutputJob, RenderOutputOperation, RenderOutputPose,
};
use render_model::RenderHandle;
use std::{collections::BTreeMap, ffi::c_void, sync::Arc};

#[derive(Clone)]
enum Request {
    Image {
        source: u64,
        camera: u64,
        width: u32,
        height: u32,
        background: [f32; 4],
        use_camera_background: bool,
        exposure: f32,
        aces_filmic: bool,
        samples: u32,
        pose: Option<RenderOutputPose>,
    },
    Glb {
        source: u64,
        include_animations: bool,
    },
}
#[derive(Clone)]
struct Entry {
    state: NativeRenderOutputState,
    request: Option<Request>,
    job: Option<Arc<RenderOutputJob>>,
    resources: Vec<CsharpRenderResource>,
    bytes: Arc<Vec<u8>>,
    diagnostic: Arc<Vec<u8>>,
}
#[derive(Clone, Default)]
pub(crate) struct RuntimeRenderOutputCall {
    entries: BTreeMap<u64, Entry>,
}

pub(crate) struct RuntimeRenderOutputBridge {
    state: RuntimeRenderOutputCall,
    staged: Option<RuntimeRenderOutputCall>,
    next: u64,
    published: Vec<u64>,
    leases: BTreeMap<u64, Arc<Vec<u8>>>,
    next_lease: u64,
}
fn error(message: impl Into<String>) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_RENDER_OUTPUT", message)
}
impl RuntimeRenderOutputBridge {
    pub(crate) fn new() -> Self {
        Self {
            state: Default::default(),
            staged: None,
            next: 1,
            published: vec![],
            leases: BTreeMap::new(),
            next_lease: 1,
        }
    }
    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(self.state.clone());
    }
    pub(crate) fn discard_call(&mut self) {
        self.staged = None;
    }
    pub(crate) fn take_call(
        &mut self,
    ) -> Result<RuntimeRenderOutputCall, CsharpEngineServicesError> {
        self.staged
            .take()
            .ok_or_else(|| error("output called outside product callback"))
    }
    pub(crate) fn commit(&mut self, call: RuntimeRenderOutputCall) {
        self.published = Self::job_ids(&call);
        self.state = call;
    }
    fn staged(&mut self) -> Result<&mut RuntimeRenderOutputCall, CsharpEngineServicesError> {
        self.staged
            .as_mut()
            .ok_or_else(|| error("output called outside product callback"))
    }
    fn request(
        &mut self,
        request: Request,
    ) -> Result<NativeRenderOutputHandle, CsharpEngineServicesError> {
        let id = self.next;
        self.next = id
            .checked_add(1)
            .ok_or_else(|| error("output handles exhausted"))?;
        self.staged()?.entries.insert(
            id,
            Entry {
                state: NativeRenderOutputState::Pending,
                request: Some(request),
                job: None,
                resources: vec![],
                bytes: Arc::new(vec![]),
                diagnostic: Arc::new(vec![]),
            },
        );
        Ok(NativeRenderOutputHandle { value: id })
    }
    fn image(
        &mut self,
        r: &NativeCaptureImageRequest,
    ) -> Result<NativeRenderOutputHandle, CsharpEngineServicesError> {
        let background = [
            r.background.r,
            r.background.g,
            r.background.b,
            r.background.a,
        ];
        if r.width == 0
            || r.height == 0
            || !background
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            || !r.exposure.is_finite()
            || r.exposure < 0.0
        {
            return Err(error(
                "capture dimensions, background, or exposure are invalid",
            ));
        }
        let pose = if r.pose_object_id == 0 {
            None
        } else {
            if !r.pose_time.is_finite() || !(0.0..=1.0).contains(&r.pose_time) {
                return Err(error("pose time must be in [0,1]"));
            }
            let clip = unsafe { borrowed_utf8(r.pose_clip.bytes, r.pose_clip.len, "pose clip") }?
                .to_owned();
            Some(RenderOutputPose {
                handle: RenderHandle::new(r.pose_object_id),
                clip,
                normalized_time: r.pose_time,
            })
        };
        self.request(Request::Image {
            source: r.source_object_id,
            camera: r.camera.value,
            width: r.width,
            height: r.height,
            background,
            use_camera_background: r.use_camera_background,
            exposure: r.exposure,
            aces_filmic: matches!(r.tone_mapping, NativeCaptureToneMapping::AcesFilmic),
            samples: r.samples,
            pose,
        })
    }
    fn glb(
        &mut self,
        r: &NativeExportSceneGlbRequest,
    ) -> Result<NativeRenderOutputHandle, CsharpEngineServicesError> {
        self.request(Request::Glb {
            source: r.source_object_id,
            include_animations: r.include_animations,
        })
    }
    fn read(
        &mut self,
        h: NativeRenderOutputHandle,
    ) -> Result<NativeRenderOutputReadout, CsharpEngineServicesError> {
        let e = self
            .staged()?
            .entries
            .get(&h.value)
            .ok_or_else(|| error("unknown output"))?;
        Ok(NativeRenderOutputReadout {
            state: e.state,
            byte_length: if e.state == NativeRenderOutputState::Completed {
                e.bytes.len()
            } else {
                0
            },
        })
    }
    fn bytes(
        &mut self,
        h: NativeRenderOutputHandle,
        diagnostic: bool,
    ) -> Result<NativeByteLease, CsharpEngineServicesError> {
        let e = self
            .staged()?
            .entries
            .get(&h.value)
            .ok_or_else(|| error("unknown output"))?;
        if !diagnostic && e.state != NativeRenderOutputState::Completed {
            return Err(error("output is not completed"));
        }
        let bytes = if diagnostic {
            e.diagnostic.clone()
        } else {
            e.bytes.clone()
        };
        let id = self.next_lease;
        self.next_lease = id
            .checked_add(1)
            .ok_or_else(|| error("lease handles exhausted"))?;
        let result = NativeByteLease {
            handle: NativeByteLeaseHandle { value: id },
            bytes: bytes.as_ptr(),
            len: bytes.len(),
        };
        self.leases.insert(id, bytes);
        Ok(result)
    }
    fn cancel(&mut self, h: NativeRenderOutputHandle) -> Result<(), CsharpEngineServicesError> {
        let e = self
            .staged()?
            .entries
            .get_mut(&h.value)
            .ok_or_else(|| error("unknown output"))?;
        if e.state == NativeRenderOutputState::Pending {
            e.state = NativeRenderOutputState::Cancelled;
            e.job = None;
            e.request = None;
            e.resources.clear();
            e.bytes = Arc::new(vec![]);
        }
        Ok(())
    }
    fn destroy(&mut self, h: NativeRenderOutputHandle) -> Result<(), CsharpEngineServicesError> {
        self.staged()?
            .entries
            .remove(&h.value)
            .ok_or_else(|| error("unknown output"))?;
        Ok(())
    }
    pub(crate) fn settle(
        call: &mut RuntimeRenderOutputCall,
        world: &render_presentation::PresentationWorld,
        cameras: &RuntimeCameraViewCall,
        resources: Vec<CsharpRenderResource>,
        appearance: &crate::appearance::RuntimeAppearanceState,
    ) -> Result<(), CsharpEngineServicesError> {
        for (&id, e) in &mut call.entries {
            let Some(request) = e.request.take() else {
                continue;
            };
            let settled = (|| -> Result<RenderOutputJob, CsharpEngineServicesError> {
                let (source, operation) = match request {
                    Request::Image {
                        source,
                        camera,
                        width,
                        height,
                        background,
                        use_camera_background,
                        exposure,
                        aces_filmic,
                        samples,
                        pose,
                    } => (
                        source,
                        RenderOutputOperation::Image {
                            camera: Box::new(cameras.output_camera(camera)?),
                            width,
                            height,
                            background,
                            use_camera_background,
                            exposure,
                            aces_filmic,
                            samples,
                            pose,
                        },
                    ),
                    Request::Glb {
                        source,
                        include_animations,
                    } => (source, RenderOutputOperation::Glb { include_animations }),
                };
                let source = appearance.output_object_handle(source)?;
                let mut operation = operation;
                if let RenderOutputOperation::Image {
                    pose: Some(pose), ..
                } = &mut operation
                {
                    pose.handle = appearance.output_object_handle(pose.handle.raw())?;
                }
                let retain_background = matches!(
                    &operation,
                    RenderOutputOperation::Image {
                        use_camera_background: true,
                        ..
                    }
                );
                let frame = world
                    .capture_output_scene(source, retain_background)
                    .map_err(|cause| error(format!("capture scene: {cause:?}")))?;
                Ok(RenderOutputJob {
                    id,
                    source,
                    frame,
                    operation,
                })
            })();
            match settled {
                Ok(job) => e.job = Some(Arc::new(job)),
                Err(cause) => {
                    e.state = NativeRenderOutputState::Failed;
                    e.diagnostic = Arc::new(cause.to_string().into_bytes());
                    continue;
                }
            }
            // Arc-backed immutable resources remain available even if the product
            // releases its source before the renderer fetches the frozen frame.
            e.resources = resources.clone();
        }
        Ok(())
    }
    pub(crate) fn jobs(call: &RuntimeRenderOutputCall) -> Vec<RenderOutputJob> {
        call.entries
            .values()
            .filter_map(|e| e.job.as_deref().cloned())
            .collect()
    }
    fn job_ids(call: &RuntimeRenderOutputCall) -> Vec<u64> {
        call.entries
            .iter()
            .filter_map(|(&id, e)| e.job.as_ref().map(|_| id))
            .collect()
    }
    pub(crate) fn changed_jobs(
        &self,
        call: &RuntimeRenderOutputCall,
    ) -> Option<Vec<RenderOutputJob>> {
        (Self::job_ids(call) != self.published).then(|| Self::jobs(call))
    }
    pub(crate) fn snapshot(&self) -> Vec<RenderOutputJob> {
        Self::jobs(&self.state)
    }
    pub(crate) fn resources(&self) -> impl Iterator<Item = &CsharpRenderResource> {
        self.state.entries.values().flat_map(|e| e.resources.iter())
    }
    pub(crate) fn ingest(
        &mut self,
        chunk: RenderOutputChunk,
    ) -> Result<(), CsharpEngineServicesError> {
        let Some(e) = self.state.entries.get_mut(&chunk.id) else {
            return Ok(());
        };
        if e.state != NativeRenderOutputState::Pending {
            return Ok(());
        }
        if let Some(message) = chunk.error {
            e.state = NativeRenderOutputState::Failed;
            e.diagnostic = Arc::new(message.into_bytes());
            e.bytes = Arc::new(vec![]);
        } else {
            // A reattached renderer can restart serialization of the same
            // frozen job. Offset zero starts a fresh transfer; terminal jobs
            // above remain immutable even if completion is retried.
            if chunk.offset == 0 {
                e.bytes = Arc::new(Vec::new());
            }
            if chunk.offset < e.bytes.len() {
                if e.bytes
                    .get(chunk.offset..chunk.offset.saturating_add(chunk.bytes.len()))
                    == Some(chunk.bytes.as_slice())
                {
                    return Ok(());
                }
                return Err(error("output chunk retry disagrees with accepted bytes"));
            }
            if chunk.offset != e.bytes.len() {
                return Err(error("output chunk is not contiguous"));
            }
            Arc::make_mut(&mut e.bytes).extend_from_slice(&chunk.bytes);
            if chunk.complete {
                e.state = NativeRenderOutputState::Completed;
            }
        }
        if e.state != NativeRenderOutputState::Pending {
            e.job = None;
            e.resources.clear();
        }
        Ok(())
    }
}

macro_rules! request_call {
    ($name:ident,$method:ident,$request:ty) => {
        unsafe extern "C" fn $name(
            context: *mut c_void,
            request: *const $request,
            out: *mut NativeRenderOutputHandle,
        ) -> i32 {
            if context.is_null() || request.is_null() || out.is_null() {
                return 0;
            }
            let bridge = unsafe { &mut *context.cast::<RuntimeRenderOutputBridge>() };
            match bridge.$method(unsafe { &*request }) {
                Ok(value) => {
                    unsafe { *out = value };
                    ABI_OK
                }
                Err(_) => 0,
            }
        }
    };
}
request_call!(capture_image, image, NativeCaptureImageRequest);
request_call!(export_scene_glb, glb, NativeExportSceneGlbRequest);
unsafe extern "C" fn read(
    context: *mut c_void,
    h: NativeRenderOutputHandle,
    out: *mut NativeRenderOutputReadout,
) -> i32 {
    if context.is_null() || out.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeRenderOutputBridge>() }.read(h) {
        Ok(v) => {
            unsafe { *out = v };
            ABI_OK
        }
        Err(_) => 0,
    }
}
unsafe fn bytes(
    context: *mut c_void,
    h: NativeRenderOutputHandle,
    out: *mut NativeByteLease,
    diagnostic: bool,
) -> i32 {
    if context.is_null() || out.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeRenderOutputBridge>() }.bytes(h, diagnostic) {
        Ok(v) => {
            unsafe { *out = v };
            ABI_OK
        }
        Err(_) => 0,
    }
}
unsafe extern "C" fn read_bytes(
    c: *mut c_void,
    h: NativeRenderOutputHandle,
    o: *mut NativeByteLease,
) -> i32 {
    unsafe { bytes(c, h, o, false) }
}
unsafe extern "C" fn read_diagnostic(
    c: *mut c_void,
    h: NativeRenderOutputHandle,
    o: *mut NativeByteLease,
) -> i32 {
    unsafe { bytes(c, h, o, true) }
}
unsafe extern "C" fn cancel(c: *mut c_void, h: NativeRenderOutputHandle) -> i32 {
    if c.is_null() {
        return 0;
    }
    if unsafe { &mut *c.cast::<RuntimeRenderOutputBridge>() }
        .cancel(h)
        .is_ok()
    {
        ABI_OK
    } else {
        0
    }
}
unsafe extern "C" fn destroy(c: *mut c_void, h: NativeRenderOutputHandle) -> i32 {
    if c.is_null() {
        return 0;
    }
    if unsafe { &mut *c.cast::<RuntimeRenderOutputBridge>() }
        .destroy(h)
        .is_ok()
    {
        ABI_OK
    } else {
        0
    }
}
unsafe extern "C" fn destroy_byte_lease(c: *mut c_void, h: NativeByteLeaseHandle) -> i32 {
    if c.is_null() {
        return 0;
    }
    if unsafe { &mut *c.cast::<RuntimeRenderOutputBridge>() }
        .leases
        .remove(&h.value)
        .is_some()
    {
        ABI_OK
    } else {
        0
    }
}
pub(crate) fn api(b: &mut RuntimeRenderOutputBridge) -> NativeRenderOutputApi {
    NativeRenderOutputApi {
        context: (b as *mut RuntimeRenderOutputBridge).cast(),
        capture_image,
        export_scene_glb,
        read,
        read_bytes,
        read_diagnostic,
        cancel,
        destroy,
        destroy_byte_lease,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chunks_complete_once_and_cancelled_jobs_cannot_be_resurrected() {
        let mut bridge = RuntimeRenderOutputBridge::new();
        bridge.begin_call();
        let handle = bridge
            .request(Request::Glb {
                source: 1,
                include_animations: true,
            })
            .unwrap();
        let call = bridge.take_call().unwrap();
        bridge.commit(call);
        let chunk = RenderOutputChunk {
            id: handle.value,
            offset: 0,
            bytes: vec![1, 2],
            complete: false,
            error: None,
        };
        bridge.ingest(chunk.clone()).unwrap();
        bridge.ingest(chunk).unwrap();
        assert!(bridge
            .ingest(RenderOutputChunk {
                id: handle.value,
                offset: 3,
                bytes: vec![3],
                complete: true,
                error: None
            })
            .is_err());
        bridge
            .ingest(RenderOutputChunk {
                id: handle.value,
                offset: 2,
                bytes: vec![3],
                complete: true,
                error: None,
            })
            .unwrap();
        bridge.begin_call();
        assert_eq!(
            bridge.read(handle).unwrap().state,
            NativeRenderOutputState::Completed
        );
        let lease = bridge.bytes(handle, false).unwrap();
        assert_eq!(
            unsafe { std::slice::from_raw_parts(lease.bytes, lease.len) },
            [1, 2, 3]
        );
        let cancelled = bridge
            .request(Request::Glb {
                source: 1,
                include_animations: false,
            })
            .unwrap();
        bridge.cancel(cancelled).unwrap();
        let call = bridge.take_call().unwrap();
        bridge.commit(call);
        bridge
            .ingest(RenderOutputChunk {
                id: cancelled.value,
                offset: 0,
                bytes: vec![9],
                complete: true,
                error: None,
            })
            .unwrap();
        bridge.begin_call();
        assert_eq!(
            bridge.read(cancelled).unwrap().state,
            NativeRenderOutputState::Cancelled
        );
        assert!(bridge.bytes(cancelled, false).is_err());
        bridge.destroy(handle).unwrap();
        bridge.discard_call();
        bridge.begin_call();
        assert_eq!(bridge.read(handle).unwrap().byte_length, 3);
    }
}
