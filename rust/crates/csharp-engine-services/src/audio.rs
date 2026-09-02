use std::{
    collections::{BTreeMap, VecDeque},
    ffi::c_void,
    sync::Arc,
};

use csharp_engine_abi::*;
use render_model::{RenderAssetKind, ResolvedRenderAsset, JSON_SAFE_U64_MAX};
use render_presentation::{
    AudioBus, AudioBusControl, AudioEmitter, AudioHandle, AudioProjectionDiagnosticCode,
    AudioProjectionOp, AudioProjector, AudioSourceDescriptor, AudioVoiceControl,
    AudioVoiceDesiredState, PresentationOpMeta,
};

#[cfg(test)]
use render_presentation::MAX_AUDIO_DIAGNOSTICS;

use crate::{
    appearance::CsharpRenderResource,
    composition::{borrowed_utf8, ABI_OK},
    CsharpEngineServicesError,
};

const MAX_AUDIO_RESOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_AUDIO_RESOURCE_COUNT: usize = 64;
const MAX_AUDIO_RESOURCE_TOTAL_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
struct AudioClip {
    asset: String,
    content_hash: String,
    resource: CsharpRenderResource,
}

#[derive(Clone)]
struct AudioState {
    projector: AudioProjector,
    clips: BTreeMap<u64, AudioClip>,
    assets: BTreeMap<String, ResolvedRenderAsset>,
    voices: BTreeMap<u64, AudioHandle>,
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
                voice_handle,
            } => NativeAudioRealizationFactAtReceipt {
                present: true,
                kind: NativeAudioRealizationFactKind::Diagnostic,
                fact_id,
                sequence,
                signal_handle: 0,
                voice_value: voice_handle.unwrap_or(0),
                code,
            },
        }
    }
}

pub(crate) struct RuntimeAudioCall {
    state: AudioState,
    pub(crate) frame: Option<render_presentation::PresentationFrameDiff>,
}

/// Engine-owned audio admission and projector bridge. Audio resource selection
/// is permitted during product Create only; selected WAV bytes are retained by
/// the product runtime and realized only by the Engine browser host.
pub(crate) struct RuntimeAudioBridge {
    state: AudioState,
    content_resources: BTreeMap<String, Arc<[u8]>>,
    selection_sealed: bool,
    staged: Option<RuntimeAudioCall>,
    callback_error: Option<CsharpEngineServicesError>,
    realized_facts: VecDeque<AudioRealizationFact>,
    renderer_evicted_fact_count: u64,
    local_evicted_fact_count: u64,
    accepted_through_fact_id: Option<u64>,
}

impl RuntimeAudioBridge {
    pub(crate) fn new(content_resources: BTreeMap<String, Arc<[u8]>>) -> Self {
        Self {
            state: AudioState {
                projector: AudioProjector::default(),
                clips: BTreeMap::new(),
                assets: BTreeMap::new(),
                voices: BTreeMap::new(),
                next_clip: 1,
                next_voice: 1,
                next_signal: 1,
            },
            content_resources,
            selection_sealed: false,
            staged: None,
            callback_error: None,
            realized_facts: VecDeque::new(),
            renderer_evicted_fact_count: 0,
            local_evicted_fact_count: 0,
            accepted_through_fact_id: None,
        }
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
        } else if evicted_fact_count > self.renderer_evicted_fact_count {
            self.renderer_evicted_fact_count = evicted_fact_count;
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
    }

    pub(crate) fn begin_call(&mut self) {
        self.staged = Some(RuntimeAudioCall {
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
        self.selection_sealed = true;
        self.content_resources.clear();
    }

    pub(crate) fn render_resources(&self) -> impl Iterator<Item = &CsharpRenderResource> {
        self.state.clips.values().map(|clip| &clip.resource)
    }

    fn staged_mut(&mut self) -> Result<&mut RuntimeAudioCall, CsharpEngineServicesError> {
        self.staged.as_mut().ok_or_else(|| {
            CsharpEngineServicesError::new(
                "CSHARP_AUDIO_CALL",
                "audio service was called outside a product call",
            )
        })
    }

    fn open_clip(
        &mut self,
        request: &NativeAudioClipRequest,
    ) -> Result<NativeAudioClipHandle, CsharpEngineServicesError> {
        let requested_path =
            unsafe { borrowed_utf8(request.path.bytes, request.path.len, "audio resource path")? }
                .to_owned();
        let relative_path = requested_path
            .strip_prefix("content/")
            .unwrap_or(&requested_path)
            .to_owned();
        let browser_path = format!("content/{relative_path}");
        if let Some((handle, _)) = self
            .staged_mut()?
            .state
            .clips
            .iter()
            .find(|(_, clip)| clip.resource.path() == browser_path)
        {
            return Ok(NativeAudioClipHandle { value: *handle });
        }
        if self.selection_sealed {
            return Err(CsharpEngineServicesError::new(
                "CSHARP_AUDIO_RESOURCE_SELECTION_CLOSED",
                "audio clips must be selected during product Create",
            ));
        }
        let bytes = self
            .content_resources
            .get(&relative_path)
            .cloned()
            .ok_or_else(|| {
                CsharpEngineServicesError::new(
                    "CSHARP_AUDIO_RESOURCE_UNKNOWN",
                    format!("product content has no audio resource `{requested_path}`"),
                )
            })?;
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
        let resource = CsharpRenderResource::admit_audio(browser_path, bytes.to_vec())?;
        let asset = format!("audio/{relative_path}");
        let content_hash = resource.content_hash().to_owned();
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
                resource,
            },
        );
        Ok(NativeAudioClipHandle { value: handle })
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
        Ok(signal_handle)
    }

