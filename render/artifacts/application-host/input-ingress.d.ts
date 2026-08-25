/**
 * Browser-only capture for the public application host. The values emitted here
 * are deliberately host-neutral: DOM events, canvas ownership, and pointer lock
 * do not cross the application-host boundary.
 */
export declare const RUSTY_APPLICATION_INPUT_QUEUE_MAXIMUM = 1024;
export declare const RUSTY_APPLICATION_INPUT_POINTER_DELTA_MAXIMUM = 256;
export declare const RUSTY_APPLICATION_INPUT_WHEEL_DELTA_MAXIMUM = 256;
export declare const RUSTY_APPLICATION_INPUT_SELECTED_CONTROLLER_MAXIMUM = 3;
export declare const RUSTY_APPLICATION_INPUT_U64_MAXIMUM = 18446744073709551615n;
/** Mirrors the Rust-owned Product Model direct product-payload bound. */
export declare const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_BYTES_MAXIMUM = 65536;
export declare const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_DEPTH_MAXIMUM = 32;
export declare const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_NODES_MAXIMUM = 4096;
export declare const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_STRING_BYTES_MAXIMUM = 16384;
export declare const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_COLLECTION_MAXIMUM = 1024;
export declare const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_SAFE_INTEGER_MAXIMUM = 9007199254740991;
export interface RustyApplicationRuntimeIdentity {
    /** Canonical unsigned 64-bit decimal text; never a lossy JavaScript number. */
    readonly instanceId: string;
    /** Canonical unsigned 64-bit decimal text; never a lossy JavaScript number. */
    readonly generation: string;
    /** Canonical unsigned 64-bit decimal text; never a lossy JavaScript number. */
    readonly controlRevision: string;
}
export interface RustyApplicationRuntimeInputBinding {
    readonly runtime: RustyApplicationRuntimeIdentity;
    /** Product-declared input context. The host preserves it but assigns no meaning. */
    readonly context: string;
}
/** Mirrors the closed Product Model control catalog; navigation keys are not admitted yet. */
export type RustyApplicationKeyboardControl = 'key-a' | 'key-b' | 'key-c' | 'key-d' | 'key-e' | 'key-f' | 'key-g' | 'key-h' | 'key-i' | 'key-j' | 'key-k' | 'key-l' | 'key-m' | 'key-n' | 'key-o' | 'key-p' | 'key-q' | 'key-r' | 'key-s' | 'key-t' | 'key-u' | 'key-v' | 'key-w' | 'key-x' | 'key-y' | 'key-z' | 'digit-0' | 'digit-1' | 'digit-2' | 'digit-3' | 'digit-4' | 'digit-5' | 'digit-6' | 'digit-7' | 'digit-8' | 'digit-9' | 'space' | 'enter' | 'escape' | 'shift-left' | 'shift-right' | 'control-left' | 'control-right' | 'alt-left' | 'alt-right';
export type RustyApplicationPointerButton = 'primary' | 'secondary' | 'middle';
export type RustyApplicationControllerButton = 'button-0' | 'button-1' | 'button-2' | 'button-3' | 'button-4' | 'button-5' | 'button-6' | 'button-7' | 'button-8' | 'button-9' | 'button-10' | 'button-11' | 'button-12' | 'button-13' | 'button-14' | 'button-15';
export type RustyApplicationControllerAxis = 'axis-0' | 'axis-1' | 'axis-2' | 'axis-3';
export type RustyApplicationInputEdge = 'pressed' | 'released';
export type RustyApplicationInputClearReason = 'focus-loss' | 'ingress-overflow' | 'interaction-mode-loss' | 'pointer-lock-loss' | 'restart' | 'control-revision-change' | 'dispose';
/** Structural mirror of the Runtime Composition physical ingress wire. */
export type RustyApplicationRuntimeInputFact = {
    readonly kind: 'key';
    readonly code: RustyApplicationKeyboardControl;
    readonly edge: RustyApplicationInputEdge;
} | {
    readonly kind: 'pointer-button';
    readonly button: RustyApplicationPointerButton;
    readonly edge: RustyApplicationInputEdge;
} | {
    readonly kind: 'pointer-delta';
    readonly x: number;
    readonly y: number;
} | {
    readonly kind: 'wheel';
    readonly x: number;
    readonly y: number;
} | {
    readonly kind: 'controller-button';
    readonly button: RustyApplicationControllerButton;
    readonly edge: RustyApplicationInputEdge;
} | {
    readonly kind: 'controller-axis';
    readonly axis: RustyApplicationControllerAxis;
    readonly value: number;
} | {
    readonly kind: 'clear';
    readonly reason: RustyApplicationInputClearReason;
};
export interface RustyApplicationRuntimeInputIngress {
    readonly runtime: RustyApplicationRuntimeIdentity;
    /** Canonical unsigned 64-bit decimal text scoped to the runtime epoch. */
    readonly sequence: string;
    readonly context: string;
    readonly fact: RustyApplicationRuntimeInputFact;
}
export type RustyApplicationRuntimeIntentValue = {
    readonly kind: 'digital';
    readonly active: boolean;
} | {
    readonly kind: 'axis';
    readonly value: number;
} | {
    readonly kind: 'product-payload';
    /** Stable product schema identity; Rust matches it to the descriptor. */
    readonly contract: string;
    /** Bounded plain JSON only; never a callback, DOM object, or command route. */
    readonly data: RustyApplicationProductPayloadJson;
};
export type RustyApplicationProductPayloadJson = null | boolean | number | string | readonly RustyApplicationProductPayloadJson[] | RustyApplicationProductPayloadJsonObject;
export interface RustyApplicationProductPayloadJsonObject {
    readonly [key: string]: RustyApplicationProductPayloadJson;
}
/** Structural mirror of a trusted product UI intent claim for the same ordered lane. */
export interface RustyApplicationRuntimeDirectIntentClaim {
    readonly runtime: RustyApplicationRuntimeIdentity;
    /** Canonical unsigned 64-bit decimal text scoped to the runtime epoch. */
    readonly sequence: string;
    readonly context: string;
    readonly intent: string;
    readonly value: RustyApplicationRuntimeIntentValue;
}
export type RustyApplicationRuntimeInputEnvelope = RustyApplicationRuntimeInputIngress | RustyApplicationRuntimeDirectIntentClaim;
export interface RustyApplicationSelectedControllerOptions {
    /** Browser gamepad index. Only one explicitly selected controller is observed. */
    readonly index: number;
}
export interface RustyApplicationRuntimeInputOptions {
    /** Initial runtime binding. Input remains inert until `bindRuntime` when omitted. */
    readonly binding?: RustyApplicationRuntimeInputBinding;
    /** Maximum queued physical facts and direct UI claims, inclusive of the fail-closed clear. */
    readonly maximumQueue?: number;
    /** Absolute pointer movement cap per DOM event. */
    readonly maximumPointerDelta?: number;
    /** Absolute wheel cap per DOM event. */
    readonly maximumWheelDelta?: number;
    /** Opt-in selected-controller observation; sampling remains caller-driven. */
    readonly selectedController?: RustyApplicationSelectedControllerOptions;
    /** Host-owned notification that queued input is available to drain. */
    readonly onAvailable?: () => void;
}
export interface RustyApplicationInputPort {
    /** Bind a runtime epoch. Rebinds clear under the new epoch before any later fact. */
    readonly bindRuntime: (binding: RustyApplicationRuntimeInputBinding) => void;
    /** Alias for a lifecycle owner synchronizing a possibly changed runtime epoch. */
    readonly synchronizeRuntime: (binding: RustyApplicationRuntimeInputBinding) => void;
    /** Change the product input context after clearing the old context's pending facts. */
    readonly setContext: (context: string) => void;
    /** Explicitly clear local held state and queue an ordered lifecycle clear fact. */
    readonly clear: (reason: RustyApplicationInputClearReason) => void;
    /** Drain the combined physical-input and direct-UI-claim lane in observation order. */
    readonly drain: () => readonly RustyApplicationRuntimeInputEnvelope[];
    /** Claim one product-declared intent from trusted UI without mutating product state. */
    readonly claim: (intent: string, value: RustyApplicationRuntimeIntentValue) => void;
    /** Sample the one selected browser controller. The caller, never this host, owns cadence. */
    readonly sampleController: () => number;
}
interface RustyApplicationInputIngressEnvironment {
    readonly canvas: () => HTMLCanvasElement;
    /** Stable application frame that receives pointer/button/wheel bubbling above the canvas. */
    readonly eventTarget: HTMLElement;
    readonly document: Document;
    readonly allowsGameplayInput: (event: Event) => boolean;
    readonly interactionMode: () => 'gameplay' | 'interface' | 'modal';
    readonly active: () => boolean;
    readonly focusGameplay: () => void;
    readonly gamepads: () => readonly (Gamepad | null)[];
}
export interface RustyApplicationInputQueue {
    readonly bindRuntime: (binding: RustyApplicationRuntimeInputBinding) => boolean;
    readonly setContext: (context: string) => boolean;
    readonly clear: (reason: RustyApplicationInputClearReason) => void;
    /** True means the bounded queue overflowed and now contains only a clear fact. */
    readonly enqueueFact: (fact: RustyApplicationRuntimeInputFact) => boolean;
    /** True means the bounded queue overflowed and now contains only a clear fact. */
    readonly claim: (intent: string, value: RustyApplicationRuntimeIntentValue) => boolean;
    readonly drain: () => readonly RustyApplicationRuntimeInputEnvelope[];
}
export interface RustyApplicationManagedInputIngress extends RustyApplicationInputPort {
    /** Application-host lifecycle seam for transactional renderer canvas replacement. */
    readonly rebindCanvas: (canvas: HTMLCanvasElement) => void;
    /** Application-host lifecycle seam; product callers use the owning host disposal instead. */
    readonly dispose: () => void;
}
/**
 * Creates the optional DOM adapter. It owns capture and cleanup only; no look
 * integration, movement, action mapping, or cadence enters this module.
 */
export declare function createRustyApplicationInputIngress(options: RustyApplicationRuntimeInputOptions, environment: RustyApplicationInputIngressEnvironment): RustyApplicationManagedInputIngress;
/** Strictly normalize DOM keyboard codes before they become host-neutral observations. */
export declare function normalizeRustyApplicationKeyboardControl(code: string): RustyApplicationKeyboardControl | null;
/**
 * The optional initial sequence exists for boundary tests. Production ingress
 * always begins a newly bound epoch at zero.
 */
export declare function createRustyApplicationInputQueue(maximumQueue: number, initialSequence?: bigint): RustyApplicationInputQueue;
export {};
