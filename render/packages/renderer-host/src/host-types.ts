import type {
  AnimationProjectionHandle,
  AudioHandle,
  BillboardHandle,
  CameraBasis,
  CameraPose,
  ParticleEmitterHandle,
  PerspectiveProjection,
  RenderHandle,
  TelemetryOverlayHandle,
} from '@rusty-engine/render-contracts';

export type AudioProjectionDiagnosticCode =
  | 'invalidDescriptor'
  | 'assetMissing'
  | 'assetKindMismatch'
  | 'contentHashMismatch'
  | 'duplicateSignal'
  | 'duplicateHandle'
  | 'unknownHandle'
  | 'unavailableHost'
  | 'audioContextBlocked'
  | 'decodeFailed'
  | 'hostFailure';

export interface AudioProjectionDiagnostic {
  readonly code: AudioProjectionDiagnosticCode;
  readonly sequence: number;
  readonly handle: AudioHandle | null;
  readonly message: string;
}

export interface AudioProjectionReadout {
  readonly activeSources: number;
  readonly cachedClips: number;
  readonly emittedSignals: number;
  readonly diagnostics: readonly AudioProjectionDiagnostic[];
}

export type BillboardProjectionDiagnosticCode =
  | 'invalidDescriptor'
  | 'assetMissing'
  | 'assetKindMismatch'
  | 'contentHashMismatch'
  | 'duplicateHandle'
  | 'unknownHandle'
  | 'anchorMissing'
  | 'unavailableHost'
  | 'fontLoadFailed'
  | 'iconLoadFailed'
  | 'hostFailure';

export interface BillboardProjectionDiagnostic {
  readonly code: BillboardProjectionDiagnosticCode;
  readonly sequence: number;
  readonly handle: BillboardHandle | null;
  readonly message: string;
}

export interface BillboardProjectionReadout {
  readonly activeBillboards: number;
  readonly loadedFonts: number;
  readonly loadedIcons: number;
  readonly culledBillboards: number;
  readonly diagnostics: readonly BillboardProjectionDiagnostic[];
}

export type ParticleProjectionDiagnosticCode =
  | 'invalidDescriptor'
  | 'assetMissing'
  | 'assetKindMismatch'
  | 'contentHashMismatch'
  | 'duplicateSignal'
  | 'duplicateHandle'
  | 'unknownHandle'
  | 'anchorMissing'
  | 'budgetExceeded'
  | 'unavailableHost'
  | 'spriteLoadFailed'
  | 'hostFailure';

export interface ParticleProjectionDiagnostic {
  readonly code: ParticleProjectionDiagnosticCode;
  readonly sequence: number;
  readonly handle: ParticleEmitterHandle | null;
  readonly message: string;
}

export interface ParticleProjectionReadout {
  readonly activeEmitters: number;
  readonly activeParticles: number;
  readonly loadedSprites: number;
  readonly emittedBursts: number;
  readonly droppedParticles: number;
  readonly diagnostics: readonly ParticleProjectionDiagnostic[];
}

export type TelemetryOverlayDiagnosticCode =
  | 'invalidDescriptor'
  | 'duplicateHandle'
  | 'unknownHandle'
  | 'unavailableHost'
  | 'snapshotUnavailable'
  | 'hostFailure';

export interface TelemetryOverlayDiagnostic {
  readonly code: TelemetryOverlayDiagnosticCode;
  readonly sequence: number;
  readonly handle: TelemetryOverlayHandle | null;
  readonly message: string;
}

export interface TelemetryOverlayReadout {
  readonly activeOverlays: number;
  readonly renderedSnapshots: number;
  readonly diagnostics: readonly TelemetryOverlayDiagnostic[];
}

export type AnimationProjectionDiagnosticCode =
  | 'invalidDescriptor'
  | 'duplicateHandle'
  | 'unknownHandle'
  | 'unknownTarget'
  | 'assetMissing'
  | 'contentHashMismatch'
  | 'clipMissing'
  | 'incompatibleRig'
  | 'invalidBlendWeight'
  | 'invalidTransition'
  | 'staleRevision'
  | 'unavailableHost'
  | 'compatibilityFallback'
  | 'hostFailure';

export interface AnimationProjectionDiagnostic {
  readonly code: AnimationProjectionDiagnosticCode;
  readonly sequence: number;
  readonly handle: AnimationProjectionHandle | null;
  readonly target: RenderHandle | null;
  readonly message: string;
}

export interface AnimationProjectionReadout {
  readonly activeControllers: number;
  readonly sampledFrames: number;
  readonly compatibilityFallbacks: number;
  readonly diagnostics: readonly AnimationProjectionDiagnostic[];
}

export type LiveTelemetryCounter =
  | 'frameTimeMs'
  | 'backendSubmissionDurationMs'
  | 'entityCount'
  | 'activeCapabilityCount'
  | 'residentChunkCount'
  | 'dirtyChunkCount'
  | 'renderDiffCount'
  | 'renderHandleCount'
  | 'drawCallCount'
  | 'geometryResourceCount'
  | 'materialResourceCount'
  | 'textureResourceCount'
  | 'animatedInstanceCount'
  | 'triangleCount'
  | 'activeAudioSourceCount'
  | 'activeBillboardCount'
  | 'activeParticleCount'
  | 'droppedFeedbackCount';

export type TelemetryMetricKind = 'durationMs' | 'gauge';

export interface LiveTelemetryMetric {
  readonly counter: LiveTelemetryCounter;
  readonly kind: TelemetryMetricKind;
  readonly value: number;
  readonly unit: string;
}

export interface LiveTelemetryDiagnostic {
  readonly code: 'counterUnavailable' | 'invalidSample';
  readonly counter: LiveTelemetryCounter | null;
  readonly message: string;
}

export interface LiveTelemetrySnapshot {
  readonly schemaVersion: 1;
  readonly sourceTick: number;
  readonly sampleSequence: number;
  readonly metrics: readonly LiveTelemetryMetric[];
  readonly frameTimeHistoryMs: readonly number[];
  readonly diagnostics: readonly LiveTelemetryDiagnostic[];
}

export interface RendererCameraSnapshot {
  readonly pose: CameraPose;
  readonly basis: CameraBasis;
  readonly projection: PerspectiveProjection;
  readonly viewport: { readonly width: number; readonly height: number };
}

export interface RendererCameraTransitionReadout {
  readonly from: RendererCameraSnapshot;
  readonly to: RendererCameraSnapshot;
  readonly durationMilliseconds: number;
  readonly easing: 'linear' | 'smoothStep';
}
