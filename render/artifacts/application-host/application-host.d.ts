import { type RustyApplicationContent } from './application-content.js';
import { type RustyDeveloperCommandShellOptions } from './developer-command-shell.js';
import { type RustyApplicationPresentationAspectBounds } from './presentation-frame.js';
import { type RustyApplicationInputPort, type RustyApplicationRuntimeInputOptions, type RustyApplicationRuntimeIntentValue } from './input-ingress.js';
import { type RustyApplicationUiProjectionOptions, type RustyApplicationUiProjectionPort, type RustyApplicationUiProjectionReadout, type RustyApplicationUiProjectionView } from './ui-projection.js';
export declare const RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION = "rusty_application_host.v1";
export type RustyApplicationInteractionMode = 'gameplay' | 'interface' | 'modal';
/** A Rust-projected Engine render frame. Strict decoding remains Engine-owned. */
export type RustyApplicationFrame = Readonly<Record<string, unknown>>;
/** A Rust-projected typed presentation diff. Strict decoding remains Engine-owned. */
export type RustyApplicationPresentationFrame = Readonly<Record<string, unknown>>;
export interface RustyApplicationCameraPose {
    readonly position: readonly [number, number, number];
    readonly pitchDegrees: number;
    readonly yawDegrees: number;
}
/** The ordinary five-value proxy enhancement mode set remains stable. */
export type RustyApplicationVoxelSpriteEnhancementMode = 'sprite' | 'relit' | 'depth-parallax' | 'sprite-splat' | 'full-splat';
export type RustyApplicationVoxelSpriteMode = RustyApplicationVoxelSpriteEnhancementMode | 'ghost-plate';
export interface RustyApplicationVoxelSpriteCaptureSettings {
    readonly resolution: number;
    readonly azimuthDegrees: number;
    readonly elevationDegrees: number;
    readonly near: number;
    readonly far: number;
    readonly fieldOfViewDegrees?: number;
    /** Defaults to an isolated capture-light rig with readable lighting. */
    readonly lighting?: RustyApplicationVoxelSpriteCaptureLighting;
}
export type RustyApplicationVoxelSpriteCaptureLighting = {
    readonly mode: 'scene';
} | {
    readonly mode: 'isolated';
    readonly ambientColor?: readonly [number, number, number];
    readonly ambientIntensity?: number;
    readonly keyDirection?: readonly [number, number, number];
    readonly keyColor?: readonly [number, number, number];
    readonly keyIntensity?: number;
    readonly fillDirection?: readonly [number, number, number];
    readonly fillColor?: readonly [number, number, number];
    readonly fillIntensity?: number;
};
export interface RustyApplicationVoxelSpriteConfig {
    readonly mode: RustyApplicationVoxelSpriteMode;
    readonly width: number;
    readonly height: number;
    readonly sampleColumns: number;
    readonly sampleRows: number;
    readonly splatColumns: number;
    readonly splatRows: number;
    readonly depthAmplitude: number;
    readonly depthContrast: number;
    readonly depthClamp: number;
    readonly depthScale: 'normalized' | 'world';
    readonly depthQuantizationSteps: number;
    readonly parallaxOcclusionScale: number;
    readonly parallaxOcclusionSteps: number;
    readonly depthDilationTexels: number;
    readonly depthConfidenceThreshold: number;
    readonly splatFootprint: number;
    readonly splatOverlap: number;
    readonly splatOpacity: number;
    readonly splatBlendMode: 'depth-write' | 'alpha-blend' | 'additive';
    readonly normalInfluence: number;
    readonly normalOrientationBlend: number;
    readonly orientationPolicy: 'camera-facing' | 'capture-held' | 'capture-camera-blend';
    readonly orientationBlend: number;
    readonly orientationElevationPolicy: 'capture' | 'world-upright';
    readonly orientationAzimuthOffsetDegrees: number;
    readonly representationTransition: 'opaque' | 'dither' | 'alpha';
    readonly representationWeight: number;
    readonly representationDitherOffset: number;
    readonly baseSpriteContribution: number;
    readonly viewAngleFalloff: number;
    /** Preserve captured shading or apply the captured normal pass independently of geometry mode. */
    readonly lightingMode: 'captured' | 'normal';
    readonly ambientLight: number;
    readonly diffuseLight: number;
    readonly outputGain: number;
    readonly ambientColor: readonly [number, number, number];
    readonly lightColor: readonly [number, number, number];
    readonly lightDirection: readonly [number, number, number];
    readonly ghostDepthRetention: number;
    readonly ghostAnchorPolicy: 'bounds-center' | 'bounds-normalized';
    readonly ghostAnchorValue: number;
    readonly ghostPlateMapping: 'plate-locked' | 'projective-surface';
    readonly ghostShellMode: 'whole-mesh' | 'strict-source' | 'repaired-source';
    readonly ghostShellDepthEpsilon: number;
    readonly ghostSectorCount: 1 | 4 | 8 | 16;
    readonly ghostSectorHysteresisDegrees: number;
    readonly ghostTransitionMode: 'hard-cut' | 'ordered-dither' | 'noise-dissolve' | 'edge-echo';
    readonly ghostTransitionDurationMilliseconds: number;
}
/** The normalized configuration reported by the ordinary five-mode enhancement. */
export type RustyApplicationVoxelSpriteEnhancementConfig = Omit<RustyApplicationVoxelSpriteConfig, 'mode' | 'ghostDepthRetention' | 'ghostAnchorPolicy' | 'ghostAnchorValue' | 'ghostPlateMapping' | 'ghostShellMode' | 'ghostShellDepthEpsilon' | 'ghostSectorCount' | 'ghostSectorHysteresisDegrees' | 'ghostTransitionMode' | 'ghostTransitionDurationMilliseconds'> & {
    readonly mode: RustyApplicationVoxelSpriteEnhancementMode;
};
export interface RustyApplicationVoxelSpritePreparedFrame {
    readonly width: number;
    readonly height: number;
    readonly textures: {
        readonly color: string;
        readonly depth: string;
        readonly normal: string;
        readonly coverage: string;
    };
    readonly depth: {
        readonly near: number;
        readonly far: number;
    };
    readonly capture: {
        readonly projection: 'perspective' | 'orthographic';
        readonly position: readonly [number, number, number];
        readonly right: readonly [number, number, number];
        readonly up: readonly [number, number, number];
        readonly forward: readonly [number, number, number];
        readonly boundsMinimum: readonly [number, number, number];
        readonly boundsMaximum: readonly [number, number, number];
    };
}
export type RustyApplicationVoxelSpriteSource = {
    readonly kind: 'retained';
    readonly handle: number;
    readonly capture: RustyApplicationVoxelSpriteCaptureSettings;
} | {
    readonly kind: 'prepared';
    readonly frame: RustyApplicationVoxelSpritePreparedFrame;
};
export interface RustyApplicationVoxelSpriteDefinition {
    readonly id: string;
    readonly source: RustyApplicationVoxelSpriteSource;
    readonly transform: {
        readonly position: readonly [number, number, number];
        readonly width: number;
        readonly height: number;
    };
    readonly mode: RustyApplicationVoxelSpriteMode;
    readonly config?: Partial<Omit<RustyApplicationVoxelSpriteConfig, 'mode' | 'width' | 'height'>>;
}
/** Explicit caller-controlled cadence; the renderer expands it into immutable normalized samples. */
export type RustyApplicationHeldAnimationSamplePlan = {
    readonly kind: 'exact';
    readonly normalizedTimes: readonly number[];
} | {
    readonly kind: 'cadence';
    readonly samplesPerSecond: 8 | 12 | 24;
    readonly count: number;
};
export interface RustyApplicationHeldAnimationFrameBankDefinition {
    readonly id: string;
    readonly animatedMesh: number;
    readonly clip: string;
    readonly samples: RustyApplicationHeldAnimationSamplePlan;
    readonly sectorCount: 1 | 4 | 8 | 16;
    readonly capture: RustyApplicationVoxelSpriteCaptureSettings;
    readonly transform: {
        readonly position: readonly [number, number, number];
        readonly width: number;
        readonly height: number;
    };
    readonly mode: RustyApplicationVoxelSpriteEnhancementMode;
    readonly config?: Partial<Omit<RustyApplicationVoxelSpriteEnhancementConfig, 'mode' | 'width' | 'height'>>;
}
export interface RustyApplicationHeldAnimationFrameBankReadout {
    readonly id: string;
    readonly state: 'preparing' | 'ready';
    readonly key: string;
    readonly generation: number;
    readonly source: {
        readonly asset: string;
        readonly assetGeneration: number;
        readonly handle: number;
        readonly contentHash: string | null;
        readonly clip: string;
        readonly origin: 'embedded' | 'pack';
        readonly pack: {
            readonly asset: string;
            readonly contentHash: string | null;
        } | null;
        readonly instanceTransform: {
            readonly position: readonly [number, number, number];
            readonly quaternion: readonly [number, number, number, number];
            readonly scale: readonly [number, number, number];
        };
    };
    readonly frameCount: number;
    readonly directionCount: number;
    readonly capturedFrameCount: number;
    readonly selectedSampleIndex: number | null;
    readonly selectedDirectionIndex: number | null;
    readonly captureCount: number;
    readonly cacheHitCount: number;
    readonly switchCount: number;
    readonly preparationCpuMilliseconds: number | null;
    readonly captureCpuMilliseconds: number | null;
    readonly lastSwitchCpuMilliseconds: number | null;
    readonly estimatedResidentBytes: number;
    readonly estimatedPeakBytes: number;
    readonly gpuTiming: 'not-measured';
    readonly cancelledCount: number;
    readonly replacementFailureCount: number;
}
export interface RustyApplicationVoxelSpriteDiagnostic {
    readonly code: 'disposed' | 'duplicate_id' | 'invalid_definition' | 'missing_source' | 'capture_failed' | 'unknown_id' | 'frame_bank_busy' | 'frame_bank_cancelled' | 'frame_bank_failed' | 'unknown_frame_bank';
    readonly message: string;
}
export interface RustyApplicationVoxelSpriteEnhancementReadout {
    readonly schemaVersion: 1;
    readonly revision: number;
    readonly mode: RustyApplicationVoxelSpriteEnhancementMode;
    readonly config: RustyApplicationVoxelSpriteEnhancementConfig;
    readonly captureCpuSubmissionMilliseconds: number | null;
    readonly steadyStateCpuSubmissionMilliseconds: number | null;
    readonly captureBasis: {
        readonly position: readonly [number, number, number];
        readonly right: readonly [number, number, number];
        readonly up: readonly [number, number, number];
        readonly forward: readonly [number, number, number];
    };
    readonly angularOffsetDegrees: number | null;
    readonly expectedDrawCalls: number;
    readonly geometrySampleCount: number;
    readonly frameTextureBytes: number;
    readonly geometryResourceCount: number;
    readonly materialResourceCount: number;
    readonly borrowedTextureCount: number;
    readonly baseSpriteVisible: boolean;
    readonly splatVisible: boolean;
    readonly composition: 'opaque-depth-writing-base' | 'base-blend-then-depth-writing-splats' | 'base-blend-then-alpha-blended-splats' | 'base-blend-then-additive-splats' | 'depth-writing-splats' | 'alpha-blended-splats' | 'additive-splats';
    readonly disposed: boolean;
    readonly limitations: readonly [
        'single-capture-view',
        'view-space-normals',
        'rgba8-depth',
        'approximate-splat-orientation',
        'unsorted-transparent-splats',
        'gpu-time-not-measured'
    ];
}
export interface RustyApplicationVoxelSpriteGhostPlateReadout {
    readonly schemaVersion: 1;
    readonly enabled: boolean;
    readonly fallbackActive: boolean;
    readonly fallbackReason: null | 'prepared-source-unsupported' | 'transition-failed';
    readonly matchedPose: boolean;
    readonly projection: 'perspective' | 'orthographic';
    readonly captureBasis: {
        readonly position: readonly [number, number, number];
        readonly right: readonly [number, number, number];
        readonly up: readonly [number, number, number];
        readonly forward: readonly [number, number, number];
    };
    readonly sourceViewBasis: {
        readonly position: readonly [number, number, number];
        readonly right: readonly [number, number, number];
        readonly up: readonly [number, number, number];
        readonly forward: readonly [number, number, number];
    };
    readonly depthRetention: number;
    readonly anchorPolicy: 'bounds-center' | 'bounds-normalized';
    readonly anchorValue: number;
    readonly anchorDepth: number;
    readonly plateMapping: 'plate-locked' | 'projective-surface';
    readonly shellMode: 'whole-mesh' | 'strict-source' | 'repaired-source';
    readonly shellDepthEpsilon: number;
    readonly shellDepthQuantizationStep: number;
    readonly shellEffectiveDepthEpsilon: number;
    readonly rejectedFragmentRatio: {
        readonly status: 'unavailable';
        readonly value: null;
    };
    readonly repairedBoundaryRatio: {
        readonly status: 'unavailable';
        readonly value: null;
    };
    readonly angularOffsetDegrees: number | null;
    readonly sectorCount: 1 | 4 | 8 | 16;
    readonly selectedSector: number;
    readonly pendingSector: number | null;
    readonly previousSector: number | null;
    readonly localAzimuthDegrees: number | null;
    readonly sectorHysteresisDegrees: number;
    readonly transitionMode: 'hard-cut' | 'ordered-dither' | 'noise-dissolve' | 'edge-echo';
    readonly transitionProgress: number;
    readonly transitionDurationMilliseconds: number;
    readonly residentSectorCount: number;
    readonly currentResourceResident: boolean;
    readonly previousResourceResident: boolean;
    readonly preparationCpuMilliseconds: number | null;
    readonly invalidationReason: string | null;
    readonly expectedDrawCalls: number;
    readonly meshCount: number;
    readonly materialResourceCount: number;
    readonly borrowedTextureCount: number;
    readonly disposed: boolean;
    readonly limitations: readonly string[];
}
export interface RustyApplicationVoxelSpriteReadout {
    readonly schemaVersion: 1;
    readonly revision: number;
    readonly entries: readonly {
        readonly id: string;
        readonly source: 'retained' | 'prepared';
        readonly sourceHandle: number | null;
        readonly capture: RustyApplicationVoxelSpriteCaptureSettings | null;
        readonly fallbackPreservedCount: number;
        readonly presentation: 'enhancement' | 'ghost-plate';
        readonly enhancement: RustyApplicationVoxelSpriteEnhancementReadout | null;
        readonly ghostPlate: RustyApplicationVoxelSpriteGhostPlateReadout | null;
    }[];
    readonly frameBanks: readonly RustyApplicationHeldAnimationFrameBankReadout[];
    readonly frameBankCandidates: readonly RustyApplicationHeldAnimationFrameBankReadout[];
    readonly frameBankMemory: {
        readonly readyResidentBytes: number;
        readonly candidateResidentBytes: number;
        readonly candidateReservedBytes: number;
        readonly peakBytes: number;
    };
    readonly frameBankOutcomes: readonly {
        readonly id: string;
        readonly cancelledCount: number;
        readonly replacementFailureCount: number;
    }[];
    readonly disposed: boolean;
}
export interface RustyApplicationVoxelSpriteReceipt {
    readonly applied: boolean;
    readonly diagnostics: readonly RustyApplicationVoxelSpriteDiagnostic[];
    readonly readout: RustyApplicationVoxelSpriteReadout;
}
/** Experimental renderer attachment. It becomes stale when application content is replaced. */
export interface RustyApplicationVoxelSpriteExperimentPort {
    readonly create: (definition: RustyApplicationVoxelSpriteDefinition) => RustyApplicationVoxelSpriteReceipt;
    readonly replace: (definition: RustyApplicationVoxelSpriteDefinition) => RustyApplicationVoxelSpriteReceipt;
    readonly configure: (id: string, patch: Partial<RustyApplicationVoxelSpriteConfig>) => RustyApplicationVoxelSpriteReceipt;
    readonly recapture: (id: string, settings?: RustyApplicationVoxelSpriteCaptureSettings) => RustyApplicationVoxelSpriteReceipt;
    readonly beginHeldAnimationFrameBank: (definition: RustyApplicationHeldAnimationFrameBankDefinition) => RustyApplicationVoxelSpriteReceipt;
    readonly prepareHeldAnimationFrameBank: (id: string, maximumCaptures?: number) => RustyApplicationVoxelSpriteReceipt;
    readonly cancelHeldAnimationFrameBank: (id: string) => RustyApplicationVoxelSpriteReceipt;
    readonly selectHeldAnimationFrameBank: (id: string, sampleIndex: number, directionIndex: number) => RustyApplicationVoxelSpriteReceipt;
    readonly destroyHeldAnimationFrameBank: (id: string) => RustyApplicationVoxelSpriteReceipt;
    readonly destroy: (id: string) => RustyApplicationVoxelSpriteReceipt;
    readonly readout: () => RustyApplicationVoxelSpriteReadout;
    readonly dispose: () => void;
}
export interface RustyApplicationFrameDiagnostic {
    readonly code: string;
    readonly message: string;
}
export interface RustyApplicationFrameReceipt {
    readonly applied: boolean;
    readonly diagnostics: readonly RustyApplicationFrameDiagnostic[];
}
export interface RustyApplicationPresentationDiagnostic {
    readonly code: string;
    readonly domain: string;
    readonly message: string;
}
export interface RustyApplicationPresentationReceipt {
    readonly applied: number;
    readonly diagnostics: readonly RustyApplicationPresentationDiagnostic[];
}
export interface RustyApplicationAudioResumeReceipt {
    readonly resumed: boolean;
    readonly diagnostics: readonly RustyApplicationFrameDiagnostic[];
}
export interface RustyApplicationRendererPort {
    readonly applyFrame: (frame: RustyApplicationFrame) => RustyApplicationFrameReceipt;
    readonly applyPresentation: (frame: RustyApplicationPresentationFrame) => Promise<RustyApplicationPresentationReceipt>;
    /** Replace product content with the Engine-owned empty/default retained frame. */
    readonly clear: () => Promise<void>;
    /** Create an experimental depth-enhanced sprite attachment on the current renderer surface. */
    readonly createVoxelSpriteExperiment: () => RustyApplicationVoxelSpriteExperimentPort;
    readonly renderOnce: (timeMs?: number) => void;
    /** Atomically replace the immutable resource catalog and complete retained frame. */
    readonly replaceContent: (content: RustyApplicationContent) => Promise<RustyApplicationFrameReceipt>;
    /** Prepare and atomically publish a complete Rust-projected retained frame. */
    readonly replaceFrame: (frame: RustyApplicationFrame) => Promise<RustyApplicationFrameReceipt>;
    /** Resume the browser audio context from a downstream user-gesture handler. */
    readonly resumeAudio: () => Promise<RustyApplicationAudioResumeReceipt>;
    readonly setCameraPose: (pose: RustyApplicationCameraPose) => void;
}
export interface RustyApplicationUiPort {
    readonly active: () => boolean;
    /**
     * Classify one original host event before a downstream adapter gives it
     * gameplay meaning. Interactive UI is rejected synchronously even before a
     * later click handler changes the coarse interaction mode.
     */
    readonly allowsGameplayInput: (event: Event) => boolean;
    readonly focusGameplay: () => void;
    readonly interactionMode: () => RustyApplicationInteractionMode;
    readonly setInteractionMode: (mode: RustyApplicationInteractionMode) => void;
}
/** Mounted DOM UI can emit a claim, but cannot drain or bind the input lane. */
export interface RustyApplicationUiIntentsPort {
    readonly claim: (intent: string, value: RustyApplicationRuntimeIntentValue) => void;
}
export interface RustyApplicationUiContext {
    readonly ui: RustyApplicationUiPort;
    /** Read-only current Product UI projection and subscription view. */
    readonly projection?: RustyApplicationUiProjectionView;
    /** Claim-only adapter for the shared ordered Runtime Composition input lane. */
    readonly intents?: RustyApplicationUiIntentsPort;
}
export interface RustyApplicationUiOwner {
    readonly dispose: () => void | Promise<void>;
}
/**
 * Mount trusted downstream product UI into the Engine-owned composition root.
 * This is an application composition seam, not an untrusted plugin boundary.
 */
