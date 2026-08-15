import { type RustyApplicationContent } from './application-content.js';
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
export type RustyApplicationVoxelSpriteMode = 'sprite' | 'relit' | 'depth-parallax' | 'sprite-splat' | 'full-splat';
export interface RustyApplicationVoxelSpriteCaptureSettings {
    readonly resolution: number;
    readonly azimuthDegrees: number;
    readonly elevationDegrees: number;
    readonly near: number;
    readonly far: number;
}
export interface RustyApplicationVoxelSpriteConfig {
    readonly mode: RustyApplicationVoxelSpriteMode;
    readonly width: number;
    readonly height: number;
    readonly sampleColumns: number;
    readonly sampleRows: number;
    readonly depthAmplitude: number;
    readonly depthClamp: number;
    readonly depthScale: 'normalized' | 'world';
    readonly depthQuantizationSteps: number;
    readonly depthDilationTexels: number;
    readonly depthConfidenceThreshold: number;
    readonly splatFootprint: number;
    readonly splatOverlap: number;
    readonly normalInfluence: number;
    readonly normalOrientationBlend: number;
    readonly baseSpriteContribution: number;
    readonly viewAngleFalloff: number;
    readonly lightDirection: readonly [number, number, number];
}
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
export interface RustyApplicationVoxelSpriteDiagnostic {
    readonly code: 'disposed' | 'duplicate_id' | 'invalid_definition' | 'missing_source' | 'capture_failed' | 'unknown_id';
    readonly message: string;
}
export interface RustyApplicationVoxelSpriteEnhancementReadout {
    readonly schemaVersion: 1;
    readonly revision: number;
    readonly mode: RustyApplicationVoxelSpriteMode;
    readonly config: RustyApplicationVoxelSpriteConfig;
    readonly captureCpuSubmissionMilliseconds: number | null;
    readonly steadyStateCpuSubmissionMilliseconds: number | null;
    readonly expectedDrawCalls: number;
    readonly geometrySampleCount: number;
    readonly frameTextureBytes: number;
    readonly geometryResourceCount: number;
    readonly materialResourceCount: number;
    readonly borrowedTextureCount: number;
    readonly baseSpriteVisible: boolean;
    readonly splatVisible: boolean;
    readonly composition: 'opaque-depth-writing-base' | 'base-blend-then-depth-writing-splats' | 'depth-writing-splats';
    readonly disposed: boolean;
    readonly limitations: readonly [
        'single-capture-view',
        'view-space-normals',
        'rgba8-depth',
        'approximate-splat-orientation',
        'gpu-time-not-measured'
    ];
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
        readonly enhancement: RustyApplicationVoxelSpriteEnhancementReadout;
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
export interface RustyApplicationUiContext {
    readonly renderer: RustyApplicationRendererPort;
    readonly ui: RustyApplicationUiPort;
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
}
export interface RustyApplicationFogOptions {
    readonly color: number;
    readonly near: number;
    readonly far: number;
}
export interface RustyApplicationHostOptions {
    readonly root: HTMLElement;
    readonly mountUi: RustyApplicationUiMount;
    readonly renderer?: RustyApplicationRendererOptions;
    readonly loadingLabel?: string;
    readonly failureLabel?: string;
    readonly initialInteractionMode?: RustyApplicationInteractionMode;
}
export interface RustyApplicationHostReadout {
    readonly compatibilityVersion: typeof RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION;
    readonly contentRevision: number;
    readonly interactionMode: RustyApplicationInteractionMode;
    readonly pointerLocked: boolean;
    readonly resourceBytes: number;
    readonly resourceCount: number;
    readonly state: 'ready' | 'disposed';
}
export interface RustyApplicationHost {
    readonly kind: 'rusty_application_host.v1';
    readonly renderer: RustyApplicationRendererPort;
    readonly ui: RustyApplicationUiPort;
    readonly readout: () => RustyApplicationHostReadout;
    readonly dispose: () => Promise<void>;
}
export declare class RustyApplicationHostError extends Error {
    readonly code: 'invalid_root' | 'mount_failed' | 'disposed' | 'stale_renderer_port';
    constructor(code: RustyApplicationHostError['code'], message: string, options?: ErrorOptions);
}
export declare function mountRustyApplication(options: RustyApplicationHostOptions): Promise<RustyApplicationHost>;
