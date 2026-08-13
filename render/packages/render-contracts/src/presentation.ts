import {
  assertJsonSafeUnsignedInteger,
  type RenderHandle,
  type Vec3,
  type Vec4,
} from './render.js';

export type AudioHandle = number & { readonly __brand: 'AudioHandle' };
export type BillboardHandle = number & { readonly __brand: 'BillboardHandle' };
export type ParticleEmitterHandle = number & { readonly __brand: 'ParticleEmitterHandle' };
export type TelemetryOverlayHandle = number & { readonly __brand: 'TelemetryOverlayHandle' };
export type AnimationProjectionHandle = number & { readonly __brand: 'AnimationProjectionHandle' };

export const audioHandle = (raw: number): AudioHandle =>
  assertJsonSafeUnsignedInteger(raw, 'audio handle') as AudioHandle;
export const billboardHandle = (raw: number): BillboardHandle =>
  assertJsonSafeUnsignedInteger(raw, 'billboard handle') as BillboardHandle;
export const particleEmitterHandle = (raw: number): ParticleEmitterHandle =>
  assertJsonSafeUnsignedInteger(raw, 'particle emitter handle') as ParticleEmitterHandle;
export const telemetryOverlayHandle = (raw: number): TelemetryOverlayHandle =>
  assertJsonSafeUnsignedInteger(raw, 'telemetry overlay handle') as TelemetryOverlayHandle;
export const animationProjectionHandle = (raw: number): AnimationProjectionHandle =>
  assertJsonSafeUnsignedInteger(raw, 'animation projection handle') as AnimationProjectionHandle;

export interface PresentationOpMeta {
  readonly sequence: number;
}

export type AudioBus = 'sfx' | 'ambient' | 'ui';
export type AudioEmitter =
  | { readonly kind: 'global2d' }
  | { readonly kind: 'world3d'; readonly position: Vec3 }
  | { readonly kind: 'entityAttached'; readonly entity: number; readonly offset: Vec3 };

export interface AudioClipRef {
  readonly asset: string;
  readonly contentHash: string;
}

export interface AudioSourceDescriptor {
  readonly clip: AudioClipRef;
  readonly bus: AudioBus;
  readonly volume: number;
  readonly pitch: number;
  readonly looping: boolean;
  readonly spatialBlend: number;
  readonly attenuation: number;
  readonly pan: number;
  readonly emitter: AudioEmitter;
}

export interface AudioSourcePatch {
  readonly volume: number | null;
  readonly pitch: number | null;
  readonly looping: boolean | null;
  readonly spatialBlend: number | null;
  readonly attenuation: number | null;
  readonly pan: number | null;
  readonly emitter: AudioEmitter | null;
}

export type AudioProjectionOp =
  | { readonly op: 'emit'; readonly signalId: string; readonly descriptor: AudioSourceDescriptor }
  | { readonly op: 'create'; readonly handle: AudioHandle; readonly descriptor: AudioSourceDescriptor }
  | { readonly op: 'update'; readonly handle: AudioHandle; readonly patch: AudioSourcePatch }
  | { readonly op: 'destroy'; readonly handle: AudioHandle };

export type BillboardAnchor =
  | { readonly kind: 'world'; readonly position: Vec3 }
  | { readonly kind: 'entityAttached'; readonly entity: number; readonly offset: Vec3 };

export interface BillboardTemplateArgument {
  readonly name: string;
  readonly value: string;
}

export interface BillboardTextureRef {
  readonly asset: string;
  readonly contentHash: string;
}

/** A renderer-neutral localized string for structured indicator content. */
export interface BillboardLocalizedText {
  readonly localizationKey: string;
  readonly fallbackText: string;
}

export type BillboardMeterFillDirection =
  | 'leftToRight'
  | 'rightToLeft'
  | 'bottomToTop'
  | 'topToBottom';

export interface BillboardMeter {
  readonly id: string;
  readonly accessibleLabel: BillboardLocalizedText;
  readonly current: number;
  readonly min: number;
  readonly max: number;
  readonly preview: number | null;
  readonly fillDirection: BillboardMeterFillDirection;
  readonly segments: number;
  readonly fill: Vec4;
  readonly previewFill: Vec4;
  readonly back: Vec4;
  readonly border: Vec4;
}

