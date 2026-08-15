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
    readonly code: 'invalid_root' | 'mount_failed' | 'disposed';
    constructor(code: RustyApplicationHostError['code'], message: string, options?: ErrorOptions);
}
export declare function mountRustyApplication(options: RustyApplicationHostOptions): Promise<RustyApplicationHost>;