export type RustyApplicationUiMount = (root: HTMLElement, context: RustyApplicationUiContext) => void | RustyApplicationUiOwner | Promise<void | RustyApplicationUiOwner>;
export interface RustyApplicationRendererOptions {
    readonly clearColor?: number;
    /** Optional Engine-owned linear fog applied by the mounted renderer surface. */
    readonly fog?: RustyApplicationFogOptions;
    readonly initialContent?: RustyApplicationContent;
    readonly initialFrame?: RustyApplicationFrame;
    readonly pixelRatio?: number;
    /** Gameplay-owned entity positions used only to resolve neutral billboard anchors. */
    readonly resolveIndicatorEntityPosition?: (entity: number) => readonly [number, number, number] | null;
    /** Gameplay-owned entity positions used only to resolve neutral particle anchors. */
    readonly resolveParticleEntityPosition?: (entity: number) => readonly [number, number, number] | null;
    /** Observe the one Engine-owned renderer cadence without creating another RAF. */
    readonly onCadence?: (timeMs: number) => void;
}
export interface RustyApplicationFogOptions {
    readonly color: number;
    readonly near: number;
    readonly far: number;
}
export interface RustyApplicationHostOptions {
    readonly root: HTMLElement;
    readonly mountUi: RustyApplicationUiMount;
    /** Optional finite inclusive aspect interval for one shared, clipped presentation frame. */
    readonly presentationAspectBounds?: RustyApplicationPresentationAspectBounds;
    /** Optional Engine-owned console UI over a product-supplied command adapter. */
    readonly developerCommands?: RustyDeveloperCommandShellOptions;
    readonly renderer?: RustyApplicationRendererOptions;
    readonly loadingLabel?: string;
    readonly failureLabel?: string;
    readonly initialInteractionMode?: RustyApplicationInteractionMode;
    /** Optional browser input ingress. Omission leaves renderer controls and DOM capture disabled. */
    readonly runtimeInput?: RustyApplicationRuntimeInputOptions;
    /** Optional strict Product UI projection channel. */
    readonly uiProjection?: RustyApplicationUiProjectionOptions;
}
export interface RustyApplicationHostReadout {
    readonly compatibilityVersion: typeof RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION;
    readonly contentRevision: number;
    readonly interactionMode: RustyApplicationInteractionMode;
    readonly pointerLocked: boolean;
    readonly resourceBytes: number;
    readonly resourceCount: number;
    readonly uiProjection?: RustyApplicationUiProjectionReadout;
    readonly state: 'ready' | 'disposed';
}
export interface RustyApplicationHost {
    readonly kind: 'rusty_application_host.v1';
    readonly renderer: RustyApplicationRendererPort;
    readonly ui: RustyApplicationUiPort;
    /** Optional ordered physical-input and direct-UI-claim transport lane. */
    readonly input?: RustyApplicationInputPort;
    /** Trusted host/composition-root ingress for Rust Product UI projections. */
    readonly uiProjection?: RustyApplicationUiProjectionPort;
    readonly readout: () => RustyApplicationHostReadout;
    readonly dispose: () => Promise<void>;
}
export declare class RustyApplicationHostError extends Error {
    readonly code: 'invalid_presentation_aspect_bounds' | 'invalid_root' | 'mount_failed' | 'disposed' | 'stale_renderer_port';
    constructor(code: RustyApplicationHostError['code'], message: string, options?: ErrorOptions);
}
export declare function mountRustyApplication(options: RustyApplicationHostOptions): Promise<RustyApplicationHost>;
