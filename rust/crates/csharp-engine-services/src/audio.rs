use runtime_diagnostics::RuntimeDiagnosticsSink;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ffi::c_void,
    sync::Arc,
};

use csharp_engine_abi::*;
use render_model::{RenderAssetKind, ResolvedRenderAsset, JSON_SAFE_U64_MAX};
use render_presentation::{
    AudioBus, AudioBusControl, AudioEmitter, AudioHandle, AudioProjectionDiagnosticCode,
    AudioProjectionOp, AudioProjector, AudioSourceDescriptor, AudioVoiceControl,
    AudioVoiceDesiredState, PresentationFrameDiff, PresentationOp, PresentationOpMeta,
};

#[cfg(test)]
use render_presentation::MAX_AUDIO_DIAGNOSTICS;

use crate::{
    appearance::CsharpRenderResource,
    composition::{borrowed_utf8, ABI_OK},
    content::{RetainedContent, RuntimeContentBridge},
    CsharpEngineServicesError,
};

const MAX_AUDIO_RESOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_AUDIO_RESOURCE_COUNT: usize = 64;
const MAX_AUDIO_RESOURCE_TOTAL_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
struct AudioClip {
    asset: String,
    content_hash: String,
    duration_seconds: Option<f64>,
    resource: CsharpRenderResource,
    /// Every successful clip admission acquires one product owner. A shared
    /// native handle remains valid until all callers release it.
    owners: u32,
}

#[derive(Clone)]
struct AudioState {
    projector: AudioProjector,
    clips: BTreeMap<u64, AudioClip>,
    assets: BTreeMap<String, ResolvedRenderAsset>,
    voices: BTreeMap<u64, AudioHandle>,
    voice_clips: BTreeMap<u64, u64>,
    one_shot_clips: BTreeMap<u64, u64>,
    /// A browser feedback overflow loses one or more terminal identities.
    /// Pending one-shots remain protected until an owner reset cancels them.
    one_shot_feedback_lost: bool,
    next_clip: u64,
    next_voice: u64,
    next_signal: u64,
}

/// Copied Engine browser-host realization feedback. Kept separate from the
/// projector state so C# can distinguish desired presentation from what Web
/// Audio actually completed or diagnosed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioRealizationFact {
    NaturalCompletionOneShot {
        fact_id: u64,
        sequence: u32,
        signal_handle: u64,
    },
    NaturalCompletionRetainedVoice {
        fact_id: u64,
        sequence: u32,
        voice_handle: u64,
    },
    Diagnostic {
        fact_id: u64,
        code: NativeAudioDiagnosticCode,
        sequence: u32,
        signal_handle: Option<u64>,
        voice_handle: Option<u64>,
    },
}

impl AudioRealizationFact {
    pub const fn fact_id(&self) -> u64 {
        match self {
            Self::NaturalCompletionOneShot { fact_id, .. }
            | Self::NaturalCompletionRetainedVoice { fact_id, .. }
            | Self::Diagnostic { fact_id, .. } => *fact_id,
        }
    }

    fn receipt(&self) -> NativeAudioRealizationFactAtReceipt {
        match *self {
            Self::NaturalCompletionOneShot {
                fact_id,
                sequence,
                signal_handle,
            } => NativeAudioRealizationFactAtReceipt {
                present: true,
                kind: NativeAudioRealizationFactKind::NaturalCompletionOneShot,
                fact_id,
                sequence,
                signal_handle,
                voice_value: 0,
                code: NativeAudioDiagnosticCode::None,
            },
            Self::NaturalCompletionRetainedVoice {
                fact_id,
                sequence,
                voice_handle,
            } => NativeAudioRealizationFactAtReceipt {
                present: true,
                kind: NativeAudioRealizationFactKind::NaturalCompletionRetainedVoice,
                fact_id,
                sequence,
                signal_handle: 0,
                voice_value: voice_handle,
                code: NativeAudioDiagnosticCode::None,
            },
            Self::Diagnostic {
                fact_id,
                code,
                sequence,
                signal_handle,
                voice_handle,
            } => NativeAudioRealizationFactAtReceipt {
                present: true,
                kind: NativeAudioRealizationFactKind::Diagnostic,
                fact_id,
                sequence,
                signal_handle: signal_handle.unwrap_or(0),
                voice_value: voice_handle.unwrap_or(0),
                code,
            },
        }
    }
}

pub(crate) struct RuntimeAudioCall {
    state: AudioState,
    pub(crate) frame: Option<render_presentation::PresentationFrameDiff>,
    /// Resources released from the final state after emitting a same-call
    /// operation remain available to that call's renderer publication.
    /// This is intentionally not committed into the persistent audio state.
    pub(crate) retired_resources: Vec<CsharpRenderResource>,
}

/// Engine-owned audio admission and projector bridge. WAV resources are
/// admitted from immutable Engine content and realized only by the browser
/// host; post-Create admission retains the selected body in this owner.
pub(crate) struct RuntimeAudioBridge {
    state: AudioState,
    content_resources: BTreeMap<String, Arc<[u8]>>,
    staged: Option<RuntimeAudioCall>,
    callback_error: Option<CsharpEngineServicesError>,
    operation_diagnostics: crate::operation_diagnostics::OperationDiagnostics,
    realized_facts: VecDeque<AudioRealizationFact>,
    renderer_evicted_fact_count: u64,
    local_evicted_fact_count: u64,
    accepted_through_fact_id: Option<u64>,
    diagnostics_sink: Option<RuntimeDiagnosticsSink>,
    reported_recoverable_codes: BTreeSet<&'static str>,
    content: Option<*const RuntimeContentBridge>,
}

impl RuntimeAudioBridge {
    pub(crate) fn new(content_resources: BTreeMap<String, Arc<[u8]>>) -> Self {
        Self {
            state: AudioState {
                projector: AudioProjector::default(),
                clips: BTreeMap::new(),
                assets: BTreeMap::new(),
                voices: BTreeMap::new(),
                voice_clips: BTreeMap::new(),
                one_shot_clips: BTreeMap::new(),
                one_shot_feedback_lost: false,
                next_clip: 1,
                next_voice: 1,
                next_signal: 1,
            },
            content_resources,
            staged: None,
            callback_error: None,
            operation_diagnostics: Default::default(),
            realized_facts: VecDeque::new(),
            renderer_evicted_fact_count: 0,
            local_evicted_fact_count: 0,
            accepted_through_fact_id: None,
            diagnostics_sink: None,
            reported_recoverable_codes: BTreeSet::new(),
            content: None,
        }
    }

    /// Composition supplies the stable boxed content owner. Audio retains
    /// copied resource bytes and never borrows through this pointer.
    pub(crate) fn bind_content(&mut self, content: &RuntimeContentBridge) {
        self.content = Some(content as *const RuntimeContentBridge);
    }