/** A compact localized or icon-backed fact owned by the submitting game. */
export interface BillboardStatusCue {
  readonly id: string;
  readonly label: BillboardLocalizedText;
  readonly icon: BillboardTextureRef | null;
}

export interface BillboardStyle {
  readonly opacity: number;
  readonly backing: Vec4;
  readonly border: Vec4;
  readonly radiusPixels: number;
}

export type BillboardAlignment = 'start' | 'center' | 'end';

export interface BillboardIndicator {
  readonly label: BillboardLocalizedText | null;
  readonly icon: BillboardTextureRef | null;
  readonly accessibleLabel: BillboardLocalizedText;
  readonly meters: readonly BillboardMeter[];
  readonly statusCues: readonly BillboardStatusCue[];
  readonly widthPixels: number;
  readonly spacingPixels: number;
  readonly alignment: BillboardAlignment;
  readonly style: BillboardStyle;
}

export interface BillboardSafeArea {
  readonly topPixels: number;
  readonly rightPixels: number;
  readonly bottomPixels: number;
  readonly leftPixels: number;
}

export type BillboardLayoutSizing =
  | { readonly kind: 'constantPixels' }
  | {
    readonly kind: 'distanceScaled';
    readonly referenceDistance: number;
    readonly minScale: number;
    readonly maxScale: number;
  };

export interface BillboardLayoutPolicy {
  readonly priority: number;
  readonly sizing: BillboardLayoutSizing;
  readonly safeArea: BillboardSafeArea;
  readonly edgeBehavior: 'clamp' | 'cull';
  readonly overlapBehavior: 'stack' | 'suppress';
}

export type BillboardContent =
  | { readonly kind: 'text'; readonly localizationKey: string; readonly fallbackText: string; readonly arguments: readonly BillboardTemplateArgument[] }
  | { readonly kind: 'value'; readonly labelKey: string; readonly fallbackLabel: string; readonly value: string; readonly unitKey: string | null; readonly fallbackUnit: string | null }
  | { readonly kind: 'icon'; readonly texture: BillboardTextureRef; readonly altKey: string; readonly fallbackAlt: string }
  | { readonly kind: 'structured'; readonly indicator: BillboardIndicator };

export type BillboardFontRef =
  | { readonly kind: 'system'; readonly family: string }
  | { readonly kind: 'asset'; readonly asset: string; readonly contentHash: string; readonly family: string };

export type BillboardLayer = 'alwaysOnTop' | 'depthTested' | 'occluded';

export interface BillboardDescriptor {
  readonly anchor: BillboardAnchor;
  readonly content: BillboardContent;
  readonly font: BillboardFontRef;
  readonly heightPixels: number;
  readonly color: Vec4;
  readonly background: Vec4;
  readonly maxDistance: number;
  readonly layer: BillboardLayer;
  readonly visible: boolean;
  /** Legacy descriptors omit layout; structured content must provide it. */
  readonly layout?: BillboardLayoutPolicy;
}

export interface BillboardPatch {
  readonly anchor: BillboardAnchor | null;
  readonly content: BillboardContent | null;
  readonly font: BillboardFontRef | null;
  readonly heightPixels: number | null;
  readonly color: Vec4 | null;
  readonly background: Vec4 | null;
  readonly maxDistance: number | null;
  readonly layer: BillboardLayer | null;
  readonly visible: boolean | null;
  /** Optional for old serialized patches; null is not a serialized value. */
  readonly layout?: BillboardLayoutPolicy;
}

export type BillboardProjectionOp =
  | { readonly op: 'create'; readonly handle: BillboardHandle; readonly descriptor: BillboardDescriptor }
  | { readonly op: 'update'; readonly handle: BillboardHandle; readonly patch: BillboardPatch }
  | { readonly op: 'destroy'; readonly handle: BillboardHandle };

