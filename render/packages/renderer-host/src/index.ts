// @rusty-engine/renderer-host public barrel.

export * from './surface.js';
export * from './animated-mesh-capture.js';
export * from './presentation-host-set.js';
export * from './browser-dom-hosts.js';

export {
  RUSTY_RENDERER_SURFACE_ADMISSION_HISTORY_LIMIT,
} from './surface-admission-observation.js';
export type {
  RendererSurfaceAutomaticSubmissionAdmissionBackend,
  RendererSurfaceAutomaticSubmissionAdmissionAttempt,
  RendererSurfaceAutomaticSubmissionAdmissionOutcome,
  RendererSurfaceAutomaticSubmissionAdmissionSample,
  RendererSurfaceAutomaticSubmissionCallbackPhases,
} from './surface-admission-observation.js';

export {
  RUSTY_RENDERER_SURFACE_MAX_TIMING_DURATION_MS,
  RUSTY_RENDERER_SURFACE_TIMING_SCHEMA_VERSION,
} from './surface-timing.js';
export type {
  RendererSurfaceFrameIntervalStatus,
  RendererSurfaceSubmissionDurationStatus,
  RendererSurfaceTimingSample,
  RendererSurfaceTimingSource,
} from './surface-timing.js';

export { RUSTY_RENDERER_SURFACE_STATISTICS_SCHEMA_VERSION } from './surface-statistics.js';
export type {
  RendererSurfaceStatistic,
  RendererSurfaceStatisticScope,
  RendererSurfaceStatisticStatus,
  RendererSurfaceStatisticsSample,
  RendererSurfaceSubmissionSample,
} from './surface-statistics.js';

export {
  RUSTY_RENDERER_EDITOR_VIEWPORT_CHANNEL_POLICIES,
  RUSTY_RENDERER_EDITOR_VIEWPORT_COMPATIBILITY_VERSION,
  RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_FRAME_OPS,
  RUSTY_RENDERER_EDITOR_VIEWPORT_MAX_RETAINED_OPS,
  mountRendererEditorViewport,
} from './editor-viewport.js';
export type {
  RendererEditorViewport,
  RendererEditorViewportBufferSource,
  RendererEditorViewportCamera,
  RendererEditorViewportCameraReceipt,
  RendererEditorViewportCameraSource,
  RendererEditorViewportChannel,
  RendererEditorViewportChannelHandle,
  RendererEditorViewportChannelPolicy,
  RendererEditorViewportChannelReceipt,
  RendererEditorViewportChannelSnapshot,
  RendererEditorViewportDiagnostic,
  RendererEditorViewportDiagnosticCode,
  RendererEditorViewportGridReceipt,
  RendererEditorViewportOptions,
  RendererEditorViewportPickFilter,
  RendererEditorViewportPickHint,
  RendererEditorViewportPickReceipt,
  RendererEditorViewportPickRequest,
  RendererEditorViewportReadout,
  RendererEditorViewportVisibilityReadout,
  RendererEditorViewportSize,
  RendererEditorViewportSizeReceipt,
  RendererEditorViewportStatus,
} from './editor-viewport.js';

export {
  RUSTY_RENDERER_INSPECTION_SURFACE_COMPATIBILITY_VERSION,
  mountRendererInspectionSurface,
} from './inspection-surface.js';
export type {
  RendererInspectionCameraChange,
  RendererInspectionSurface,
  RendererInspectionSurfaceControlPreferences,
  RendererInspectionSurfaceControlsOptions,
  RendererInspectionSurfaceKeyboardBindings,
  RendererInspectionSurfaceOptions,
  RendererInspectionSurfaceReadout,
  RendererInspectionSurfaceStatus,
} from './inspection-surface.js';

export { resolveRendererStoredEditorCamera } from './stored-editor-camera.js';
export type {
  RendererStoredEditorCameraDiagnostic,
  RendererStoredEditorCameraDiagnosticCode,
  RendererStoredEditorCameraInput,
  RendererStoredEditorCameraResolution,
} from './stored-editor-camera.js';

export { sampleCameraTransition } from './camera-transition.js';

export {
  RendererHostError,
  createRendererAnimatedMeshProjection,
  validEmbeddedMaterialSlots,
} from './animated-mesh-host.js';

