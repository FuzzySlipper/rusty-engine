import { type RustyApplicationContent } from './application-content.js';
import { type RustyApplicationPresentationAspectBounds } from './presentation-frame.js';
import { type RustyApplicationInputPort, type RustyApplicationRuntimeInputOptions, type RustyApplicationRuntimeIntentValue } from './input-ingress.js';
import { type RustyApplicationUiProjectionOptions, type RustyApplicationUiProjectionPort, type RustyApplicationUiProjectionReadout, type RustyApplicationUiProjectionView } from './ui-projection.js';
export declare const RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION = "rusty_application_host.v1";
export type RustyApplicationInteractionMode = 'gameplay' | 'interface' | 'modal';
/** A Rust-projected Engine render frame. Strict decoding remains Engine-owned. */
export type RustyApplicationFrame = Readonly<Record<string, unknown>>;
/** A Rust-projected typed presentation diff. Strict decoding remains Engine-owned. */
export type RustyApplicationPresentationFrame = Readonly<Record<string, unknown>>;
export interface RustyApplicationViewCompositionCamera {
    readonly id: string;
    readonly pose: {
        readonly position: readonly [number, number, number];
        readonly pitchDegrees: number;
        readonly yawDegrees: number;
    };
    readonly basis?: {
        readonly forward: readonly [number, number, number];
        readonly right: readonly [number, number, number];
        readonly up: readonly [number, number, number];
    };
    readonly projection: {
        readonly kind: 'perspective';
        readonly fovYDegrees: number;
        readonly near: number;
        readonly far: number;
    } | {
        readonly kind: 'orthographic';
        readonly verticalSize: number;
        readonly near: number;
        readonly far: number;
    };
}
export interface RustyApplicationViewCompositionTarget {
    readonly id: string;
    readonly revision: number;
    readonly width: number;
    readonly height: number;
    readonly color: 'rgba8_srgb';
    readonly depth: 'depth24' | 'none';
    readonly sampling: 'linear' | 'nearest';
}
export interface RustyApplicationViewCompositionViewport {
    readonly x: number;
    readonly y: number;
    readonly width: number;
    readonly height: number;
}
export interface RustyApplicationViewCompositionView {
    readonly id: string;
    readonly cameraId: string;
    readonly target: {
        readonly kind: 'primary';
    } | {
        readonly kind: 'offscreen';
        readonly targetId: string;
        readonly targetRevision: number;
    };
    readonly viewport: RustyApplicationViewCompositionViewport;
    readonly order: number;
}
export interface RustyApplicationViewCompositionPresentation {
    readonly id: string;
    readonly sourceTargetId: string;
    readonly sourceTargetRevision: number;
    readonly destination: {
        readonly kind: 'primary';
        readonly viewport: RustyApplicationViewCompositionViewport;
    };
    readonly order: number;
}
/** Typed Engine view composition, realized by the Engine renderer against its current surface. */
export interface RustyApplicationViewComposition {
    readonly schemaVersion: 1;
    readonly cameras: readonly RustyApplicationViewCompositionCamera[];
    readonly targets: readonly RustyApplicationViewCompositionTarget[];
    readonly views: readonly RustyApplicationViewCompositionView[];
    readonly presentations: readonly RustyApplicationViewCompositionPresentation[];
}
/** A product-provided marker snapshot realized only by the Engine animation host. */
export interface RustyApplicationAnimationCueDefinition {
    readonly cueId: string;
    readonly asset: string;
    readonly clip: string;
    readonly atSeconds: number;
    readonly signal: {
        readonly domain: 'audio' | 'particle';
        readonly id: string;
    };
}
export interface RustyApplicationCameraPose {
    readonly position: readonly [number, number, number];
    readonly pitchDegrees: number;
    readonly yawDegrees: number;
}
/** Focused renderer-owned retained ghost facts; no backend object crosses this port. */
export interface RustyApplicationGhostPlateReadout {
    readonly activePlates: number;
    readonly plates: readonly {
        readonly handle: number;
        readonly source: number;
        readonly sourceMatch: boolean;
        readonly currentSector: number;
        readonly localAzimuthDegrees: number | null;
        readonly capture: {
            readonly resolution: number;
            readonly azimuthDegrees: number;
            readonly elevationDegrees: number;
            readonly near: number;
            readonly far: number;
            readonly fieldOfViewDegrees: number;
            readonly lighting: {
                readonly mode: 'scene' | 'isolated';
            };
        };
        readonly config: {
            readonly depthRetention: number;
            readonly anchorPolicy: 'bounds-center' | 'bounds-normalized';
            readonly anchorValue: number;
            readonly plateMapping: 'plate-locked' | 'projective-surface';
            readonly shellMode: 'whole-mesh' | 'strict-source' | 'repaired-source';
            readonly shellDepthEpsilon: number;
            readonly sectorCount: 1 | 4 | 8 | 16;
            readonly sectorHysteresisDegrees: number;
        };
        readonly fallbackActive: boolean;
        readonly fallbackReason: string | null;
        /** Closed GhostPlateLimitationMask bits; no renderer limitation strings cross this port. */
        readonly limitationMask: number;
        readonly preparationCpuMilliseconds: number | null;
        readonly captureCpuSubmissionMilliseconds: number | null;
        readonly retainedResourceCounts: {
            readonly sectors: number;
            readonly meshes: number;
            readonly materials: number;
            readonly borrowedTextures: number;
        };
    }[];
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
export type RustyApplicationAudioDiagnosticCode = 'invalidDescriptor' | 'assetMissing' | 'assetKindMismatch' | 'contentHashMismatch' | 'duplicateSignal' | 'duplicateHandle' | 'unknownHandle' | 'unavailableHost' | 'audioContextBlocked' | 'decodeFailed' | 'hostFailure';
export interface RustyApplicationAudioDiagnostic {
    readonly code: RustyApplicationAudioDiagnosticCode;
    readonly sequence: number;
    readonly handle: number | null;
    readonly message: string;
}
export type RustyApplicationAudioRealizedFact = {
    readonly kind: 'naturalCompletion';
    readonly factId: number;
    readonly source: 'oneShot';
    readonly sequence: number;
    readonly signalHandle: number;
} | {
    readonly kind: 'naturalCompletion';
    readonly factId: number;
    readonly source: 'retainedVoice';
    readonly sequence: number;
    readonly handle: number;
} | {
    readonly kind: 'diagnostic';
    readonly factId: number;
    readonly diagnostic: RustyApplicationAudioDiagnostic;
};
export interface RustyApplicationAudioRealizedFactsReadout {
    readonly retainedFactCount: number;
    readonly evictedFactCount: number;
    readonly facts: readonly RustyApplicationAudioRealizedFact[];
}
export type RustyApplicationAnimationDiagnosticCode = 'invalidDescriptor' | 'duplicateHandle' | 'unknownHandle' | 'unknownTarget' | 'assetMissing' | 'contentHashMismatch' | 'clipMissing' | 'incompatibleRig' | 'invalidBlendWeight' | 'invalidTransition' | 'staleRevision' | 'unavailableHost' | 'compatibilityFallback' | 'hostFailure';
export interface RustyApplicationAnimationDiagnostic {
    readonly code: RustyApplicationAnimationDiagnosticCode;
    readonly sequence: number;
    readonly handle: number | null;
    readonly target: number | null;
    readonly message: string;
}
export type RustyApplicationAnimationRealizedFact = {
    readonly kind: 'playbackObservation';
    readonly factId: number;
    readonly objectId: number;
    readonly generation: number;
    readonly sequence: number;
    readonly status: 'unavailable' | 'not_started' | 'playing' | 'paused' | 'sampled' | 'stopped';
    readonly selectedClip: string | null;
    readonly sampledAtSeconds: number | null;
} | {
    readonly kind: 'diagnostic';
    readonly factId: number;
    readonly objectId: number | null;
    readonly generation: number | null;
    readonly diagnostic: RustyApplicationAnimationDiagnostic;
} | {
    readonly kind: 'cue';
    readonly factId: number;
    readonly objectId: number;
    readonly generation: number;
    readonly cueId: string;
    readonly clip: string;
    readonly markerSeconds: number;
    readonly sampledAtSeconds: number;
    readonly signal: RustyApplicationAnimationCueDefinition['signal'];
} | {
    readonly kind: 'stopped';
    readonly factId: number;
    readonly objectId: number;
    readonly generation: number;
    readonly sequence: number;
    readonly reason: 'destroyed' | 'teardown';
} | {
    readonly kind: 'naturalCompletion';
    readonly factId: number;
    readonly objectId: number;
    readonly generation: number;
    readonly clip: string;
};
export interface RustyApplicationAnimationRealizedFactsReadout {
    readonly retainedFactCount: number;
    readonly evictedFactCount: number;
    readonly facts: readonly RustyApplicationAnimationRealizedFact[];
}
export interface RustyApplicationViewCompositionReceipt {
    readonly applied: boolean;
    readonly diagnostics: readonly {
        readonly code: 'invalid_view_composition' | 'stale_target_revision' | 'surface_disposed' | 'target_allocation_failed';
        readonly message: string;
    }[];
    readonly revision: number;
}
export interface RustyApplicationRendererPort {
    readonly applyFrame: (frame: RustyApplicationFrame) => RustyApplicationFrameReceipt;
    readonly applyPresentation: (frame: RustyApplicationPresentationFrame) => Promise<RustyApplicationPresentationReceipt>;
    /** Atomically replace marker definitions consumed by the existing animation host. */
    readonly replaceAnimationCueDefinitions: (definitions: readonly RustyApplicationAnimationCueDefinition[]) => RustyApplicationFrameReceipt;
    /** Read Engine-realized audio facts without exposing the browser audio owner. */
    readonly audioRealizedFacts: () => RustyApplicationAudioRealizedFactsReadout | null;
    readonly animationRealizedFacts: () => RustyApplicationAnimationRealizedFactsReadout | null;
    readonly ghostPlateReadout: () => RustyApplicationGhostPlateReadout | null;
    /** Acknowledge only the submitted Engine-realized audio fact range. */
    readonly acknowledgeAudioRealizedFacts: (throughFactId: number) => boolean;
    readonly acknowledgeAnimationRealizedFacts: (throughFactId: number) => boolean;
    /** Invalidate the realized-audio owner when a product runtime binding changes. */
    readonly resetAudioRealizationOwner: () => boolean;
    readonly resetAnimationRealizationOwner: () => boolean;
    readonly configureViews: (composition: RustyApplicationViewComposition) => RustyApplicationViewCompositionReceipt;
    /** Replace product content with the Engine-owned empty/default retained frame. */
    readonly clear: () => Promise<void>;
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
    /** Claim-only adapter for the shared ordered Engine input lane. */
    readonly intents?: RustyApplicationUiIntentsPort;
}
export interface RustyApplicationUiOwner {
    readonly dispose: () => void | Promise<void>;
}
/**
 * Mount trusted downstream product UI into the Engine-owned composition root.
 * This is an application composition seam, not an untrusted plugin boundary.
 * The root is hit-test transparent: native interactive controls and descendants
 * marked `data-rusty-ui-interactive` receive pointer events, while other overlay
 * regions pass through to the Engine canvas and its input arbitration.
 */
export type RustyApplicationUiMount = (root: HTMLElement, context: RustyApplicationUiContext) => void | RustyApplicationUiOwner | Promise<void | RustyApplicationUiOwner>;
export interface RustyApplicationRendererOptions {
    readonly clearColor?: number;
    /** Optional Engine-owned linear fog applied by the mounted renderer surface. */
    readonly fog?: RustyApplicationFogOptions;
    /** Optional retained-light policy for the mounted world and shadow backend. */
    readonly lighting?: RustyApplicationLightingOptions;
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
export interface RustyApplicationLightingOptions {
    readonly defaultLights?: {
        readonly world?: 'neutral' | 'disabled';
        readonly viewmodel?: 'neutral' | 'disabled';
    };
    readonly shadows?: {
        readonly enabled?: boolean;
        readonly maximumActiveLights?: number;
    };
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