    /// Replaces or incrementally admits a browser-owned snapshot between C#
    /// calls. Monotonic fact ids make retries harmless; the copied FIFO stays
    /// bounded independently of the browser host's own eviction count.
    pub(crate) fn ingest_realized_feedback(
        &mut self,
        replace_owner: bool,
        evicted_fact_count: u64,
        facts: impl IntoIterator<Item = AudioRealizationFact>,
    ) -> Result<(), CsharpEngineServicesError> {
        let facts: Vec<_> = facts.into_iter().collect();
        if facts.len() > 128
            || facts
                .windows(2)
                .any(|facts| facts[0].fact_id() >= facts[1].fact_id())
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_AUDIO_REALIZATION_BOUNDS",
                "audio realization feedback must contain at most 128 strictly ordered facts",
            ));
        }
        if replace_owner {
            self.realized_facts.clear();
            self.renderer_evicted_fact_count = evicted_fact_count;
            self.local_evicted_fact_count = 0;
            self.accepted_through_fact_id = None;
            // Historical one-shots are not replayed into a fresh host, so
            // their resources no longer have an active realization owner.
            self.state.one_shot_clips.clear();
            self.state.one_shot_feedback_lost = false;
        } else if evicted_fact_count > self.renderer_evicted_fact_count {
            self.renderer_evicted_fact_count = evicted_fact_count;
            self.state.one_shot_feedback_lost = !self.state.one_shot_clips.is_empty();
        }
        for fact in facts {
            if self
                .accepted_through_fact_id
                .is_some_and(|last| fact.fact_id() <= last)
            {
                continue;
            }
            if self.realized_facts.len() == 128 {
                self.realized_facts.pop_front();
                self.local_evicted_fact_count = self.local_evicted_fact_count.saturating_add(1);
            }
            match &fact {
                AudioRealizationFact::NaturalCompletionOneShot { signal_handle, .. }
                | AudioRealizationFact::Diagnostic {
                    signal_handle: Some(signal_handle),
                    ..
                } => {
                    self.state.one_shot_clips.remove(signal_handle);
                }
                _ => {}
            }
            let fact_id = fact.fact_id();
            self.realized_facts.push_back(fact);
            self.accepted_through_fact_id = Some(fact_id);
        }
        Ok(())
    }

    pub(crate) fn reset_realized_feedback(&mut self) {
        self.realized_facts.clear();
        self.renderer_evicted_fact_count = 0;
        self.local_evicted_fact_count = 0;
        self.accepted_through_fact_id = None;
        // An exact runtime binding reset also resets the browser audio owner,
        // cancelling active one-shots before the next feedback owner starts.
        self.state.one_shot_clips.clear();
        self.state.one_shot_feedback_lost = false;
    }

    pub(crate) fn bind_diagnostics_sink(&mut self, sink: RuntimeDiagnosticsSink) {
        self.diagnostics_sink = Some(sink);
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(RuntimeAudioCall {
            state: self.state.clone(),
            frame: None,
            retired_resources: Vec::new(),
        });
        self.callback_error = None;
    }

    /// Stages retained playback against the update interval that the runtime
    /// already admitted. Advancing the clone keeps a failed callback from
    /// mutating canonical playback intent.
    pub(crate) fn begin_update_call(&mut self, elapsed_seconds: f64) {
        self.begin_call();
        if let Some(staged) = self.staged.as_mut() {
            staged.state.projector.advance_elapsed(elapsed_seconds);
        }
    }

    pub(crate) fn discard_call(&mut self) {
        self.staged = None;
        self.callback_error = None;
    }

    pub(crate) fn take_staged_call(
        &mut self,
    ) -> Result<RuntimeAudioCall, CsharpEngineServicesError> {
        if let Some(error) = self.callback_error.take() {
            self.staged = None;
            return Err(error);
        }
        self.staged.take().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CALL",
                "audio service was called outside a product call",
            )
        })
    }

    pub(crate) fn commit(&mut self, call: RuntimeAudioCall) {
        self.state = call.state;
    }
    pub(crate) fn seal_resource_selection(&mut self) {
        self.content_resources.clear();
    }

    pub(crate) fn render_resources(&self) -> impl Iterator<Item = &CsharpRenderResource> {
        self.state.clips.values().map(|clip| &clip.resource)
    }

    /// Resources required to realize this call. In addition to the final
    /// retained clip set, this includes a clip released after its same-call
    /// voice operations were staged.
    #[cfg(test)]
    pub(crate) fn publication_resources(
        call: &RuntimeAudioCall,
    ) -> impl Iterator<Item = &CsharpRenderResource> {
        call.state
            .clips
            .values()
            .map(|clip| &clip.resource)
            .chain(call.retired_resources.iter())
    }

    /// Reconstructs the retained audio intent on a fresh realization. This
    /// copies only voices and closed-bus state: historical one-shot signals
    /// and realization feedback intentionally never cross a baseline.
    ///
    /// A baseline uses the Engine cursor and desired state. Historical
    /// one-shots and browser realization feedback remain outside it.
    #[cfg(test)]
    pub(crate) fn snapshot_frame(
        &self,
    ) -> Result<PresentationFrameDiff, CsharpEngineServicesError> {
        Self::snapshot_frame_for_state(&self.state)
    }

    /// Snapshot a staged call before commit when its enclosing presentation
    /// publication needs the same transaction boundary.
    pub(crate) fn snapshot_call_frame(
        call: &RuntimeAudioCall,
    ) -> Result<PresentationFrameDiff, CsharpEngineServicesError> {
        Self::snapshot_frame_for_state(&call.state)
    }

    fn snapshot_frame_for_state(
        state: &AudioState,
    ) -> Result<PresentationFrameDiff, CsharpEngineServicesError> {
        let mut ops = Vec::new();
        for voice in state.projector.active_voices() {
            ops.push(PresentationOp::Audio {
                meta: PresentationOpMeta::new(next_presentation_sequence(ops.len())?),
                op: AudioProjectionOp::Restore {
                    handle: voice.handle,
                    descriptor: voice.descriptor,
                    desired_state: voice.desired_state,
                    cursor_seconds: voice.cursor_seconds,
                },
            });
        }
        for bus in state.projector.buses() {
            ops.push(PresentationOp::Audio {
                meta: PresentationOpMeta::new(next_presentation_sequence(ops.len())?),
                op: AudioProjectionOp::BusControl {
                    bus: bus.bus,
                    control: AudioBusControl::SetVolume { volume: bus.volume },
                },
            });
            ops.push(PresentationOp::Audio {
                meta: PresentationOpMeta::new(next_presentation_sequence(ops.len())?),
                op: AudioProjectionOp::BusControl {
                    bus: bus.bus,
                    control: AudioBusControl::SetMuted { muted: bus.muted },
                },
            });
        }
        PresentationFrameDiff::try_from_ops(ops).map_err(|error| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_BASELINE",
                format!("retained audio baseline is invalid: {error:?}"),
            )
        })
    }

    fn staged_mut(&mut self) -> Result<&mut RuntimeAudioCall, CsharpEngineServicesError> {
        self.staged.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CALL",
                "audio service was called outside a product call",
            )
        })
    }

    fn staged_ref(&self) -> Result<&RuntimeAudioCall, CsharpEngineServicesError> {
        self.staged.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CALL",
                "audio service was called outside a product call",
            )
        })
    }

    fn optional_preload_receipt(
        &self,
        outcome: NativeAudioOptionalPreloadOutcome,
        clip: NativeAudioClipHandle,
    ) -> Result<NativeAudioOptionalPreloadReceipt, CsharpEngineServicesError> {
        let state = &self.staged_ref()?.state;
        let admitted_bytes = state
            .clips
            .values()
            .map(|clip| clip.resource.bytes().len() as u64)
            .sum();
        Ok(NativeAudioOptionalPreloadReceipt {
            outcome,
            clip,
            admitted_clip_count: state.clips.len() as u32,
            admitted_bytes,
            max_clip_count: MAX_AUDIO_RESOURCE_COUNT as u32,
            max_total_bytes: MAX_AUDIO_RESOURCE_TOTAL_BYTES as u64,
        })
    }

    fn report_optional_preload_skip(&mut self, code: &'static str, message: &'static str) {
        if let Some(sink) = self.diagnostics_sink.as_ref() {
            crate::diagnostics::publish_recoverable_once(
                sink,
                &mut self.reported_recoverable_codes,
                "audio",
                code,
                message,
            );
        }
    }

    fn clip_source_from_retained(
        &self,
        content: RetainedContent,
    ) -> Result<(String, Arc<[u8]>), CsharpEngineServicesError> {
        if content.path.is_empty() {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_AUDIO_RESOURCE_PATH",
                "audio content reference has no canonical path",
            ));
        }
        Ok((content.path, content.bytes))
    }

    fn clip_source_from_path(
        &self,
        requested_path: &str,
    ) -> Result<(String, Arc<[u8]>), CsharpEngineServicesError> {
        let relative_path = requested_path
            .strip_prefix("content/")
            .unwrap_or(requested_path);
        let retained = self
            .content
            .and_then(|content| unsafe { content.as_ref() })
            .and_then(|content| content.retained_path(relative_path))
            .map(|content| self.clip_source_from_retained(content))
            .transpose()?
            .or_else(|| {
                self.content_resources
                    .get(relative_path)
                    .cloned()
                    .map(|bytes| (relative_path.to_owned(), bytes))
            });
        retained.ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_RESOURCE_UNKNOWN",
                format!("product content has no audio resource `{requested_path}`"),
            )
        })
    }

    fn clip_source_from_reference(
        &self,
        reference: NativeContentReferenceHandle,
    ) -> Result<(String, Arc<[u8]>), CsharpEngineServicesError> {
        let content = self
            .content
            .and_then(|content| unsafe { content.as_ref() })
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_CONTENT",
                    "audio content references are not composed",
                )
            })?
            .retained_content(reference)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_CONTENT",
                    "audio content reference is not retained",
                )
            })?;
        self.clip_source_from_retained(content)
    }

    fn acquire_existing_clip(
        &mut self,
        resource_identity: &str,
    ) -> Result<Option<NativeAudioClipHandle>, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let Some((handle, clip)) = staged
            .state
            .clips
            .iter_mut()
            .find(|(_, clip)| clip.resource.identity() == resource_identity)
        else {
            return Ok(None);
        };
        clip.owners = clip.owners.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CLIP_OWNERS",
                "audio clip owner count exhausted",
            )
        })?;
        Ok(Some(NativeAudioClipHandle { value: *handle }))
    }

    fn admit_clip(
        &mut self,
        relative_path: String,
        bytes: Arc<[u8]>,
    ) -> Result<NativeAudioClipHandle, CsharpEngineServicesError> {
        let browser_path = format!("content/{relative_path}");
        let resource = CsharpRenderResource::admit_audio(browser_path, bytes.to_vec())?;
        if let Some(handle) = self.acquire_existing_clip(resource.identity())? {
            return Ok(handle);
        }
        if bytes.len() > MAX_AUDIO_RESOURCE_BYTES {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_AUDIO_RESOURCE_SIZE",
                "audio resource exceeds the Engine audio preload limit",
            ));
        }
        let staged = self.staged_mut()?;
        if staged.state.clips.len() == MAX_AUDIO_RESOURCE_COUNT {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_AUDIO_RESOURCE_COUNT",
                "audio resource count exceeds the Engine browser-host limit",
            ));
        }
        let total_bytes = staged
            .state
            .clips
            .values()
            .map(|clip| clip.resource.bytes().len())
            .sum::<usize>();
        if total_bytes.saturating_add(bytes.len()) > MAX_AUDIO_RESOURCE_TOTAL_BYTES {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_AUDIO_RESOURCE_TOTAL_SIZE",
                "audio resources exceed the Engine browser-host total preload limit",
            ));
        }
        let duration_seconds = wav_duration_seconds(resource.bytes());
        let content_hash = resource.content_hash().to_owned();
        let asset = format!(
            "audio/{}",
            content_hash
                .strip_prefix("sha256:")
                .expect("audio resource identity has a SHA-256 hash")
        );
        let handle = staged.state.next_clip;
        staged.state.next_clip = handle.checked_add(1).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CLIP_HANDLE",
                "audio clip handles exhausted",
            )
        })?;
        staged.state.assets.insert(
            asset.clone(),
            ResolvedRenderAsset {
                id: asset.clone(),
                kind: RenderAssetKind::Audio,
                content_hash: Some(content_hash.clone()),
                version: 0,
            },
        );
        staged.state.clips.insert(
            handle,
            AudioClip {
                asset,
                content_hash,
                duration_seconds,
                resource,
                owners: 1,
            },
        );
        Ok(NativeAudioClipHandle { value: handle })
    }

    fn open_clip(
        &mut self,
        request: &NativeAudioClipRequest,
    ) -> Result<NativeAudioClipHandle, CsharpEngineServicesError> {
        let requested_path =
            unsafe { borrowed_utf8(request.path.bytes, request.path.len, "audio resource path")? };
        let (relative_path, bytes) = self.clip_source_from_path(requested_path)?;
        self.admit_clip(relative_path, bytes)
    }

    fn open_clip_from_content(
        &mut self,
        request: NativeAudioClipFromContentRequest,
    ) -> Result<NativeAudioClipHandle, CsharpEngineServicesError> {
        let (relative_path, bytes) = self.clip_source_from_reference(request.content)?;
        self.admit_clip(relative_path, bytes)
    }

    fn preload_optional(
        &mut self,
        request: &NativeAudioClipRequest,
    ) -> Result<NativeAudioOptionalPreloadReceipt, CsharpEngineServicesError> {
        let requested_path =
            unsafe { borrowed_utf8(request.path.bytes, request.path.len, "audio resource path")? }
                .to_owned();
        let (relative_path, bytes) = match self.clip_source_from_path(&requested_path) {
            Ok(source) => source,
            Err(error) if error.code() == "CSHARP_AUDIO_RESOURCE_UNKNOWN" => {
                let receipt = self.optional_preload_receipt(
                    NativeAudioOptionalPreloadOutcome::SkippedMissing,
                    NativeAudioClipHandle::default(),
                )?;
                self.report_optional_preload_skip(
                    "CSHARP_AUDIO_PRELOAD_SKIPPED_MISSING",
                    "optional audio preload was skipped because the content resource is absent",
                );
                return Ok(receipt);
            }
            Err(error) => return Err(error),
        };
        let browser_path = format!("content/{relative_path}");
        // A present resource must be structurally admitted before an optional
        // capacity receipt is allowed. Otherwise a corrupt oversized body
        // could masquerade as routine budget pressure.
        let resource = CsharpRenderResource::admit_audio(browser_path, bytes.to_vec())?;
        if let Some(handle) = self.acquire_existing_clip(resource.identity())? {
            return self
                .optional_preload_receipt(NativeAudioOptionalPreloadOutcome::Admitted, handle);
        }
        let duration_seconds = wav_duration_seconds(resource.bytes());
        let (admitted_clip_count, admitted_bytes) = {
            let state = &self.staged_ref()?.state;
            (
                state.clips.len(),
                state
                    .clips
                    .values()
                    .map(|clip| clip.resource.bytes().len())
                    .sum::<usize>(),
            )
        };
        if bytes.len() > MAX_AUDIO_RESOURCE_BYTES
            || admitted_clip_count == MAX_AUDIO_RESOURCE_COUNT
            || admitted_bytes.saturating_add(bytes.len()) > MAX_AUDIO_RESOURCE_TOTAL_BYTES
        {
            let receipt = self.optional_preload_receipt(
                NativeAudioOptionalPreloadOutcome::SkippedCapacity,
                NativeAudioClipHandle::default(),
            )?;
            self.report_optional_preload_skip(
                "CSHARP_AUDIO_PRELOAD_SKIPPED_CAPACITY",
                "optional audio preload was skipped because the Engine preload budget is exhausted",
            );
            return Ok(receipt);
        }
        let content_hash = resource.content_hash().to_owned();
        let asset = format!(
            "audio/{}",
            content_hash
                .strip_prefix("sha256:")
                .expect("audio resource identity has a SHA-256 hash")
        );
        let handle = {
            let staged = self.staged_mut()?;
            let handle = staged.state.next_clip;
            staged.state.next_clip = handle.checked_add(1).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_CLIP_HANDLE",
                    "audio clip handles exhausted",
                )
            })?;
            staged.state.assets.insert(
                asset.clone(),
                ResolvedRenderAsset {
                    id: asset.clone(),
                    kind: RenderAssetKind::Audio,
                    content_hash: Some(content_hash.clone()),
                    version: 0,
                },
            );
            staged.state.clips.insert(
                handle,
                AudioClip {
                    asset,
                    content_hash,
                    duration_seconds,
                    resource,
                    owners: 1,
                },
            );
            handle
        };
        self.optional_preload_receipt(
            NativeAudioOptionalPreloadOutcome::Admitted,
            NativeAudioClipHandle { value: handle },
        )
    }

    fn descriptor(
        &mut self,
        value: NativeAudioSourceDescriptor,
    ) -> Result<AudioSourceDescriptor, CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let clip = staged.state.clips.get(&value.clip.value).ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CLIP_HANDLE",
                "audio clip handle is not admitted",
            )
        })?;
        let bus = match value.bus {
            NativeAudioBus::Sfx => AudioBus::Sfx,
            NativeAudioBus::Ambient => AudioBus::Ambient,
            NativeAudioBus::Ui => AudioBus::Ui,
        };
        let emitter = match value.emitter_kind {
            NativeAudioEmitterKind::Global2d => AudioEmitter::Global2d,
            NativeAudioEmitterKind::World3d => AudioEmitter::World3d {
                position: [value.position.x, value.position.y, value.position.z],
            },
            NativeAudioEmitterKind::EntityAttached => AudioEmitter::EntityAttached {
                entity: value.entity,
                offset: [value.offset.x, value.offset.y, value.offset.z],
            },
        };
        Ok(AudioSourceDescriptor {
            clip: render_presentation::AudioClipRef {
                asset: clip.asset.clone(),
                content_hash: clip.content_hash.clone(),
                duration_seconds: clip.duration_seconds,
            },
            bus,
            volume: value.volume,
            pitch: value.pitch,
            looping: value.looping,
            spatial_blend: value.spatial_blend,
            attenuation: value.attenuation,
            pan: value.pan,
            emitter,
        })
    }

    fn stage_op(&mut self, op: AudioProjectionOp) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let sequence = u32::try_from(staged.frame.as_ref().map_or(0, |frame| frame.ops.len()))
            .map_err(|_| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_FRAME",
                    "audio presentation frame has too many operations",
                )
            })?;
        let projected = staged
            .state
            .projector
            .project(&staged.state.assets, PresentationOpMeta::new(sequence), op)
            .map_err(audio_error)?;
        let frame = staged
            .frame
            .get_or_insert_with(render_presentation::PresentationFrameDiff::new);
        frame.ops.push(projected);
        Ok(())
    }

    fn emit(
        &mut self,
        request: NativeAudioEmitRequest,
    ) -> Result<NativeAudioSignalHandle, CsharpEngineServicesError> {
        let signal_id = unsafe {
            borrowed_utf8(
                request.signal_id.bytes,
                request.signal_id.len,
                "audio signal id",
            )?
        }
        .to_owned();
        let descriptor = self.descriptor(request.descriptor)?;
        let signal_handle = {
            let staged = self.staged_mut()?;
            let value = staged.state.next_signal;
            if value > JSON_SAFE_U64_MAX {
                return Err(CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_SIGNAL_HANDLE",
                    "audio signal handles exhausted the JSON-safe range",
                ));
            }
            staged.state.next_signal = value.checked_add(1).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_SIGNAL_HANDLE",
                    "audio signal handles exhausted",
                )
            })?;
            NativeAudioSignalHandle { value }
        };
        self.stage_op(AudioProjectionOp::Emit {
            signal_handle: render_presentation::AudioSignalHandle::new(signal_handle.value),
            signal_id,
            descriptor,
        })?;
        self.staged_mut()?
            .state
            .one_shot_clips
            .insert(signal_handle.value, request.descriptor.clip.value);
        Ok(signal_handle)
    }

    fn create_voice(
        &mut self,
        descriptor: NativeAudioSourceDescriptor,
    ) -> Result<NativeAudioVoiceHandle, CsharpEngineServicesError> {
        let clip = descriptor.clip.value;
        let descriptor = self.descriptor(descriptor)?;
        let voice = {
            let staged = self.staged_mut()?;
            let voice = staged.state.next_voice;
            staged.state.next_voice = voice.checked_add(1).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_VOICE_HANDLE",
                    "audio voice handles exhausted",
                )
            })?;
            staged.state.voices.insert(voice, AudioHandle::new(voice));
            staged.state.voice_clips.insert(voice, clip);
            voice
        };
        self.stage_op(AudioProjectionOp::Create {
            handle: AudioHandle::new(voice),
            descriptor,
        })?;
        Ok(NativeAudioVoiceHandle { value: voice })
    }

    fn update_voice(
        &mut self,
        request: NativeAudioVoiceUpdateRequest,
    ) -> Result<(), CsharpEngineServicesError> {
        let descriptor = self.descriptor(request.descriptor)?;
        let handle = self
            .staged_mut()?
            .state
            .voices
            .get(&request.voice.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_VOICE_HANDLE",
                    "audio voice handle is not live",
                )
            })?;
        let current = self
            .staged_mut()?
            .state
            .projector
            .descriptor(handle)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_VOICE_HANDLE",
                    "audio voice projector state is missing",
                )
            })?;
        if current.clip != descriptor.clip || current.bus != descriptor.bus {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_AUDIO_UPDATE_REPLACEMENT",
                "UpdateVoice cannot replace an immutable clip or change bus; use ReplaceVoice",
            ));
        }
        let emitter = (current.emitter != descriptor.emitter).then_some(descriptor.emitter);
        self.stage_op(AudioProjectionOp::Update {
            handle,
            patch: render_presentation::AudioSourcePatch {
                volume: Some(descriptor.volume),
                pitch: Some(descriptor.pitch),
                looping: Some(descriptor.looping),
                spatial_blend: Some(descriptor.spatial_blend),
                attenuation: Some(descriptor.attenuation),
                pan: Some(descriptor.pan),
                emitter,
            },
        })
    }

    fn replace_voice(
        &mut self,
        request: NativeAudioVoiceReplaceRequest,
    ) -> Result<NativeAudioVoiceHandle, CsharpEngineServicesError> {
        let replacement_clip = request.descriptor.clip.value;
        let descriptor = self.descriptor(request.descriptor)?;
        let old = self
            .staged_mut()?
            .state
            .voices
            .remove(&request.voice.value)
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_VOICE_HANDLE",
                    "audio voice handle is not live",
                )
            })?;
        self.staged_mut()?
            .state
            .voice_clips
            .remove(&request.voice.value);
        self.stage_op(AudioProjectionOp::Destroy { handle: old })?;
        self.create_voice_native(descriptor, replacement_clip)
    }

    fn create_voice_native(
        &mut self,
        descriptor: AudioSourceDescriptor,
        clip: u64,
    ) -> Result<NativeAudioVoiceHandle, CsharpEngineServicesError> {
        let voice = {
            let staged = self.staged_mut()?;
            let voice = staged.state.next_voice;
            staged.state.next_voice = voice.checked_add(1).ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_VOICE_HANDLE",
                    "audio voice handles exhausted",
                )
            })?;
            staged.state.voices.insert(voice, AudioHandle::new(voice));
            staged.state.voice_clips.insert(voice, clip);
            voice
        };
        self.stage_op(AudioProjectionOp::Create {
            handle: AudioHandle::new(voice),
            descriptor,
        })?;
        Ok(NativeAudioVoiceHandle { value: voice })
    }

    fn destroy_voice(
        &mut self,
        voice: NativeAudioVoiceHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let Some(handle) = self.staged_mut()?.state.voices.remove(&voice.value) else {
            return Ok(());
        };
        self.staged_mut()?.state.voice_clips.remove(&voice.value);
        self.stage_op(AudioProjectionOp::Destroy { handle })
    }

    fn destroy_clip(
        &mut self,
        clip: NativeAudioClipHandle,
    ) -> Result<(), CsharpEngineServicesError> {
        let staged = self.staged_mut()?;
        let Some(current) = staged.state.clips.get(&clip.value) else {
            return Ok(());
        };
        if current.owners > 1 {
            staged
                .state
                .clips
                .get_mut(&clip.value)
                .expect("checked clip")
                .owners -= 1;
            return Ok(());
        }
        if staged
            .state
            .voice_clips
            .values()
            .any(|owner| *owner == clip.value)
        {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CLIP_IN_USE",
                "dispose or replace retained voices before disposing their clip",
            ));
        }
        if staged
            .state
            .one_shot_clips
            .values()
            .any(|owner| *owner == clip.value)
        {
            let (code, detail) = if staged.state.one_shot_feedback_lost {
                (
                    "CSHARP_AUDIO_CLIP_FEEDBACK_LOST",
                    "one-shot feedback overflowed; reset the runtime audio owner before disposing its clip",
                )
            } else {
                (
                    "CSHARP_AUDIO_CLIP_PENDING_SIGNAL",
                    "wait for the one-shot terminal realization before disposing its clip",
                )
            };
            return Err(CsharpEngineServicesError::new(code, detail));
        }
        let clip = staged
            .state
            .clips
            .remove(&clip.value)
            .expect("checked clip");
        staged.state.assets.remove(&clip.asset);
        staged.retired_resources.push(clip.resource);
        Ok(())
    }

    fn control_voice(
        &mut self,
        voice: NativeAudioVoiceHandle,
        control: NativeAudioVoiceControl,
    ) -> Result<(), CsharpEngineServicesError> {
        let handle = self
            .staged_mut()?
            .state
            .voices
            .get(&voice.value)
            .copied()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_VOICE_HANDLE",
                    "audio voice handle is not live",
                )
            })?;
        let control = match control {
            NativeAudioVoiceControl::Pause => AudioVoiceControl::Pause,
            NativeAudioVoiceControl::Resume => AudioVoiceControl::Resume,
            NativeAudioVoiceControl::Retrigger => AudioVoiceControl::Retrigger,
        };
        self.stage_op(AudioProjectionOp::VoiceControl { handle, control })
    }

    fn set_bus_volume(
        &mut self,
        bus: NativeAudioBus,
        volume: f32,
    ) -> Result<(), CsharpEngineServicesError> {
        self.stage_op(AudioProjectionOp::BusControl {
            bus: audio_bus(bus),
            control: AudioBusControl::SetVolume { volume },
        })
    }

    fn set_bus_muted(
        &mut self,
        bus: NativeAudioBus,
        muted: bool,
    ) -> Result<(), CsharpEngineServicesError> {
        self.stage_op(AudioProjectionOp::BusControl {
            bus: audio_bus(bus),
            control: AudioBusControl::SetMuted { muted },
        })
    }

    fn read(&mut self) -> Result<NativeAudioReadout, CsharpEngineServicesError> {
        let staged = self.staged.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CALL",
                "audio service was called outside a product call",
            )
        })?;
        let readout = staged.state.projector.readout();
        Ok(NativeAudioReadout {
            active_voices: readout.active_sources,
            paused_voices: readout.paused_sources,
            admitted_clips: staged.state.clips.len() as u32,
            emitted_signals: readout.emitted_signals,
            retained_diagnostic_count: readout.retained_diagnostic_count,
            evicted_diagnostic_count: readout.evicted_diagnostic_count,
        })
    }

    fn read_realization(
        &mut self,
    ) -> Result<NativeAudioRealizationReadout, CsharpEngineServicesError> {
        self.staged.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CALL",
                "audio service was called outside a product call",
            )
        })?;
        Ok(NativeAudioRealizationReadout {
            retained_fact_count: self.realized_facts.len() as u32,
            evicted_fact_count: self
                .renderer_evicted_fact_count
                .saturating_add(self.local_evicted_fact_count),
        })
    }

    fn read_realization_fact_at(
        &mut self,
        request: NativeAudioRealizationFactAtRequest,
    ) -> Result<NativeAudioRealizationFactAtReceipt, CsharpEngineServicesError> {
        self.staged.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CALL",
                "audio service was called outside a product call",
            )
        })?;
        Ok(self.realized_facts.get(request.index as usize).map_or_else(
            NativeAudioRealizationFactAtReceipt::default,
            AudioRealizationFact::receipt,
        ))
    }

    fn read_voice(
        &mut self,
        voice: NativeAudioVoiceHandle,
    ) -> Result<NativeAudioVoiceReadout, CsharpEngineServicesError> {
        let staged = self.staged.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CALL",
                "audio service was called outside a product call",
            )
        })?;
        Ok(staged
            .state
            .projector
            .voice(AudioHandle::new(voice.value))
            .map_or_else(NativeAudioVoiceReadout::default, |voice| {
                NativeAudioVoiceReadout {
                    present: true,
                    desired_state: match voice.desired_state {
                        AudioVoiceDesiredState::Playing => NativeAudioVoiceDesiredState::Playing,
                        AudioVoiceDesiredState::Paused => NativeAudioVoiceDesiredState::Paused,
                    },
                }
            }))
    }

    fn read_bus(
        &mut self,
        bus: NativeAudioBus,
    ) -> Result<NativeAudioBusReadout, CsharpEngineServicesError> {
        let staged = self.staged.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CALL",
                "audio service was called outside a product call",
            )
        })?;
        let readout = staged.state.projector.bus(audio_bus(bus));
        Ok(NativeAudioBusReadout {
            volume: readout.volume,
            muted: readout.muted,
        })
    }

    fn read_diagnostic_at(
        &mut self,
        request: NativeAudioDiagnosticAtRequest,
    ) -> Result<NativeAudioDiagnosticAtReceipt, CsharpEngineServicesError> {
        let staged = self.staged.as_ref().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CALL",
                "audio service was called outside a product call",
            )
        })?;
        let Some(diagnostic) = staged
            .state
            .projector
            .readout()
            .diagnostics
            .get(request.index as usize)
            .cloned()
        else {
            return Ok(NativeAudioDiagnosticAtReceipt::default());
        };
        Ok(NativeAudioDiagnosticAtReceipt {
            present: true,
            code: diagnostic_code(diagnostic.code),
            sequence: diagnostic.sequence,
            voice_value: diagnostic.handle.map_or(0, AudioHandle::raw),
        })
    }
}

