//! Engine-owned offline output lifetime. Product calls stage requests; each
//! call's settled jobs go to the runtime's executor once, as
//! [`RenderOutputWork`], and their results arrive between calls, never
//! through a callback.
use crate::operation_diagnostics::{clear_receipt, refuse};
use crate::{
    camera_view::RuntimeCameraViewCall,
    composition::{borrowed_utf8, ABI_OK},
    render_resources::RenderOutputResources,
    CsharpEngineServicesError,
};
use csharp_engine_abi::*;
use render_host_contracts::{RenderOutputJob, RenderOutputOperation};
use std::{
    collections::BTreeMap,
    ffi::c_void,
    sync::{Arc, Mutex, PoisonError},
};

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
        pose: Option<OutputPose>,
    },
    Glb {
        source: u64,
        include_animations: bool,
    },
}
/// An image of one animated instance of the source posed by a clip.
#[derive(Clone)]
struct OutputPose {
    object: u64,
    clip: String,
    /// A fraction of the clip.
    normalized_time: f32,
}
#[derive(Clone)]
struct Entry {
    state: NativeRenderOutputState,
    request: Option<Request>,
    bytes: Arc<Vec<u8>>,
    diagnostic: Arc<Vec<u8>>,
}

type Results = Arc<Mutex<Vec<(u64, Result<Vec<u8>, String>)>>>;

/// A settled output job, the resources its frozen frame reads, and where its
/// result goes. The resources stay alive with the work even if the product
/// releases them first.
pub struct RenderOutputWork {
    pub job: RenderOutputJob,
    pub resources: RenderOutputResources,
    results: Option<Results>,
}

impl RenderOutputWork {
    /// Deliver the job's PNG or GLB bytes, or the diagnostic that failed it.
    /// The product reads it from its next callback on.
    pub fn complete(mut self, result: Result<Vec<u8>, String>) {
        self.deliver(result);
    }

    fn deliver(&mut self, result: Result<Vec<u8>, String>) {
        if let Some(results) = self.results.take() {
            results
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((self.job.id, result));
        }
    }
}

/// Work dropped unfinished (its executor stopped) fails rather than staying
/// pending forever.
impl Drop for RenderOutputWork {
    fn drop(&mut self) {
        self.deliver(Err(
            "the render output executor stopped before the job finished".to_owned(),
        ));
    }
}
#[derive(Clone, Default)]
pub(crate) struct RuntimeRenderOutputCall {
    entries: BTreeMap<u64, Entry>,
}