    fn create_voice(
        &mut self,
        descriptor: NativeAudioSourceDescriptor,
    ) -> Result<NativeAudioVoiceHandle, CsharpEngineServicesError> {
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
        self.stage_op(AudioProjectionOp::Destroy { handle: old })?;
        self.create_voice_native(descriptor)
    }

    fn create_voice_native(
        &mut self,
        descriptor: AudioSourceDescriptor,
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
        self.stage_op(AudioProjectionOp::Destroy { handle })
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
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn emit_audio(
    context: *mut c_void,
    request: *const NativeAudioEmitRequest,
    result: *mut NativeAudioSignalHandle,
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn create_audio_voice(
    context: *mut c_void,
    request: *const NativeAudioSourceDescriptor,
    result: *mut NativeAudioVoiceHandle,
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn update_audio_voice(
    context: *mut c_void,
    request: *const NativeAudioVoiceUpdateRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.update_voice(unsafe { *request }) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn replace_audio_voice(
    context: *mut c_void,
    request: *const NativeAudioVoiceReplaceRequest,
    result: *mut NativeAudioVoiceHandle,
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn destroy_audio_voice(
    context: *mut c_void,
    voice: NativeAudioVoiceHandle,
) -> i32 {
    if context.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    match bridge.destroy_voice(voice) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn control_audio_voice(
    context: *mut c_void,
    request: *const NativeAudioVoiceControlRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    let request = unsafe { *request };
    match bridge.control_voice(request.voice, request.control) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_audio_bus_volume(
    context: *mut c_void,
    request: *const NativeAudioBusVolumeRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    let request = unsafe { *request };
    match bridge.set_bus_volume(request.bus, request.volume) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn set_audio_bus_muted(
    context: *mut c_void,
    request: *const NativeAudioBusMutedRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return 0;
    }
    let bridge = unsafe { &mut *context.cast::<RuntimeAudioBridge>() };
    let request = unsafe { *request };
    match bridge.set_bus_muted(request.bus, request.muted) {
        Ok(()) => ABI_OK,
        Err(error) => {
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn read_audio(
    context: *mut c_void,
    result: *mut NativeAudioReadout,
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_audio_voice(
    context: *mut c_void,
    request: *const NativeAudioVoiceReadRequest,
    result: *mut NativeAudioVoiceReadout,
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_audio_bus(
    context: *mut c_void,
    request: *const NativeAudioBusReadRequest,
    result: *mut NativeAudioBusReadout,
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
}
pub(crate) unsafe extern "C" fn read_audio_diagnostic_at(
    context: *mut c_void,
    request: NativeAudioDiagnosticAtRequest,
    result: *mut NativeAudioDiagnosticAtReceipt,
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_audio_realization(
    context: *mut c_void,
    result: *mut NativeAudioRealizationReadout,
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) unsafe extern "C" fn read_audio_realization_fact_at(
    context: *mut c_void,
    request: NativeAudioRealizationFactAtRequest,
    result: *mut NativeAudioRealizationFactAtReceipt,
) -> i32 {
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
            bridge.callback_error = Some(error);
            0
        }
    }
}

pub(crate) fn api(bridge: &mut RuntimeAudioBridge) -> NativeAudioApi {
    NativeAudioApi {
        context: (bridge as *mut RuntimeAudioBridge).cast(),
        open_clip: open_audio_clip,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn wav() -> Arc<[u8]> {
        let mut bytes = vec![0_u8; 44];
        bytes[..4].copy_from_slice(b"RIFF");
        bytes[8..12].copy_from_slice(b"WAVE");
        Arc::from(bytes)
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
                .read_diagnostic_at(NativeAudioDiagnosticAtRequest {
                    index: 1,
                })
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
}