fn next_presentation_sequence(length: usize) -> Result<u32, CsharpEngineServicesError> {
    u32::try_from(length).map_err(|_| {
        CsharpEngineServicesError::new(
            "CSHARP_AUDIO_BASELINE",
            "retained audio baseline has too many operations",
        )
    })
}

/// Reads the duration of ordinary RIFF/WAVE content with byte-rate metadata. Admission
/// deliberately remains compatible with the previous lightweight RIFF check:
/// an opaque or unusual body simply has no Engine timeline duration and is not
/// expired during recovery.
fn wav_duration_seconds(bytes: &[u8]) -> Option<f64> {
    if bytes.len() < 12 || bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return None;
    }
    let mut cursor = 12_usize;
    let mut byte_rate = None;
    let mut data_bytes = None;
    while cursor.checked_add(8)? <= bytes.len() {
        let id = bytes.get(cursor..cursor + 4)?;
        let length =
            u32::from_le_bytes(bytes.get(cursor + 4..cursor + 8)?.try_into().ok()?) as usize;
        let body = cursor.checked_add(8)?;
        let end = body.checked_add(length)?;
        if end > bytes.len() {
            return None;
        }
        if id == b"fmt " && length >= 12 {
            byte_rate = Some(u32::from_le_bytes(
                bytes.get(body + 8..body + 12)?.try_into().ok()?,
            ));
        } else if id == b"data" {
            data_bytes = Some(length);
        }
        if byte_rate.is_some() && data_bytes.is_some() {
            break;
        }
        cursor = end.checked_add(length % 2)?;
    }
    let byte_rate = byte_rate?;
    let data_bytes = data_bytes?;
    if byte_rate == 0 {
        return None;
    }
    let duration = data_bytes as f64 / f64::from(byte_rate);
    duration
        .is_finite()
        .then_some(duration)
        .filter(|duration| *duration > 0.0)
}