export {
  RUSTY_RENDERER_MESH_RESOURCE_MAX_BYTES,
  RUSTY_RENDERER_MESH_RESOURCE_MAX_COUNT,
  RUSTY_RENDERER_MESH_RESOURCE_MAX_TOTAL_BYTES,
  RendererMeshResourceError,
  loadRendererMeshResourceSource,
} from './mesh-resource-host.js';
export {
  RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_BYTES,
  RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_COUNT,
  RUSTY_RENDERER_TEXTURE_RESOURCE_MAX_TOTAL_BYTES,
  RendererTextureResourceError,
  loadRendererTextureResourceSource,
} from './texture-resource-host.js';
export type {
  RendererTextureResourceDescriptor,
  RendererTextureResourceErrorCode,
  RendererTextureResourceManifest,
  RendererTextureResourceResolver,
} from './texture-resource-host.js';
export type {
  RendererMeshResourceDescriptor,
  RendererMeshResourceErrorCode,
  RendererMeshResourceManifest,
  RendererMeshResourceResolver,
} from './mesh-resource-host.js';
export type {
  RendererAnimationControllerClip,
  RendererAnimationClipPackResourceDescriptor,
  RendererAnimatedMeshFrameReceipt,
  RendererAnimatedMeshPlaybackReadout,
  RendererAnimatedMeshEffectiveClip,
  RendererAnimatedMeshPoseSample,
  RendererAnimatedMeshProjection,
  RendererAnimatedMeshProjectionOptions,
  RendererAnimatedMeshResourceDescriptor,
  RendererAnimatedMeshResourceManifest,
  RendererAnimatedMeshResourceResolver,
  RendererHostDiagnostic,
  RendererHostDiagnosticCode,
} from './animated-mesh-host.js';

export { RendererAnimationHost } from './animation-host.js';
export type {
  RendererAnimationClipCueDefinition,
  RendererAnimationCueSignalDomain,
  RendererAnimationFrameReceipt,
  RendererAnimationHostOptions,
  RendererAnimationSampledCue,
} from './animation-host.js';

export { RendererAudioHost } from './audio-host.js';
export type {
  RendererAudioContext,
  RendererAudioEntityPositionResolver,
  RendererAudioFrameReceipt,
  RendererAudioHostOptions,
  RendererAudioRealizedFact,
  RendererAudioRealizedFactsReadout,
  RendererAudioResource,
  RendererAudioResourceResolver,
} from './audio-host.js';

export { RendererBillboardHost } from './billboard-host.js';
export type {
  RendererBillboardContainer,
  RendererBillboardContainerPort,
  RendererBillboardElement,
  RendererBillboardElementFactory,
  RendererBillboardElementStyle,
  RendererBillboardEntityPositionResolver,
  RendererBillboardFontLoader,
  RendererBillboardFrameReceipt,
  RendererBillboardHostOptions,
  RendererBillboardLocalizer,
  RendererBillboardResource,
  RendererBillboardResourceResolver,
  RendererBillboardScreenProjection,
  RendererBillboardWorldProjector,
} from './billboard-host.js';

export { RendererParticleHost } from './particle-host.js';
export type {
  RendererParticleBillboard,
  RendererParticleBillboardSink,
  RendererParticleEntityPositionResolver,
  RendererParticleFrameReceipt,
  RendererParticleHostOptions,
  RendererParticleInstance,
  RendererParticlePreparedVisual,
  RendererParticleResource,
  RendererParticleResourceResolver,
  RendererParticleSceneSink,
  RendererParticleSink,
  RendererParticleSinkReadout,
} from './particle-host.js';

export {
  RendererLiveTelemetryCollector,
  RendererTelemetryOverlayHost,
} from './telemetry-host.js';
export type {
  RendererLiveTelemetryCollectorOptions,
  RendererLiveTelemetrySample,
  RendererSurfaceProductTelemetryCounter,
  RendererSurfaceTelemetrySample,
  RendererTelemetryOverlayFrameReceipt,
  RendererTelemetryOverlayHostOptions,
  RendererTelemetryOverlaySink,
} from './telemetry-host.js';

export type {
  AnimationProjectionDiagnostic,
  AnimationProjectionDiagnosticCode,
  AnimationProjectionReadout,
  AudioProjectionDiagnostic,
  AudioProjectionDiagnosticCode,
  AudioProjectionReadout,
  BillboardProjectionDiagnostic,
  BillboardProjectionDiagnosticCode,
  BillboardProjectionReadout,
  LiveTelemetryCounter,
  LiveTelemetryDiagnostic,
  LiveTelemetryMetric,
  LiveTelemetrySnapshot,
  ParticleProjectionDiagnostic,
  ParticleProjectionDiagnosticCode,
  ParticleProjectionReadout,
  RendererCameraSnapshot,
  RendererCameraTransitionReadout,
  TelemetryMetricKind,
  TelemetryOverlayDiagnostic,
  TelemetryOverlayDiagnosticCode,
  TelemetryOverlayReadout,
} from './host-types.js';