pub(crate) struct RuntimeRenderOutputBridge {
    state: RuntimeRenderOutputCall,
    staged: Option<RuntimeRenderOutputCall>,
    next: u64,
    /// Results delivered since the last call began.
    results: Results,
    borrowed: crate::operation_diagnostics::BorrowedResult,
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
            results: Default::default(),
            borrowed: Default::default(),
        }
    }
    pub(crate) fn begin_call(&mut self) {
        let results =
            std::mem::take(&mut *self.results.lock().unwrap_or_else(PoisonError::into_inner));
        for (id, result) in results {
            self.complete(id, result);
        }
        // The call owns the state until it finishes; nothing is copied.
        self.staged = Some(std::mem::take(&mut self.state));
    }
    pub(crate) fn take_call(
        &mut self,
    ) -> Result<RuntimeRenderOutputCall, CsharpEngineServicesError> {
        self.staged
            .take()
            .ok_or_else(|| error("output called outside product callback"))
    }
    /// Ends the open call, keeping its state.
    #[cfg(test)]
    pub(crate) fn end_call(&mut self) {
        let call = self.take_call().expect("an open render output call");
        self.commit(call);
    }
    pub(crate) fn commit(&mut self, call: RuntimeRenderOutputCall) {
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
        self.next = id + 1;
        self.staged()?.entries.insert(
            id,
            Entry {
                state: NativeRenderOutputState::Pending,
                request: Some(request),
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
        // A one-off capture keeps no state across frames: an odd background
        // or exposure only shows in its own image.
        if r.width == 0 || r.height == 0 {
            return Err(error("capture dimensions must be above 0"));
        }
        let pose = if r.pose_object_id == 0 {
            None
        } else {
            let clip = unsafe { borrowed_utf8(r.pose_clip.bytes, r.pose_clip.len, "pose clip") }?
                .to_owned();
            Some(OutputPose {
                object: r.pose_object_id,
                clip,
                // Within the clip; NaN takes its start.
                normalized_time: if r.pose_time.is_nan() {
                    0.0
                } else {
                    r.pose_time.clamp(0.0, 1.0) as f32
                },
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
    ) -> Result<NativeByteResult, CsharpEngineServicesError> {
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
        let result = NativeByteResult {
            bytes: bytes.as_ptr(),
            len: bytes.len(),
        };
        self.borrowed.hold(bytes);
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
            e.request = None;
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
    /// Freeze each new request's scene into a job, for the executor. A
    /// request that cannot settle fails with its diagnostic.
    pub(crate) fn settle(
        &self,
        call: &mut RuntimeRenderOutputCall,
        world: &render_presentation::PresentationWorld,
        cameras: &RuntimeCameraViewCall,
        appearance: &crate::appearance::RuntimeAppearanceState,
    ) -> Vec<RenderOutputWork> {
        let mut work = Vec::new();
        for (&id, e) in &mut call.entries {
            let Some(request) = e.request.take() else {
                continue;
            };
            let settled = (|| -> Result<RenderOutputJob, CsharpEngineServicesError> {
                let (source, operation, pose) = match request {
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
                        },
                        pose,
                    ),
                    Request::Glb {
                        source,
                        include_animations,
                    } => (
                        source,
                        RenderOutputOperation::Glb { include_animations },
                        None,
                    ),
                };
                let source = appearance.output_object_handle(source)?;
                let retain_background = matches!(
                    &operation,
                    RenderOutputOperation::Image {
                        use_camera_background: true,
                        ..
                    }
                );
                let mut frame = world
                    .capture_output_scene(source, retain_background)
                    .map_err(|cause| error(format!("capture scene: {cause:?}")))?;
                // A posed image draws its instance in the clip's pose.
                if let Some(pose) = pose {
                    let handle = appearance.output_object_handle(pose.object)?;
                    let posed = appearance
                        .poses
                        .sample_clip(handle, &pose.clip, pose.normalized_time, world)
                        .ok_or_else(|| {
                            error(format!(
                                "capture: pose sample: object {} is not a posed animated mesh",
                                pose.object
                            ))
                        })?;
                    frame.push(render_model::RenderDiff::SetEvaluatedPose {
                        handle,
                        pose: posed,
                    });
                }
                Ok(RenderOutputJob {
                    id,
                    source,
                    frame,
                    operation,
                })
            })();
            match settled {
                Ok(job) => work.push(RenderOutputWork {
                    job,
                    resources: appearance.render_resources.output_resources(),
                    results: Some(Arc::clone(&self.results)),
                }),
                Err(cause) => {
                    e.state = NativeRenderOutputState::Failed;
                    e.diagnostic = Arc::new(cause.to_string().into_bytes());
                }
            }
        }
        work
    }
    /// A result for a pending job; a cancelled or destroyed job stays so.
    fn complete(&mut self, id: u64, result: Result<Vec<u8>, String>) {
        let Some(e) = self.state.entries.get_mut(&id) else {
            return;
        };
        if e.state != NativeRenderOutputState::Pending {
            return;
        }
        match result {
            Ok(bytes) => {
                e.state = NativeRenderOutputState::Completed;
                e.bytes = Arc::new(bytes);
            }
            Err(message) => {
                e.state = NativeRenderOutputState::Failed;
                e.diagnostic = Arc::new(message.into_bytes());
            }
        }
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
    error: *mut NativeOperationErrorReceipt,
) -> i32 {
    clear_receipt(error);
    if context.is_null() || out.is_null() {
        return 0;
    }
    match unsafe { &mut *context.cast::<RuntimeRenderOutputBridge>() }.read(h) {
        Ok(v) => {
            unsafe { *out = v };
            ABI_OK
        }
        Err(refusal) => refuse(&refusal, error),
    }
}
unsafe fn bytes(
    context: *mut c_void,
    h: NativeRenderOutputHandle,
    out: *mut NativeByteResult,
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
    o: *mut NativeByteResult,
) -> i32 {
    unsafe { bytes(c, h, o, false) }
}
unsafe extern "C" fn read_diagnostic(
    c: *mut c_void,
    h: NativeRenderOutputHandle,
    o: *mut NativeByteResult,
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
    }
}

#[cfg(test)]
mod tests {
    use render_model::RenderHandle;

    use super::*;

    fn work(bridge: &RuntimeRenderOutputBridge, id: u64) -> RenderOutputWork {
        RenderOutputWork {
            job: RenderOutputJob {
                id,
                source: RenderHandle::new(1),
                frame: render_model::RenderFrameDiff {
                    publication: None,
                    ops: Vec::new(),
                },
                operation: RenderOutputOperation::Glb {
                    include_animations: false,
                },
            },
            resources: RenderOutputResources::default(),
            results: Some(Arc::clone(&bridge.results)),
        }
    }

    #[test]
    fn results_arrive_at_the_next_call_and_cancelled_jobs_stay_cancelled() {
        let mut bridge = RuntimeRenderOutputBridge::new();
        bridge.begin_call();
        let request = |bridge: &mut RuntimeRenderOutputBridge| {
            bridge
                .request(Request::Glb {
                    source: 1,
                    include_animations: true,
                })
                .unwrap()
        };
        let (done, failed, cancelled, dropped) = (
            request(&mut bridge),
            request(&mut bridge),
            request(&mut bridge),
            request(&mut bridge),
        );
        bridge.cancel(cancelled).unwrap();
        let call = bridge.take_call().unwrap();
        bridge.commit(call);

        work(&bridge, done.value).complete(Ok(vec![1, 2, 3]));
        work(&bridge, failed.value).complete(Err("no source".to_owned()));
        work(&bridge, cancelled.value).complete(Ok(vec![9]));
        drop(work(&bridge, dropped.value));

        bridge.begin_call();
        assert_eq!(
            bridge.read(done).unwrap().state,
            NativeRenderOutputState::Completed
        );
        let result = bridge.bytes(done, false).unwrap();
        assert_eq!(
            unsafe { std::slice::from_raw_parts(result.bytes, result.len) },
            [1, 2, 3]
        );
        assert_eq!(
            bridge.read(failed).unwrap().state,
            NativeRenderOutputState::Failed
        );
        assert_eq!(
            bridge.read(cancelled).unwrap().state,
            NativeRenderOutputState::Cancelled
        );
        assert!(bridge.bytes(cancelled, false).is_err());
        assert_eq!(
            bridge.read(dropped).unwrap().state,
            NativeRenderOutputState::Failed
        );
        bridge.destroy(done).unwrap();
        assert!(bridge.read(done).is_err(), "a destroy is final");
        bridge.end_call();
    }
}
