// Generated from the Rust wire types by product-host's
// `typescript_contracts_are_current` test. Do not edit: change the Rust types
// and run scripts/generate-typescript-contracts.sh.

/** Where the page pulls the frames the runtime renders. */
export const FRAME_STREAM_PATH = "/__rusty/product/runtime/frames";

/** The little-endian `RSF1` frame header: its magic, field offsets, formats and flags. */
export const FRAME_STREAM_HEADER = {
  magic: 0x31465352,
  offsets: { headerBytes: 4, sequence: 8, step: 16, width: 24, height: 28, format: 32, flags: 33, payloadBytes: 36 },
  bytes: 40,
  formats: { jpeg: 1, rgba8: 2 },
  flags: { held: 1, video: 2 },
} as const;

/** Where the page listens to the audio the runtime mixes. */
export const AUDIO_STREAM_PATH = "/__rusty/product/runtime/audio";

/** The streamed audio: interleaved signed 16-bit little-endian PCM. */
export const AUDIO_STREAM_FORMAT = { sampleRate: 48000, channels: 2 } as const;

/** Where the page reports how it presents the product. */
export const PRESENTATION_PATH = "/__rusty/product/runtime/presentation";

export type ControllerAxis = "axis-0" | "axis-1" | "axis-2" | "axis-3";

export type ControllerButton = "button-0" | "button-1" | "button-2" | "button-3" | "button-4" | "button-5" | "button-6" | "button-7" | "button-8" | "button-9" | "button-10" | "button-11" | "button-12" | "button-13" | "button-14" | "button-15";

export type JsonValue = number | string | boolean | Array<JsonValue> | { [key in string]?: JsonValue } | null;

export type KeyboardControl = "key-a" | "key-b" | "key-c" | "key-d" | "key-e" | "key-f" | "key-g" | "key-h" | "key-i" | "key-j" | "key-k" | "key-l" | "key-m" | "key-n" | "key-o" | "key-p" | "key-q" | "key-r" | "key-s" | "key-t" | "key-u" | "key-v" | "key-w" | "key-x" | "key-y" | "key-z" | "digit-0" | "digit-1" | "digit-2" | "digit-3" | "digit-4" | "digit-5" | "digit-6" | "digit-7" | "digit-8" | "digit-9" | "space" | "enter" | "escape" | "shift-left" | "shift-right" | "control-left" | "control-right" | "alt-left" | "alt-right" | "arrow-up" | "arrow-down" | "arrow-left" | "arrow-right";

export type PointerButton = "primary" | "secondary" | "middle";

/**
 * How gameplay holds the pointer.
 */
export type ProductHostCursorMode = "pointer-lock" | "unlocked" | "confined";

/**
 * What the page reports about its presentation.
 */
export type ProductHostPresentationReport = { cssWidth: number, cssHeight: number, devicePixelRatio: number, uiScale: number, anchors: Array<ProductHostViewportAnchorReport>, };

/**
 * Where the runtime presents the frames it renders.
 */
export type ProductHostRenderOutput = "stream" | "window";

/**
 * One anchored element's rect, normalized to the presentation surface,
 * bottom-left based.
 */
export type ProductHostViewportAnchorReport = { name: string, x: number, y: number, width: number, height: number, };

/**
 * The runtime binding an envelope belongs to, as canonical decimal text.
 */
export type RuntimeInputWireBinding = { instanceId: string, generation: string, controlRevision: string, };

/**
 * Why a host cleared held input.
 */
export type RuntimeInputWireClearReason = "focus-loss" | "interaction-mode-loss" | "pointer-lock-loss" | "restart" | "control-revision-change" | "dispose" | "ingress-overflow";

export type RuntimeInputWireEdge = "pressed" | "released";

/**
 * One input envelope a host submits, in observation order: a physical fact
 * or an intent claimed by product UI.
 */
export type RuntimeInputWireEvent = RuntimeInputWirePhysical | RuntimeInputWireIntentClaim;

/**
 * A physical fact from the closed Engine input catalog.
 */
export type RuntimeInputWireFact = { "kind": "key", code: KeyboardControl, edge: RuntimeInputWireEdge, } | { "kind": "pointer-button", button: PointerButton, edge: RuntimeInputWireEdge, position?: RuntimeInputWirePointerPosition, } | { "kind": "pointer-delta", x: number, y: number, } | { "kind": "pointer-position", x: number, y: number, } | { "kind": "wheel", x: number, y: number, } | { "kind": "controller-button", button: ControllerButton, edge: RuntimeInputWireEdge, } | { "kind": "controller-axis", axis: ControllerAxis, value: number, } | { "kind": "controller-button-value", button: ControllerButton, value: number, } | { "kind": "clear", reason: RuntimeInputWireClearReason, };

/**
 * A product-declared intent claimed by product UI, in the same ordered lane.
 */
export type RuntimeInputWireIntentClaim = { runtime: RuntimeInputWireBinding, 
/**
 * Canonical decimal u64, increasing within the binding.
 */
sequence: string, 
/**
 * Product-declared input context.
 */
context: string, 
/**
 * Product-declared intent name.
 */
intent: string, value: RuntimeInputWireIntentValue, };

export type RuntimeInputWireIntentValue = { "kind": "digital", active: boolean, } | { "kind": "axis", value: number, } | { "kind": "product-payload", contract: string, data: JsonValue, };

/**
 * A physical input fact observed for one runtime binding.
 */
export type RuntimeInputWirePhysical = { runtime: RuntimeInputWireBinding, 
/**
 * Canonical decimal u64, increasing within the binding.
 */
sequence: string, 
/**
 * Product-declared input context.
 */
context: string, fact: RuntimeInputWireFact, };

/**
 * A cursor position on the presentation surface, normalized and
 * bottom-left based like a camera viewport.
 */
export type RuntimeInputWirePointerPosition = { x: number, y: number, };

/**
 * The wire shape of a UI projection envelope: what the browser host
 * receives and passes to the mounted product UI.
 */
export type RuntimeUiProjectionEnvelope = { artifact: "rusty.product.ui-projection", runtime: RuntimeUiRuntimeWire, 
/**
 * Canonical decimal u64, increasing within the runtime binding.
 */
sequence: string, stream: string, 
/**
 * The product's projection contract identity.
 */
contract: string, 
/**
 * The product-owned projection value.
 */
value: JsonValue, };

/**
 * The runtime binding a projection belongs to, as canonical decimal text.
 */
export type RuntimeUiRuntimeWire = { instanceId: string, generation: string, controlRevision: string, };