export type ParticleAnchor =
  | { readonly kind: 'world'; readonly position: Vec3 }
  | { readonly kind: 'entityAttached'; readonly entity: number; readonly offset: Vec3 };

export interface ParticleSpriteRef {
  readonly asset: string;
  readonly contentHash: string;
  readonly frameCount: number;
}

export type ParticleVisual =
  | { readonly kind: 'billboard'; readonly sprite: ParticleSpriteRef }
  | { readonly kind: 'cube' };

export type ParticleCollisionLimitBehavior = 'sleep' | 'kill';

export type ParticleCollisionVolume =
  | { readonly kind: 'plane'; readonly normal: Vec3; readonly offset: number }
  | { readonly kind: 'aabb'; readonly minimum: Vec3; readonly maximum: Vec3 };

export interface ParticleCollisionDescriptor {
  readonly radius: number;
  readonly restitution: number;
  readonly friction: number;
  readonly maximumImpacts: number;
  readonly sleepSpeed: number;
  readonly limitBehavior: ParticleCollisionLimitBehavior;
  readonly volumes: readonly ParticleCollisionVolume[];
}

export interface ParticleScalarKey {
  readonly age: number;
  readonly value: number;
}

export interface ParticleColorKey {
  readonly age: number;
  readonly color: Vec4;
}

interface ParticleEmitterDescriptorCommon {
  readonly anchor: ParticleAnchor;
  readonly ratePerSecond: number;
  readonly burstCount: number;
  readonly lifetimeSeconds: readonly [number, number];
  readonly velocityMin: Vec3;
  readonly velocityMax: Vec3;
  readonly acceleration: Vec3;
  readonly sizeCurve: readonly ParticleScalarKey[];
  readonly colorCurve: readonly ParticleColorKey[];
  readonly flipbookFramesPerSecond: number;
  readonly seed: number;
  readonly maxParticles: number;
  readonly visible: boolean;
  /** Omitted by legacy sprite-only v1 frames. */
  readonly collision?: ParticleCollisionDescriptor;
}

/**
 * New frames use `visual`. The `sprite` branch remains decodable so checked
 * v1 frames and existing downstream serialized content continue to run.
 */
export type ParticleEmitterDescriptor = ParticleEmitterDescriptorCommon & (
  | { readonly visual: ParticleVisual; readonly sprite?: never }
  | { readonly visual?: never; readonly sprite: ParticleSpriteRef }
);

export interface ParticleEmitterPatch {
  readonly anchor: ParticleAnchor | null;
  readonly visual?: ParticleVisual | null;
  /** Legacy patch form. Prefer `visual`. */
  readonly sprite: ParticleSpriteRef | null;
  readonly ratePerSecond: number | null;
  readonly burstCount: number | null;
  readonly lifetimeSeconds: readonly [number, number] | null;
  readonly velocityMin: Vec3 | null;
  readonly velocityMax: Vec3 | null;
  readonly acceleration: Vec3 | null;
  readonly sizeCurve: readonly ParticleScalarKey[] | null;
  readonly colorCurve: readonly ParticleColorKey[] | null;
  readonly flipbookFramesPerSecond: number | null;
  readonly maxParticles: number | null;
  readonly visible: boolean | null;
  readonly collision?: ParticleCollisionDescriptor | null;
}

export type ParticleProjectionOp =
  | { readonly op: 'emit'; readonly signalId: string; readonly descriptor: ParticleEmitterDescriptor }
  | { readonly op: 'create'; readonly handle: ParticleEmitterHandle; readonly descriptor: ParticleEmitterDescriptor }
  | { readonly op: 'update'; readonly handle: ParticleEmitterHandle; readonly patch: ParticleEmitterPatch }
  | { readonly op: 'destroy'; readonly handle: ParticleEmitterHandle };

export type TelemetryOverlayCorner = 'topLeft' | 'topRight' | 'bottomLeft' | 'bottomRight';

export interface TelemetryOverlayDescriptor {
  readonly title: string;
  readonly corner: TelemetryOverlayCorner;
  readonly refreshIntervalMs: number;
  readonly maxFrameTimeSamples: number;
  readonly visible: boolean;
}