fn audio_bus(bus: NativeAudioBus) -> AudioBus {
    match bus {
        NativeAudioBus::Sfx => AudioBus::Sfx,
        NativeAudioBus::Ambient => AudioBus::Ambient,
        NativeAudioBus::Ui => AudioBus::Ui,
    }
}

fn audio_error(error: render_presentation::AudioProjectionDiagnostic) -> CsharpEngineServicesError {
    CsharpEngineServicesError::new("CSHARP_AUDIO_PROJECTION", error.message)
}
fn diagnostic_code(code: AudioProjectionDiagnosticCode) -> NativeAudioDiagnosticCode {
    match code {
        AudioProjectionDiagnosticCode::InvalidDescriptor => {
            NativeAudioDiagnosticCode::InvalidDescriptor
        }
        AudioProjectionDiagnosticCode::AssetMissing => NativeAudioDiagnosticCode::AssetMissing,
        AudioProjectionDiagnosticCode::AssetKindMismatch => {
            NativeAudioDiagnosticCode::AssetKindMismatch
        }
        AudioProjectionDiagnosticCode::ContentHashMismatch => {
            NativeAudioDiagnosticCode::ContentHashMismatch
        }
        AudioProjectionDiagnosticCode::DuplicateSignal => {
            NativeAudioDiagnosticCode::DuplicateSignal
        }
        AudioProjectionDiagnosticCode::DuplicateHandle => {
            NativeAudioDiagnosticCode::DuplicateHandle
        }
        AudioProjectionDiagnosticCode::UnknownHandle => NativeAudioDiagnosticCode::UnknownHandle,
        AudioProjectionDiagnosticCode::InvalidControl => NativeAudioDiagnosticCode::InvalidControl,
        AudioProjectionDiagnosticCode::UnavailableHost => {
            NativeAudioDiagnosticCode::UnavailableHost
        }
        AudioProjectionDiagnosticCode::AudioContextBlocked => {
            NativeAudioDiagnosticCode::AudioContextBlocked
        }
        AudioProjectionDiagnosticCode::DecodeFailed => NativeAudioDiagnosticCode::DecodeFailed,
        AudioProjectionDiagnosticCode::HostFailure => NativeAudioDiagnosticCode::HostFailure,
    }
}

