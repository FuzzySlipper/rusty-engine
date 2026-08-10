export declare const RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION = "rusty_application_host.v1";
export type RustyApplicationInteractionMode = 'gameplay' | 'interface' | 'modal';
/** A Rust-projected Engine render frame. Strict decoding remains Engine-owned. */
export type RustyApplicationFrame = Readonly<Record<string, unknown>>;
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
export interface RustyApplicationRendererPort {
    readonly applyFrame: (frame: RustyApplicationFrame) => RustyApplicationFrameReceipt;
    /** Replace product content with the Engine-owned empty/default retained frame. */
    readonly clear: () => Promise<void>;
    readonly renderOnce: (timeMs?: number) => void;
    /** Prepare and atomically publish a complete Rust-projected retained frame. */
    readonly replaceFrame: (frame: RustyApplicationFrame) => Promise<RustyApplicationFrameReceipt>;
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
    readonly initialFrame?: RustyApplicationFrame;
    readonly pixelRatio?: number;
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
    readonly interactionMode: RustyApplicationInteractionMode;
    readonly pointerLocked: boolean;
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