export interface TelemetryOverlayPatch {
  readonly title: string | null;
  readonly corner: TelemetryOverlayCorner | null;
  readonly refreshIntervalMs: number | null;
  readonly maxFrameTimeSamples: number | null;
  readonly visible: boolean | null;
}

export type TelemetryOverlayProjectionOp =
  | { readonly op: 'create'; readonly handle: TelemetryOverlayHandle; readonly descriptor: TelemetryOverlayDescriptor }
  | { readonly op: 'update'; readonly handle: TelemetryOverlayHandle; readonly patch: TelemetryOverlayPatch }
  | { readonly op: 'destroy'; readonly handle: TelemetryOverlayHandle };

export interface ResolvedAnimationMotion {
  readonly clipA: string;
  readonly clipB: string | null;
  readonly blendWeightMilli: number;
  readonly speedMilli: number;
}

export interface AnimationTransitionState {
  readonly transitionId: string;
  readonly fromStateId: string;
  readonly toStateId: string;
  readonly elapsedTicks: number;
  readonly durationTicks: number;
  readonly targetMotion: ResolvedAnimationMotion;
}

export type AnimationTransitionFactMoment = 'started' | 'completed';

export interface AnimationTransitionFact {
  readonly controllerTick: number;
  readonly transitionId: string;
  readonly fromStateId: string;
  readonly toStateId: string;
  readonly moment: AnimationTransitionFactMoment;
  readonly durationTicks: number;
}

export interface AnimationControllerProjectionState {
  readonly entity: number;
  readonly graphId: string;
  readonly graphVersion: number;
  readonly stateId: string;
  readonly revision: number;
  readonly controllerTick: number;
  readonly motion: ResolvedAnimationMotion;
  readonly transition: AnimationTransitionState | null;
  readonly transitionFact: AnimationTransitionFact | null;
}

export interface AnimationProjectionDescriptor {
  readonly target: RenderHandle;
  readonly asset: string;
  readonly contentHash: string;
  readonly tickDurationMillis: number;
  readonly controller: AnimationControllerProjectionState;
}

export type AnimationProjectionOp =
  | { readonly op: 'create'; readonly handle: AnimationProjectionHandle; readonly descriptor: AnimationProjectionDescriptor }
  | { readonly op: 'update'; readonly handle: AnimationProjectionHandle; readonly controller: AnimationControllerProjectionState }
  | { readonly op: 'destroy'; readonly handle: AnimationProjectionHandle };

export type PresentationOp =
  | { readonly domain: 'audio'; readonly meta: PresentationOpMeta; readonly op: AudioProjectionOp }
  | { readonly domain: 'billboard'; readonly meta: PresentationOpMeta; readonly op: BillboardProjectionOp }
  | { readonly domain: 'particle'; readonly meta: PresentationOpMeta; readonly op: ParticleProjectionOp }
  | { readonly domain: 'telemetryOverlay'; readonly meta: PresentationOpMeta; readonly op: TelemetryOverlayProjectionOp }
  | { readonly domain: 'animation'; readonly meta: PresentationOpMeta; readonly op: AnimationProjectionOp };

export interface PresentationFrameDiff {
  readonly schemaVersion: 1;
  readonly ops: readonly PresentationOp[];
}

export type PresentationDiagnosticCode =
  | 'invalidDescriptor'
  | 'assetMissing'
  | 'assetKindMismatch'
  | 'contentHashMismatch'
  | 'duplicateSignal'
  | 'duplicateHandle'
  | 'unknownHandle'
  | 'anchorMissing'
  | 'budgetExceeded'
  | 'unknownTarget'
  | 'clipMissing'
  | 'invalidTransition'
  | 'staleRevision'
  | 'unavailableHost'
  | 'hostFailure';

export interface PresentationHostDiagnostic {
  readonly domain: PresentationOp['domain'];
  readonly code: PresentationDiagnosticCode | string;
  readonly sequence: number;
  readonly handle: number | null;
  readonly message: string;
}