pub(crate) unsafe extern "C" fn open_audio_clip(
    context: *mut c_void,
    request: *const NativeAudioClipRequest,
    result: *mut NativeAudioClipHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.open_clip(unsafe { &*request }) {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn open_audio_clip_from_content(
    context: *mut c_void,
    request: *const NativeAudioClipFromContentRequest,
    result: *mut NativeAudioClipHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.open_clip_from_content(unsafe { *request }) {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn preload_optional_audio_clip(
    context: *mut c_void,
    request: *const NativeAudioClipRequest,
    result: *mut NativeAudioOptionalPreloadReceipt,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.preload_optional(unsafe { &*request }) {
        Ok(receipt) => {
            unsafe { *result = receipt };
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn emit_audio(
    context: *mut c_void,
    request: *const NativeAudioEmitRequest,
    result: *mut NativeAudioSignalHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.emit(unsafe { *request }) {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn create_audio_voice(
    context: *mut c_void,
    request: *const NativeAudioSourceDescriptor,
    result: *mut NativeAudioVoiceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.create_voice(unsafe { *request }) {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn update_audio_voice(
    context: *mut c_void,
    request: *const NativeAudioVoiceUpdateRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.update_voice(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn replace_audio_voice(
    context: *mut c_void,
    request: *const NativeAudioVoiceReplaceRequest,
    result: *mut NativeAudioVoiceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.replace_voice(unsafe { *request }) {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn destroy_audio_voice(
    context: *mut c_void,
    voice: NativeAudioVoiceHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.destroy_voice(voice) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn destroy_audio_clip(
    context: *mut c_void,
    clip: NativeAudioClipHandle,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.destroy_clip(clip) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn control_audio_voice(
    context: *mut c_void,
    request: *const NativeAudioVoiceControlRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    let request = unsafe { *request };
    match bridge.control_voice(request.voice, request.control) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_audio_bus_volume(
    context: *mut c_void,
    request: *const NativeAudioBusVolumeRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    let request = unsafe { *request };
    match bridge.set_bus_volume(request.bus, request.volume) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_audio_bus_muted(
    context: *mut c_void,
    request: *const NativeAudioBusMutedRequest,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    let request = unsafe { *request };
    match bridge.set_bus_muted(request.bus, request.muted) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn read_audio(
    context: *mut c_void,
    result: *mut NativeAudioReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.read() {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_audio_voice(
    context: *mut c_void,
    request: *const NativeAudioVoiceReadRequest,
    result: *mut NativeAudioVoiceReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.read_voice(unsafe { (*request).voice }) {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_audio_bus(
    context: *mut c_void,
    request: *const NativeAudioBusReadRequest,
    result: *mut NativeAudioBusReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || request.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.read_bus(unsafe { (*request).bus }) {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn read_audio_diagnostic_at(
    context: *mut c_void,
    request: NativeAudioDiagnosticAtRequest,
    result: *mut NativeAudioDiagnosticAtReceipt,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.read_diagnostic_at(request) {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_audio_realization(
    context: *mut c_void,
    result: *mut NativeAudioRealizationReadout,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.read_realization() {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_audio_realization_fact_at(
    context: *mut c_void,
    request: NativeAudioRealizationFactAtRequest,
    result: *mut NativeAudioRealizationFactAtReceipt,
    operation_error: *mut NativeOperationErrorReceipt,
) -> i32 {
    if !operation_error.is_null() {
        unsafe { *operation_error = std::mem::zeroed() };
    }
    if context.is_null() || result.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.read_realization_fact_at(request) {
        Ok(value) => {
            unsafe {
                *result = value;
            }
            ABI_OK
        }
        Err(error) => {
            bridge.operation_diagnostics.retain(&error, operation_error);
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) fn api(bridge: &mut RuntimeAudioBridge) -> NativeAudioApi {
    NativeAudioApi {
        context: (bridge as *mut RuntimeAudioBridge).cast(),
        destroy_operation_diagnostic_lease,
        open_clip: open_audio_clip,
        open_clip_from_content: open_audio_clip_from_content,
        destroy_clip: destroy_audio_clip,
        preload_optional: preload_optional_audio_clip,
        emit: emit_audio,
        create_voice: create_audio_voice,
        update_voice: update_audio_voice,
        replace_voice: replace_audio_voice,
        destroy_voice: destroy_audio_voice,
        control_voice: control_audio_voice,
        set_bus_volume: set_audio_bus_volume,
        set_bus_muted: set_audio_bus_muted,
        read: read_audio,
        read_voice: read_audio_voice,
        read_bus: read_audio_bus,
        read_diagnostic_at: read_audio_diagnostic_at,
        read_realization: read_audio_realization,
        read_realization_fact_at: read_audio_realization_fact_at,
    }
}

unsafe extern "C" fn destroy_operation_diagnostic_lease(
    context: *mut c_void,
    handle: NativeEngineDiagnosticLeaseHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    bridge.operation_diagnostics.destroy(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav() -> Arc<[u8]> {
        let mut bytes = vec![0_u8; 48];
        bytes[..4].copy_from_slice(b"RIFF");
        bytes[4..8].copy_from_slice(&40_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(b"WAVE");
        bytes[12..16].copy_from_slice(b"fmt ");
        bytes[16..20].copy_from_slice(&16_u32.to_le_bytes());
        bytes[20..22].copy_from_slice(&1_u16.to_le_bytes());
        bytes[22..24].copy_from_slice(&1_u16.to_le_bytes());
        bytes[24..28].copy_from_slice(&4_u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&4_u32.to_le_bytes());
        bytes[32..34].copy_from_slice(&1_u16.to_le_bytes());
        bytes[34..36].copy_from_slice(&8_u16.to_le_bytes());
        bytes[36..40].copy_from_slice(b"data");
        bytes[40..44].copy_from_slice(&4_u32.to_le_bytes());
        Arc::from(bytes)
    }

    #[test]
    fn mixed_containers_share_encoded_byte_count_and_owner_budgets() {
        let samples: &[(&str, &[u8])] = &[
            (
                "wav",
                include_bytes!("../../../../fixtures/audio-containers/tone.wav"),
            ),
            (
                "ogg",
                include_bytes!("../../../../fixtures/audio-containers/tone.ogg"),
            ),
            (
                "opus",
                include_bytes!("../../../../fixtures/audio-containers/tone.opus"),
            ),
            (
                "mp3",
                include_bytes!("../../../../fixtures/audio-containers/tone.mp3"),
            ),
            (
                "flac",
                include_bytes!("../../../../fixtures/audio-containers/tone.flac"),
            ),
        ];
        let mut bridge = RuntimeAudioBridge::new(BTreeMap::new());
        bridge.begin_call();
        for (extension, bytes) in samples {
            let path = format!("audio/tone.{extension}");
            let first = bridge.admit_clip(path.clone(), Arc::from(*bytes)).unwrap();
            assert_eq!(
                bridge.admit_clip(path, Arc::from(*bytes)).unwrap().value,
                first.value
            );
        }
        let state = &bridge.staged_ref().unwrap().state;
        assert_eq!(state.clips.len(), 5);
        assert_eq!(
            state
                .clips
                .values()
                .map(|c| c.resource.bytes().len())
                .sum::<usize>(),
            samples.iter().map(|(_, b)| b.len()).sum::<usize>()
        );
        for n in 5..MAX_AUDIO_RESOURCE_COUNT {
            let mut bytes = samples[3].1.to_vec();
            bytes.extend_from_slice(&(n as u64).to_le_bytes());
            bridge
                .admit_clip(format!("audio/{n}.mp3"), Arc::from(bytes))
                .unwrap();
        }
        let mut extra = samples[3].1.to_vec();
        extra.push(255);
        assert_eq!(
            bridge
                .admit_clip("audio/extra.mp3".into(), Arc::from(extra))
                .unwrap_err()
                .code(),
            "CSHARP_AUDIO_RESOURCE_COUNT"
        );
        assert_eq!(
            bridge.staged_ref().unwrap().state.clips.len(),
            MAX_AUDIO_RESOURCE_COUNT
        );

        let mut bridge = RuntimeAudioBridge::new(BTreeMap::new());
        bridge.begin_call();
        for (index, (extension, source)) in samples.iter().take(4).enumerate() {
            let mut bytes = source.to_vec();
            bytes.resize(MAX_AUDIO_RESOURCE_BYTES, index as u8);
            bridge
                .admit_clip(format!("audio/full.{extension}"), Arc::from(bytes))
                .unwrap();
        }
        assert_eq!(
            bridge
                .admit_clip("audio/extra.flac".into(), Arc::from(samples[4].1))
                .unwrap_err()
                .code(),
            "CSHARP_AUDIO_RESOURCE_TOTAL_SIZE"
        );
        let mut oversized = samples[4].1.to_vec();
        oversized.resize(MAX_AUDIO_RESOURCE_BYTES + 1, 0);
        assert_eq!(
            bridge
                .admit_clip("audio/large.flac".into(), Arc::from(oversized))
                .unwrap_err()
                .code(),
            "CSHARP_AUDIO_RESOURCE_SIZE"
        );
        assert_eq!(
            bridge
                .admit_clip(
                    "audio/unknown.aac".into(),
                    Arc::from(b"not audio".as_slice())
                )
                .unwrap_err()
                .code(),
            "CSHARP_AUDIO_RESOURCE_CONTAINER"
        );
    }

    fn descriptor(clip: NativeAudioClipHandle, bus: NativeAudioBus) -> NativeAudioSourceDescriptor {
        NativeAudioSourceDescriptor {
            clip,
            bus,
            volume: 0.5,
            pitch: 1.0,
            looping: false,
            spatial_blend: 0.0,
            attenuation: 32.0,
            pan: 0.0,
            emitter_kind: NativeAudioEmitterKind::Global2d,
            position: NativeVec3::default(),
            entity: 0,
            offset: NativeVec3::default(),
        }
    }

    #[test]
    fn stages_admitted_audio_one_shots_and_retained_voice_lifetimes() {
        let mut content = BTreeMap::new();
        content.insert("audio/trial.wav".to_owned(), wav());
        let mut bridge = RuntimeAudioBridge::new(content);
        bridge.begin_call();
        let path = b"content/audio/trial.wav";
        let request = NativeAudioClipRequest {
            path: NativeUtf8Slice {
                bytes: path.as_ptr(),
                len: path.len(),
            },
        };
        let clip = bridge.open_clip(&request).expect("admitted WAV clip");
        assert_eq!(bridge.open_clip(&request).expect("idempotent path"), clip);
        bridge
            .emit(NativeAudioEmitRequest {
                signal_id: NativeUtf8Slice {
                    bytes: b"trial-one-shot".as_ptr(),
                    len: b"trial-one-shot".len(),
                },
                descriptor: descriptor(clip, NativeAudioBus::Ui),
            })
            .expect("one-shot");
        let signal = bridge
            .emit(NativeAudioEmitRequest {
                signal_id: NativeUtf8Slice {
                    bytes: b"trial-one-shot-2".as_ptr(),
                    len: b"trial-one-shot-2".len(),
                },
                descriptor: descriptor(clip, NativeAudioBus::Ui),
            })
            .expect("Engine-issued one-shot signal");
        assert_eq!(signal.value, 2);
        let voice = bridge
            .create_voice(descriptor(clip, NativeAudioBus::Sfx))
            .expect("retained voice");
        bridge
            .update_voice(NativeAudioVoiceUpdateRequest {
                voice,
                descriptor: descriptor(clip, NativeAudioBus::Sfx),
            })
            .expect("parameter update");
        let replacement = bridge
            .replace_voice(NativeAudioVoiceReplaceRequest {
                voice,
                descriptor: descriptor(clip, NativeAudioBus::Ambient),
            })
            .expect("replace voice");
        bridge
            .destroy_voice(voice)
            .expect("old replacement tombstone releases safely");
        let readout = bridge.read().expect("projector readout");
        assert_eq!(readout.admitted_clips, 1);
        assert_eq!(readout.active_voices, 1);
        assert_eq!(readout.emitted_signals, 2);
        let staged = bridge.take_staged_call().expect("staged call");
        assert_eq!(staged.frame.as_ref().expect("audio frame").ops.len(), 6);
        bridge.commit(staged);
        bridge.seal_resource_selection();
        assert_eq!(bridge.render_resources().count(), 1);
        bridge.begin_call();
        bridge
            .destroy_voice(replacement)
            .expect("replacement release");
        assert_eq!(bridge.read().expect("post-stop readout").active_voices, 0);
    }

    #[test]
    fn optional_preload_skips_missing_and_capacity_without_faulting_the_call_then_admits() {
        let mut content = BTreeMap::new();
        content.insert("audio/valid.wav".to_owned(), wav());
        let mut valid_oversized = wav().to_vec();
        valid_oversized.resize(MAX_AUDIO_RESOURCE_BYTES + 1, 0);
        content.insert("audio/too-large.wav".to_owned(), Arc::from(valid_oversized));
        content.insert(
            "audio/corrupt.wav".to_owned(),
            Arc::from(vec![0_u8; MAX_AUDIO_RESOURCE_BYTES + 1]),
        );
        let sink =
            RuntimeDiagnosticsSink::new(Default::default()).expect("bounded diagnostics sink");
        let mut bridge = RuntimeAudioBridge::new(content);
        bridge.bind_diagnostics_sink(sink.clone());
        bridge.begin_call();
        let request = |path: &'static [u8]| NativeAudioClipRequest {
            path: NativeUtf8Slice {
                bytes: path.as_ptr(),
                len: path.len(),
            },
        };

        let missing = bridge
            .preload_optional(&request(b"content/audio/missing.wav"))
            .expect("missing optional content is a receipt");
        assert_eq!(
            missing.outcome,
            NativeAudioOptionalPreloadOutcome::SkippedMissing
        );
        assert_eq!(missing.clip.value, 0);
        assert!(
            bridge
                .preload_optional(&request(b"content/audio/corrupt.wav"))
                .is_err(),
            "a present corrupt optional resource remains a strict admission failure"
        );
        let capacity = bridge
            .preload_optional(&request(b"content/audio/too-large.wav"))
            .expect("capacity pressure is a receipt");
        assert_eq!(
            capacity.outcome,
            NativeAudioOptionalPreloadOutcome::SkippedCapacity
        );
        let admitted = bridge
            .preload_optional(&request(b"content/audio/valid.wav"))
            .expect("later valid preload continues the same product call");
        assert_eq!(
            admitted.outcome,
            NativeAudioOptionalPreloadOutcome::Admitted
        );
        assert_ne!(admitted.clip.value, 0);
        assert_eq!(admitted.admitted_clip_count, 1);
        bridge
            .take_staged_call()
            .expect("recoverable preload outcomes do not abort the callback");
        let codes = sink
            .snapshot()
            .events
            .into_iter()
            .map(|event| event.code().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            codes,
            vec![
                "CSHARP_AUDIO_PRELOAD_SKIPPED_MISSING",
                "CSHARP_AUDIO_PRELOAD_SKIPPED_CAPACITY",
            ]
        );

        let mut required = RuntimeAudioBridge::new(BTreeMap::new());
        required.begin_call();
        assert!(
            required
                .open_clip(&request(b"content/audio/missing.wav"))
                .is_err(),
            "the required admission path remains strict"
        );
    }

    #[test]
    fn coalesces_repeated_projector_diagnostics_without_eviction() {
        let mut content = BTreeMap::new();
        content.insert("audio/trial.wav".to_owned(), wav());
        let mut bridge = RuntimeAudioBridge::new(content);
        bridge.begin_call();
        let path = b"content/audio/trial.wav";
        let clip = bridge
            .open_clip(&NativeAudioClipRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .expect("admitted WAV clip");
        let request = NativeAudioEmitRequest {
            signal_id: NativeUtf8Slice {
                bytes: b"duplicate-signal".as_ptr(),
                len: b"duplicate-signal".len(),
            },
            descriptor: descriptor(clip, NativeAudioBus::Ui),
        };
        bridge.emit(request).expect("initial one-shot");
        for _ in 0..MAX_AUDIO_DIAGNOSTICS + 2 {
            assert!(
                bridge.emit(request).is_err(),
                "duplicate signal is diagnostic"
            );
        }

        let readout = bridge.read().expect("diagnostic readout");
        assert_eq!(readout.retained_diagnostic_count, 1);
        assert_eq!(readout.evicted_diagnostic_count, 0);
        assert!(
            bridge
                .read_diagnostic_at(NativeAudioDiagnosticAtRequest { index: 0 })
                .expect("oldest retained diagnostic")
                .present
        );
        assert!(
            !bridge
                .read_diagnostic_at(NativeAudioDiagnosticAtRequest { index: 1 })
                .expect("out-of-window diagnostic")
                .present
        );
    }

    #[test]
    fn stages_typed_voice_and_fixed_bus_controls_with_projector_readouts() {
        let mut content = BTreeMap::new();
        content.insert("audio/trial.wav".to_owned(), wav());
        let mut bridge = RuntimeAudioBridge::new(content);
        bridge.begin_call();
        let path = b"content/audio/trial.wav";
        let clip = bridge
            .open_clip(&NativeAudioClipRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .expect("admitted WAV clip");
        let voice = bridge
            .create_voice(descriptor(clip, NativeAudioBus::Sfx))
            .expect("retained voice");

        bridge
            .control_voice(voice, NativeAudioVoiceControl::Pause)
            .expect("pause retained voice");
        assert_eq!(bridge.read().expect("paused voice count").paused_voices, 1);
        assert_eq!(
            bridge
                .read_voice(voice)
                .expect("point voice readout")
                .desired_state,
            NativeAudioVoiceDesiredState::Paused
        );
        assert!(
            bridge
                .read_voice(voice)
                .expect("live voice is present")
                .present
        );
        assert!(
            !bridge
                .read_voice(NativeAudioVoiceHandle { value: 99 })
                .expect("tombstone point readout")
                .present
        );

        bridge
            .control_voice(voice, NativeAudioVoiceControl::Resume)
            .expect("resume retained voice");
        bridge
            .control_voice(voice, NativeAudioVoiceControl::Retrigger)
            .expect("retrigger retained voice");
        assert_eq!(bridge.read().expect("resumed voice count").paused_voices, 0);
        assert_eq!(
            bridge
                .read_voice(voice)
                .expect("resumed point readout")
                .desired_state,
            NativeAudioVoiceDesiredState::Playing
        );

        bridge
            .set_bus_volume(NativeAudioBus::Ui, 0.25)
            .expect("set fixed bus volume");
        bridge
            .set_bus_muted(NativeAudioBus::Ui, true)
            .expect("set fixed bus mute");
        assert_eq!(
            bridge
                .read_bus(NativeAudioBus::Ui)
                .expect("fixed bus readout"),
            NativeAudioBusReadout {
                volume: 0.25,
                muted: true,
            }
        );
        assert_eq!(
            diagnostic_code(AudioProjectionDiagnosticCode::InvalidControl),
            NativeAudioDiagnosticCode::InvalidControl
        );

        let staged = bridge.take_staged_call().expect("staged controls");
        assert_eq!(staged.frame.expect("audio frame").ops.len(), 6);
    }

    #[test]
    fn baseline_recreates_retained_audio_without_replaying_one_shots() {
        let mut content = BTreeMap::new();
        content.insert("audio/trial.wav".to_owned(), wav());
        let mut bridge = RuntimeAudioBridge::new(content);
        bridge.begin_call();
        let path = b"content/audio/trial.wav";
        let clip = bridge
            .open_clip(&NativeAudioClipRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .expect("admitted clip");
        let voice = bridge
            .create_voice(descriptor(clip, NativeAudioBus::Sfx))
            .expect("retained voice");
        bridge
            .control_voice(voice, NativeAudioVoiceControl::Pause)
            .expect("paused retained voice");
        bridge
            .set_bus_muted(NativeAudioBus::Ui, true)
            .expect("muted UI bus");
        bridge
            .emit(NativeAudioEmitRequest {
                signal_id: NativeUtf8Slice {
                    bytes: b"historical-one-shot".as_ptr(),
                    len: b"historical-one-shot".len(),
                },
                descriptor: descriptor(clip, NativeAudioBus::Ui),
            })
            .expect("one-shot remains historical");
        let call = bridge.take_staged_call().expect("committed audio call");
        bridge.commit(call);

        let before = bridge.state.projector.readout();
        let baseline = bridge.snapshot_frame().expect("retained baseline");
        assert!(baseline.ops.iter().all(|op| !matches!(
            op,
            PresentationOp::Audio {
                op: AudioProjectionOp::Emit { .. },
                ..
            }
        )));
        assert!(matches!(
            baseline.ops.first(),
            Some(PresentationOp::Audio {
                op: AudioProjectionOp::Restore { .. },
                ..
            })
        ));
        assert_eq!(
            baseline.ops.len(),
            7,
            "one restored voice and three bus states"
        );
        assert_eq!(bridge.state.projector.readout(), before);
    }

    #[test]
    fn staged_update_time_restores_playing_cursor_and_quiets_completed_one_shot_voices() {
        let mut content = BTreeMap::new();
        content.insert("audio/trial.wav".to_owned(), wav());
        let mut bridge = RuntimeAudioBridge::new(content);
        bridge.begin_call();
        let path = b"content/audio/trial.wav";
        let clip = bridge
            .open_clip(&NativeAudioClipRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .expect("admitted clip");
        let voice = bridge
            .create_voice(descriptor(clip, NativeAudioBus::Sfx))
            .expect("retained one-shot voice");
        let initial = bridge.take_staged_call().expect("initial call");
        bridge.commit(initial);

        bridge.begin_update_call(0.75);
        let staged = bridge.take_staged_call().expect("staged elapsed state");
        let baseline = RuntimeAudioBridge::snapshot_call_frame(&staged).expect("baseline");
        assert!(matches!(
            &baseline.ops[0],
            PresentationOp::Audio { op: AudioProjectionOp::Restore {
                handle, desired_state: AudioVoiceDesiredState::Playing, cursor_seconds, ..
            }, .. } if *handle == AudioHandle::new(voice.value) && (*cursor_seconds - 0.75).abs() < f64::EPSILON
        ));
        bridge.commit(staged);

        bridge.begin_update_call(0.5);
        let staged = bridge.take_staged_call().expect("completed elapsed state");
        let baseline = RuntimeAudioBridge::snapshot_call_frame(&staged).expect("baseline");
        assert!(matches!(
            &baseline.ops[0],
            PresentationOp::Audio { op: AudioProjectionOp::Restore {
                desired_state: AudioVoiceDesiredState::Paused, cursor_seconds, ..
            }, .. } if (*cursor_seconds - 1.0).abs() < f64::EPSILON
        ));
        bridge.commit(staged);
    }

    #[test]
    fn retains_realization_facts_separately_with_cumulative_evictions() {
        let mut bridge = RuntimeAudioBridge::new(BTreeMap::new());
        bridge
            .ingest_realized_feedback(
                true,
                3,
                [AudioRealizationFact::NaturalCompletionOneShot {
                    fact_id: 4,
                    sequence: 2,
                    signal_handle: 7,
                }],
            )
            .expect("initial owner snapshot");
        bridge.begin_call();
        assert_eq!(
            bridge
                .read_realization()
                .expect("committed realization readout"),
            NativeAudioRealizationReadout {
                retained_fact_count: 1,
                evicted_fact_count: 3,
            }
        );
        assert_eq!(
            bridge
                .read_realization_fact_at(NativeAudioRealizationFactAtRequest { index: 0 })
                .expect("indexed realization fact"),
            NativeAudioRealizationFactAtReceipt {
                present: true,
                kind: NativeAudioRealizationFactKind::NaturalCompletionOneShot,
                fact_id: 4,
                sequence: 2,
                signal_handle: 7,
                voice_value: 0,
                code: NativeAudioDiagnosticCode::None,
            }
        );
        bridge.discard_call();
        // A retry is deduplicated, while a newer browser cumulative eviction
        // count remains visible independently of local store evictions.
        bridge
            .ingest_realized_feedback(
                false,
                5,
                [AudioRealizationFact::NaturalCompletionOneShot {
                    fact_id: 4,
                    sequence: 2,
                    signal_handle: 7,
                }],
            )
            .expect("idempotent retry");
        bridge.begin_call();
        assert_eq!(
            bridge
                .read_realization()
                .expect("updated realization readout")
                .evicted_fact_count,
            5
        );
        bridge.discard_call();
        bridge.reset_realized_feedback();
        bridge.begin_call();
        assert_eq!(
            bridge
                .read_realization()
                .expect("replacement owner readout"),
            NativeAudioRealizationReadout::default()
        );
    }

    #[test]
    fn admits_content_references_after_selection_and_releases_shared_clips() {
        let mut resources = BTreeMap::new();
        resources.insert("audio/bundle.wav".to_owned(), wav());
        let mut content = Box::new(RuntimeContentBridge::new(resources));
        let content_api = crate::content::api(&mut content);
        let mut reference = NativeContentReferenceHandle::default();
        let path = b"audio/bundle.wav";
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

        let mut bridge = RuntimeAudioBridge::new(BTreeMap::new());
        bridge.bind_content(&content);
        bridge.seal_resource_selection();
        bridge.begin_call();
        let first = bridge
            .open_clip_from_content(NativeAudioClipFromContentRequest { content: reference })
            .expect("reference admission after Create selection");
        let second = bridge
            .open_clip_from_content(NativeAudioClipFromContentRequest { content: reference })
            .expect("same immutable clip has another owner");
        assert_eq!(first, second);
        assert_eq!(
            unsafe { (content_api.destroy_reference)(content_api.context, reference) },
            ABI_OK,
            "audio has copied its Engine-owned retained resource"
        );
        let voice = bridge
            .create_voice(descriptor(first, NativeAudioBus::Sfx))
            .expect("retained voice");
        bridge.destroy_clip(first).expect("first owner releases");
        assert!(
            bridge.destroy_clip(second).is_err(),
            "voice keeps final clip owner live"
        );
        bridge.destroy_voice(voice).expect("voice release");
        bridge
            .destroy_clip(second)
            .expect("final owner releases resource");
        let call = bridge.take_staged_call().expect("staged resource release");
        bridge.commit(call);
        assert_eq!(bridge.render_resources().count(), 0);
    }

    #[test]
    fn publication_keeps_same_call_retired_clip_for_its_voice_operations() {
        let mut resources = BTreeMap::new();
        resources.insert("audio/trial.wav".to_owned(), wav());
        let mut bridge = RuntimeAudioBridge::new(resources);
        bridge.begin_call();
        let path = b"content/audio/trial.wav";
        let clip = bridge
            .open_clip(&NativeAudioClipRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .expect("clip");
        let voice = bridge
            .create_voice(descriptor(clip, NativeAudioBus::Sfx))
            .expect("retained voice");
        bridge.destroy_voice(voice).expect("voice release");
        bridge
            .destroy_clip(clip)
            .expect("final clip release after same-call voice operations");

        let call = bridge.take_staged_call().expect("publication call");
        assert_eq!(
            RuntimeAudioBridge::publication_resources(&call).count(),
            1,
            "the staged create and destroy operations still have their clip body"
        );
        assert_eq!(
            call.frame.as_ref().expect("audio frame").ops.len(),
            2,
            "same-call voice create and destroy are both published"
        );
        bridge.commit(call);
        assert_eq!(bridge.render_resources().count(), 0);
    }

    #[test]
    fn waits_for_one_shot_completion_before_releasing_final_clip_owner() {
        let mut resources = BTreeMap::new();
        resources.insert("audio/trial.wav".to_owned(), wav());
        let mut bridge = RuntimeAudioBridge::new(resources);
        bridge.begin_call();
        let path = b"content/audio/trial.wav";
        let clip = bridge
            .open_clip(&NativeAudioClipRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .expect("clip");
        let signal = bridge
            .emit(NativeAudioEmitRequest {
                signal_id: NativeUtf8Slice {
                    bytes: b"release-after-completion".as_ptr(),
                    len: b"release-after-completion".len(),
                },
                descriptor: descriptor(clip, NativeAudioBus::Ui),
            })
            .expect("one-shot");
        let call = bridge.take_staged_call().expect("initial call");
        bridge.commit(call);
        bridge.begin_call();
        assert!(
            bridge.destroy_clip(clip).is_err(),
            "pending one-shot owns final clip"
        );
        bridge.discard_call();
        bridge
            .ingest_realized_feedback(
                false,
                0,
                [AudioRealizationFact::NaturalCompletionOneShot {
                    fact_id: 1,
                    sequence: 0,
                    signal_handle: signal.value,
                }],
            )
            .expect("completion feedback");
        bridge.begin_call();
        bridge
            .destroy_clip(clip)
            .expect("completed signal releases clip");
        let call = bridge.take_staged_call().expect("release call");
        bridge.commit(call);
        assert_eq!(bridge.render_resources().count(), 0);
    }

    #[test]
    fn terminal_one_shot_diagnostic_releases_only_its_matching_clip() {
        let mut resources = BTreeMap::new();
        resources.insert("audio/trial.wav".to_owned(), wav());
        let mut bridge = RuntimeAudioBridge::new(resources);
        bridge.begin_call();
        let path = b"content/audio/trial.wav";
        let clip = bridge
            .open_clip(&NativeAudioClipRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .expect("clip");
        let signal = bridge
            .emit(NativeAudioEmitRequest {
                signal_id: NativeUtf8Slice {
                    bytes: b"failed-one-shot".as_ptr(),
                    len: b"failed-one-shot".len(),
                },
                descriptor: descriptor(clip, NativeAudioBus::Ui),
            })
            .expect("one-shot");
        let call = bridge.take_staged_call().expect("initial call");
        bridge.commit(call);
        bridge
            .ingest_realized_feedback(
                false,
                0,
                [AudioRealizationFact::Diagnostic {
                    fact_id: 1,
                    code: NativeAudioDiagnosticCode::DecodeFailed,
                    sequence: 0,
                    signal_handle: Some(signal.value),
                    voice_handle: None,
                }],
            )
            .expect("terminal signal diagnostic");
        bridge.begin_call();
        assert_eq!(
            bridge
                .read_realization_fact_at(NativeAudioRealizationFactAtRequest { index: 0 })
                .expect("diagnostic readout")
                .signal_handle,
            signal.value,
            "the public realization receipt retains the terminal signal identity"
        );
        bridge
            .destroy_clip(clip)
            .expect("signal-specific terminal diagnostic releases clip");
    }

    #[test]
    fn admits_distinct_content_versions_at_one_path_as_distinct_audio_assets() {
        let mut bridge = RuntimeAudioBridge::new(BTreeMap::new());
        let first = wav();
        let mut second = first.to_vec();
        second[44] = 7;
        bridge.begin_call();
        let first = bridge
            .admit_clip("audio/revision.wav".to_owned(), first)
            .expect("first content version");
        let second = bridge
            .admit_clip("audio/revision.wav".to_owned(), Arc::from(second))
            .expect("second content version");
        assert_ne!(
            first, second,
            "path does not alias immutable content versions"
        );
        let state = &bridge.staged_ref().expect("audio call").state;
        assert_eq!(state.assets.len(), 2);
        assert_ne!(
            state.clips[&first.value].asset, state.clips[&second.value].asset,
            "render assets are keyed by immutable body identity"
        );
    }

    #[test]
    fn feedback_overflow_requires_owner_reset_before_pending_one_shot_release() {
        let mut resources = BTreeMap::new();
        resources.insert("audio/trial.wav".to_owned(), wav());
        let mut bridge = RuntimeAudioBridge::new(resources);
        bridge.begin_call();
        let path = b"content/audio/trial.wav";
        let clip = bridge
            .open_clip(&NativeAudioClipRequest {
                path: NativeUtf8Slice {
                    bytes: path.as_ptr(),
                    len: path.len(),
                },
            })
            .expect("clip");
        bridge
            .emit(NativeAudioEmitRequest {
                signal_id: NativeUtf8Slice {
                    bytes: b"lost-terminal-fact".as_ptr(),
                    len: b"lost-terminal-fact".len(),
                },
                descriptor: descriptor(clip, NativeAudioBus::Ui),
            })
            .expect("one-shot");
        let call = bridge.take_staged_call().expect("initial call");
        bridge.commit(call);
        bridge
            .ingest_realized_feedback(false, 1, [])
            .expect("host feedback overflow observation");
        bridge.begin_call();
        let error = bridge
            .destroy_clip(clip)
            .expect_err("unknown terminal fact stays live");
        assert_eq!(error.code(), "CSHARP_AUDIO_CLIP_FEEDBACK_LOST");
        bridge.discard_call();
        bridge.reset_realized_feedback();
        bridge.begin_call();
        bridge
            .destroy_clip(clip)
            .expect("owner reset cancels outstanding browser one-shots");
    }
}
