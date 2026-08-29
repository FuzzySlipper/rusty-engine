/**
 * Browser-only capture for the public application host. The values emitted here
 * are deliberately host-neutral: DOM events, canvas ownership, and pointer lock
 * do not cross the application-host boundary.
 */

export const RUSTY_APPLICATION_INPUT_QUEUE_MAXIMUM = 1_024;
export const RUSTY_APPLICATION_INPUT_POINTER_DELTA_MAXIMUM = 256;
export const RUSTY_APPLICATION_INPUT_WHEEL_DELTA_MAXIMUM = 256;
export const RUSTY_APPLICATION_INPUT_SELECTED_CONTROLLER_MAXIMUM = 3;
export const RUSTY_APPLICATION_INPUT_U64_MAXIMUM = 18_446_744_073_709_551_615n;
/** Mirrors the Engine runtime's direct product-payload bound. */
export const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_BYTES_MAXIMUM = 65_536;
export const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_DEPTH_MAXIMUM = 32;
export const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_NODES_MAXIMUM = 4_096;
export const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_STRING_BYTES_MAXIMUM = 16_384;
export const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_COLLECTION_MAXIMUM = 1_024;
export const RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_SAFE_INTEGER_MAXIMUM = 9_007_199_254_740_991;

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
  /** Engine-published next sequence for this runtime epoch. Generic hosts default to zero. */
  readonly nextSequence?: string;
}

/** Mirrors the closed Engine input control catalog; navigation keys are not admitted yet. */
export type RustyApplicationKeyboardControl =
  | 'key-a' | 'key-b' | 'key-c' | 'key-d' | 'key-e' | 'key-f' | 'key-g'
  | 'key-h' | 'key-i' | 'key-j' | 'key-k' | 'key-l' | 'key-m' | 'key-n'
  | 'key-o' | 'key-p' | 'key-q' | 'key-r' | 'key-s' | 'key-t' | 'key-u'
  | 'key-v' | 'key-w' | 'key-x' | 'key-y' | 'key-z'
  | 'digit-0' | 'digit-1' | 'digit-2' | 'digit-3' | 'digit-4'
  | 'digit-5' | 'digit-6' | 'digit-7' | 'digit-8' | 'digit-9'
  | 'space' | 'enter' | 'escape'
  | 'shift-left' | 'shift-right'
  | 'control-left' | 'control-right'
  | 'alt-left' | 'alt-right';

export type RustyApplicationPointerButton = 'primary' | 'secondary' | 'middle';
export type RustyApplicationControllerButton =
  | 'button-0' | 'button-1' | 'button-2' | 'button-3'
  | 'button-4' | 'button-5' | 'button-6' | 'button-7'
  | 'button-8' | 'button-9' | 'button-10' | 'button-11'
  | 'button-12' | 'button-13' | 'button-14' | 'button-15';
export type RustyApplicationControllerAxis = 'axis-0' | 'axis-1' | 'axis-2' | 'axis-3';
export type RustyApplicationInputEdge = 'pressed' | 'released';
export type RustyApplicationInputClearReason =
  | 'focus-loss'
  | 'ingress-overflow'
  | 'interaction-mode-loss'
  | 'pointer-lock-loss'
  | 'restart'
  | 'control-revision-change'
  | 'dispose';

/** Structural mirror of the Engine runtime physical ingress wire. */
export type RustyApplicationRuntimeInputFact =
  | {
      readonly kind: 'key';
      readonly code: RustyApplicationKeyboardControl;
      readonly edge: RustyApplicationInputEdge;
    }
  | {
      readonly kind: 'pointer-button';
      readonly button: RustyApplicationPointerButton;
      readonly edge: RustyApplicationInputEdge;
    }
  | { readonly kind: 'pointer-delta'; readonly x: number; readonly y: number }
  | { readonly kind: 'wheel'; readonly x: number; readonly y: number }
  | {
      readonly kind: 'controller-button';
      readonly button: RustyApplicationControllerButton;
      readonly edge: RustyApplicationInputEdge;
    }
  | {
      readonly kind: 'controller-axis';
      readonly axis: RustyApplicationControllerAxis;
      readonly value: number;
    }
  | { readonly kind: 'clear'; readonly reason: RustyApplicationInputClearReason };

export interface RustyApplicationRuntimeInputIngress {
  readonly runtime: RustyApplicationRuntimeIdentity;
  /** Canonical unsigned 64-bit decimal text scoped to the runtime epoch. */
  readonly sequence: string;
  readonly context: string;
  readonly fact: RustyApplicationRuntimeInputFact;
}

export type RustyApplicationRuntimeIntentValue =
  | { readonly kind: 'digital'; readonly active: boolean }
  | { readonly kind: 'axis'; readonly value: number }
  | {
      readonly kind: 'product-payload';
      /** Stable product schema identity; Rust matches it to the descriptor. */
      readonly contract: string;
      /** Bounded plain JSON only; never a callback, DOM object, or command route. */
      readonly data: RustyApplicationProductPayloadJson;
    };

export type RustyApplicationProductPayloadJson =
  | null
  | boolean
  | number
  | string
  | readonly RustyApplicationProductPayloadJson[]
  | RustyApplicationProductPayloadJsonObject;

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

export type RustyApplicationRuntimeInputEnvelope =
  | RustyApplicationRuntimeInputIngress
  | RustyApplicationRuntimeDirectIntentClaim;

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

interface NormalizedInputOptions {
  readonly initialBinding: RustyApplicationRuntimeInputBinding | null;
  readonly maximumPointerDelta: number;
  readonly maximumQueue: number;
  readonly maximumWheelDelta: number;
  readonly onAvailable: (() => void) | null;
  readonly selectedController: number | null;
}

/**
 * Creates the optional DOM adapter. It owns capture and cleanup only; no look
 * integration, movement, action mapping, or cadence enters this module.
 */
export function createRustyApplicationInputIngress(
  options: RustyApplicationRuntimeInputOptions,
  environment: RustyApplicationInputIngressEnvironment,
): RustyApplicationManagedInputIngress {
  const normalized = normalizeOptions(options);
  const queue = createRustyApplicationInputQueue(normalized.maximumQueue);
  const heldKeys = new Set<RustyApplicationKeyboardControl>();
  const heldPointerButtons = new Set<RustyApplicationPointerButton>();
  const controllerAxes = new Map<RustyApplicationControllerAxis, number>();
  const heldControllerButtons = new Set<RustyApplicationControllerButton>();
  let attachedCanvas = environment.canvas();
  let disposed = false;

  const pointerLocked = (): boolean => environment.document.pointerLockElement === environment.canvas();
  const gameplayFocused = (): boolean => pointerLocked()
    || environment.document.activeElement === environment.canvas();
  const clearLocal = (): void => {
    heldKeys.clear();
    heldPointerButtons.clear();
    heldControllerButtons.clear();
    controllerAxes.clear();
  };
  const clear = (reason: RustyApplicationInputClearReason): void => {
    clearLocal();
    queue.clear(reason);
    normalized.onAvailable?.();
  };
  const enqueueFact = (fact: RustyApplicationRuntimeInputFact): boolean => {
    const overflowed = queue.enqueueFact(fact);
    if (overflowed) clearLocal();
    normalized.onAvailable?.();
    return overflowed;
  };
  const admit = (event: Event, requiresFocus: boolean): boolean => {
    const allowed = environment.allowsGameplayInput(event);
    if (!allowed) {
      clear('interaction-mode-loss');
      return false;
    }
    if (requiresFocus && !gameplayFocused()) {
      clear('focus-loss');
      return false;
    }
    return true;
  };
  const onPointerDown = (event: PointerEvent): void => {
    if (!admit(event, false)) return;
    const button = normalizePointerButton(event.button);
    if (button === null) return;
    if (!heldPointerButtons.has(button)) {
      heldPointerButtons.add(button);
      enqueueFact(Object.freeze({ kind: 'pointer-button', button, edge: 'pressed' }));
    }
    if (button === 'primary') environment.focusGameplay();
  };
  const onPointerUp = (event: PointerEvent): void => {
    if (!admit(event, true)) return;
    const button = normalizePointerButton(event.button);
    if (button === null || !heldPointerButtons.delete(button)) return;
    enqueueFact(Object.freeze({ kind: 'pointer-button', button, edge: 'released' }));
  };
  const onPointerCancel = (event: PointerEvent): void => {
    environment.allowsGameplayInput(event);
    clear('interaction-mode-loss');
  };
  const onPointerMove = (event: PointerEvent): void => {
    if (!admit(event, false)) return;
    if (!pointerLocked()) return;
    const x = boundedNumber(event.movementX, normalized.maximumPointerDelta);
    const y = boundedNumber(event.movementY, normalized.maximumPointerDelta);
    if (x === 0 && y === 0) return;
    // The canonical convention is intentionally raw here: rightward pointer movement is +X/yaw.
    enqueueFact(Object.freeze({ kind: 'pointer-delta', x, y }));
  };
  const onWheel = (event: WheelEvent): void => {
    if (!admit(event, true)) return;
    const x = boundedNumber(event.deltaX, normalized.maximumWheelDelta);
    const y = boundedNumber(event.deltaY, normalized.maximumWheelDelta);
    if (x === 0 && y === 0) return;
    enqueueFact(Object.freeze({ kind: 'wheel', x, y }));
  };
  const onKeyDown = (event: KeyboardEvent): void => {
    if (!admit(event, true)) return;
    const code = normalizeRustyApplicationKeyboardControl(event.code);
    if (code === null || heldKeys.has(code)) return;
    heldKeys.add(code);
    enqueueFact(Object.freeze({ kind: 'key', code, edge: 'pressed' }));
  };
  const onKeyUp = (event: KeyboardEvent): void => {
    if (!admit(event, true)) return;
    const code = normalizeRustyApplicationKeyboardControl(event.code);
    if (code === null || !heldKeys.delete(code)) return;
    enqueueFact(Object.freeze({ kind: 'key', code, edge: 'released' }));
  };
  const onPointerLockChange = (event: Event): void => {
    // Pointer lock changes are DOM events too, even though losing it must clear regardless.
    environment.allowsGameplayInput(event);
    if (!pointerLocked()) clear('pointer-lock-loss');
  };
  const onWindowBlur = (event: Event): void => {
    environment.allowsGameplayInput(event);
    clear('focus-loss');
  };
  const onGamepadConnected = (event: GamepadEvent): void => {
    // Controller observation is caller-sampled. Keep the browser event inside
    // the same UI-arbitration boundary without creating a second cadence.
    environment.allowsGameplayInput(event);
  };
  const onGamepadDisconnected = (event: GamepadEvent): void => {
    environment.allowsGameplayInput(event);
    if (normalized.selectedController === event.gamepad.index) clear('interaction-mode-loss');
  };

  const attachEventTarget = (): void => {
    environment.eventTarget.addEventListener('pointerdown', onPointerDown);
    environment.document.addEventListener('pointerup', onPointerUp);
    environment.document.addEventListener('pointercancel', onPointerCancel);
    environment.document.addEventListener('wheel', onWheel, { passive: true });
  };
  const detachEventTarget = (): void => {
    environment.eventTarget.removeEventListener('pointerdown', onPointerDown);
    environment.document.removeEventListener('pointerup', onPointerUp);
    environment.document.removeEventListener('pointercancel', onPointerCancel);
    environment.document.removeEventListener('wheel', onWheel);
  };
  const sampleController = (): number => {
    if (disposed || normalized.selectedController === null) return 0;
    if (!environment.active() || environment.interactionMode() !== 'gameplay' || !gameplayFocused()) {
      clear('interaction-mode-loss');
      return 0;
    }
    const controller = environment.gamepads()[normalized.selectedController];
    if (controller === null || controller === undefined || !controller.connected) {
      if (heldControllerButtons.size > 0 || controllerAxes.size > 0) clear('interaction-mode-loss');
      return 0;
    }
    let observed = 0;
    for (let index = 0; index < 4; index += 1) {
      const axis = controllerAxis(index);
      const value = boundedNumber(controller.axes[index] ?? 0, 1);
      const prior = controllerAxes.get(axis) ?? 0;
      if (value === prior) continue;
      controllerAxes.set(axis, value);
      if (enqueueFact(Object.freeze({ kind: 'controller-axis', axis, value }))) return observed;
      observed += 1;
    }
    for (let index = 0; index < 16; index += 1) {
      const button = controllerButton(index);
      const pressed = controller.buttons[index]?.pressed === true;
      const wasPressed = heldControllerButtons.has(button);
      if (pressed === wasPressed) continue;
      if (pressed) heldControllerButtons.add(button);
      else heldControllerButtons.delete(button);
      if (enqueueFact(Object.freeze({
        kind: 'controller-button', button, edge: pressed ? 'pressed' : 'released',
      }))) return observed;
      observed += 1;
    }
    return observed;
  };

  attachEventTarget();
  environment.document.addEventListener('pointermove', onPointerMove);
  environment.document.addEventListener('keydown', onKeyDown);
  environment.document.addEventListener('keyup', onKeyUp);
  environment.document.addEventListener('pointerlockchange', onPointerLockChange);
  environment.document.defaultView?.addEventListener('blur', onWindowBlur);
  environment.document.defaultView?.addEventListener('gamepadconnected', onGamepadConnected);
  environment.document.defaultView?.addEventListener('gamepaddisconnected', onGamepadDisconnected);
  if (normalized.initialBinding !== null) queue.bindRuntime(normalized.initialBinding);

  return Object.freeze({
    bindRuntime: (binding: RustyApplicationRuntimeInputBinding) => {
      if (disposed) return;
      if (queue.bindRuntime(binding)) clearLocal();
    },
    synchronizeRuntime: (binding: RustyApplicationRuntimeInputBinding) => {
      if (disposed) return;
      if (queue.bindRuntime(binding)) clearLocal();
    },
    setContext: (context: string) => {
      if (disposed) return;
      if (queue.setContext(context)) clearLocal();
    },
    clear: (reason: RustyApplicationInputClearReason) => {
      if (!disposed) clear(reason);
    },
    drain: () => queue.drain(),
    claim: (intent: string, value: RustyApplicationRuntimeIntentValue) => {
      if (!disposed) {
        if (queue.claim(intent, value)) clearLocal();
        normalized.onAvailable?.();
      }
    },
    sampleController,
    rebindCanvas: (canvas: HTMLCanvasElement) => {
      if (disposed || canvas === attachedCanvas) return;
      attachedCanvas = canvas;
      clear('pointer-lock-loss');
    },
    dispose: () => {
      if (disposed) return;
      clear('dispose');
      disposed = true;
      detachEventTarget();
      environment.document.removeEventListener('pointermove', onPointerMove);
      environment.document.removeEventListener('keydown', onKeyDown);
      environment.document.removeEventListener('keyup', onKeyUp);
      environment.document.removeEventListener('pointerlockchange', onPointerLockChange);
      environment.document.defaultView?.removeEventListener('blur', onWindowBlur);
      environment.document.defaultView?.removeEventListener('gamepadconnected', onGamepadConnected);
      environment.document.defaultView?.removeEventListener('gamepaddisconnected', onGamepadDisconnected);
    },
  });
}

/** Strictly normalize DOM keyboard codes before they become host-neutral observations. */
export function normalizeRustyApplicationKeyboardControl(
  code: string,
): RustyApplicationKeyboardControl | null {
  const alpha = /^Key([A-Z])$/u.exec(code);
  if (alpha !== null) return `key-${alpha[1]!.toLowerCase()}` as RustyApplicationKeyboardControl;
  const digit = /^Digit([0-9])$/u.exec(code);
  if (digit !== null) return `digit-${digit[1]!}` as RustyApplicationKeyboardControl;
  const mapped = KEYBOARD_CODE_MAP.get(code);
  return mapped ?? null;
}

/**
 * The optional initial sequence exists for boundary tests. Production ingress
 * always begins a newly bound epoch at zero.
 */
export function createRustyApplicationInputQueue(
  maximumQueue: number,
  initialSequence = 0n,
): RustyApplicationInputQueue {
  if (initialSequence < 0n || initialSequence > RUSTY_APPLICATION_INPUT_U64_MAXIMUM) {
    throw new RangeError('initial input sequence must fit u64');
  }
  let binding: RustyApplicationRuntimeInputBinding | null = null;
  let sequence = initialSequence;
  let initialBinding = true;
  let terminal = false;
  let entries: RustyApplicationRuntimeInputEnvelope[] = [];
  const nextSequence = (): string | null => {
    if (terminal || sequence >= RUSTY_APPLICATION_INPUT_U64_MAXIMUM) return null;
    const result = sequence.toString(10);
    sequence += 1n;
    return result;
  };
  const terminalClear = (): void => {
    if (binding === null || terminal) return;
    terminal = true;
    const firstDiscarded = entries[0];
    const terminalSequence = firstDiscarded?.sequence
      ?? RUSTY_APPLICATION_INPUT_U64_MAXIMUM.toString(10);
    if (firstDiscarded !== undefined) sequence = BigInt(terminalSequence) + 1n;
    entries = [freezeIngress(
      binding,
      terminalSequence,
      Object.freeze({ kind: 'clear', reason: 'ingress-overflow' }),
    )];
  };
  const replaceQueuedWithClear = (reason: RustyApplicationInputClearReason): void => {
    if (binding === null || terminal) return;
    const firstDiscarded = entries[0];
    const clearSequence = firstDiscarded === undefined ? nextSequence() : firstDiscarded.sequence;
    if (clearSequence === null) {
      terminalClear();
      return;
    }
    if (firstDiscarded !== undefined) sequence = BigInt(clearSequence) + 1n;
    entries = [freezeIngress(binding, clearSequence, Object.freeze({ kind: 'clear', reason }))];
  };
  const appendFact = (fact: RustyApplicationRuntimeInputFact): boolean => {
    if (binding === null) return false;
    if (terminal) return true;
    if (entries.length >= maximumQueue) {
      replaceQueuedWithClear('ingress-overflow');
      return true;
    }
    const next = nextSequence();
    if (next === null) {
      terminalClear();
      return true;
    }
    entries.push(freezeIngress(binding, next, fact));
    return false;
  };
  const appendClaim = (intent: string, value: RustyApplicationRuntimeIntentValue): boolean => {
    if (binding === null) return false;
    if (terminal) return true;
    if (entries.length >= maximumQueue) {
      replaceQueuedWithClear('ingress-overflow');
      return true;
    }
    const next = nextSequence();
    if (next === null) {
      terminalClear();
      return true;
    }
    entries.push(freezeClaim(binding, next, intent, value));
    return false;
  };
  return {
    bindRuntime: (next) => {
      const normalized = validateBinding(next);
      const previous = binding;
      if (previous !== null && sameBinding(previous, normalized)) return false;
      // An exhausted epoch cannot adopt a different context. Its one terminal
      // clear is the final wire value for that epoch; only a newer epoch can
      // reset the sequence and recover input.
      if (terminal && previous !== null && sameRuntime(previous.runtime, normalized.runtime)) {
        return false;
      }
      if (previous !== null && previous.runtime.instanceId === normalized.runtime.instanceId) {
        const priorGeneration = BigInt(previous.runtime.generation);
        const nextGeneration = BigInt(normalized.runtime.generation);
        if (nextGeneration < priorGeneration) {
          throw new RangeError('runtime generation cannot move backward within one instance');
        }
        if (nextGeneration > priorGeneration
          && BigInt(normalized.runtime.controlRevision) <= BigInt(previous.runtime.controlRevision)) {
          throw new RangeError('runtime control revision must advance with generation');
        }
        if (nextGeneration === priorGeneration
          && BigInt(normalized.runtime.controlRevision) < BigInt(previous.runtime.controlRevision)) {
          throw new RangeError('runtime control revision cannot move backward within one generation');
        }
      }
      if (previous === null) {
        binding = normalized;
        sequence = normalized.nextSequence === undefined
          ? (initialBinding ? initialSequence : 0n)
          : BigInt(normalized.nextSequence);
        initialBinding = false;
        terminal = false;
        return true;
      }
      if (sameRuntime(previous.runtime, normalized.runtime)) {
        if (previous.context === normalized.context) return false;
        binding = normalized;
        replaceQueuedWithClear('interaction-mode-loss');
        return true;
      }
      const reason: RustyApplicationInputClearReason = previous.runtime.instanceId !== normalized.runtime.instanceId
        || previous.runtime.generation !== normalized.runtime.generation
        ? 'restart'
        : 'control-revision-change';
      binding = normalized;
      sequence = normalized.nextSequence === undefined ? 0n : BigInt(normalized.nextSequence);
      terminal = false;
      entries = [];
      replaceQueuedWithClear(reason);
      return true;
    },
    setContext: (context) => {
      const normalized = validateContext(context);
      if (binding === null || terminal || binding.context === normalized) return false;
      binding = Object.freeze({ runtime: binding.runtime, context: normalized });
      replaceQueuedWithClear('interaction-mode-loss');
      return true;
    },
    clear: (reason) => {
      replaceQueuedWithClear(validateClearReason(reason));
    },
    enqueueFact: (fact) => {
      return appendFact(validateInputFact(fact));
    },
    claim: (intent, value) => {
      if (binding === null) return false;
      const normalizedIntent = validateIntent(intent);
      const normalizedValue = validateIntentValue(value);
      return appendClaim(normalizedIntent, normalizedValue);
    },
    drain: () => {
      const drained = entries;
      entries = [];
      return Object.freeze(drained);
    },
  };
}

function freezeIngress(
  binding: RustyApplicationRuntimeInputBinding,
  sequence: string,
  fact: RustyApplicationRuntimeInputFact,
): RustyApplicationRuntimeInputIngress {
  return Object.freeze({
    runtime: binding.runtime,
    sequence,
    context: binding.context,
    fact: Object.freeze({ ...fact }) as RustyApplicationRuntimeInputFact,
  });
}

function freezeClaim(
  binding: RustyApplicationRuntimeInputBinding,
  sequence: string,
  intent: string,
  value: RustyApplicationRuntimeIntentValue,
): RustyApplicationRuntimeDirectIntentClaim {
  const normalizedIntent = validateIntent(intent);
  const normalizedValue = validateIntentValue(value);
  return Object.freeze({
    runtime: binding.runtime,
    sequence,
    context: binding.context,
    intent: normalizedIntent,
    value: normalizedValue,
  });
}

function normalizeOptions(options: RustyApplicationRuntimeInputOptions): NormalizedInputOptions {
  return Object.freeze({
    initialBinding: options.binding === undefined ? null : validateBinding(options.binding),
    maximumPointerDelta: boundedPositiveInteger(
      options.maximumPointerDelta ?? RUSTY_APPLICATION_INPUT_POINTER_DELTA_MAXIMUM,
      'maximumPointerDelta',
      4_096,
    ),
    maximumQueue: boundedPositiveInteger(
      options.maximumQueue ?? RUSTY_APPLICATION_INPUT_QUEUE_MAXIMUM,
      'maximumQueue',
      RUSTY_APPLICATION_INPUT_QUEUE_MAXIMUM,
    ),
    maximumWheelDelta: boundedPositiveInteger(
      options.maximumWheelDelta ?? RUSTY_APPLICATION_INPUT_WHEEL_DELTA_MAXIMUM,
      'maximumWheelDelta',
      4_096,
    ),
    onAvailable: options.onAvailable === undefined
      ? null
      : requireInputAvailabilityCallback(options.onAvailable),
    selectedController: options.selectedController === undefined
      ? null
      : boundedInteger(
        options.selectedController.index,
        'selectedController.index',
        0,
        RUSTY_APPLICATION_INPUT_SELECTED_CONTROLLER_MAXIMUM,
      ),
  });
}

function requireInputAvailabilityCallback(value: unknown): () => void {
  if (typeof value !== 'function') {
    throw new TypeError('onAvailable must be a function');
  }
  return value as () => void;
}

function validateBinding(binding: RustyApplicationRuntimeInputBinding): RustyApplicationRuntimeInputBinding {
  if (typeof binding !== 'object' || binding === null || typeof binding.runtime !== 'object'
    || binding.runtime === null) {
    throw new TypeError('runtime input binding must include one runtime identity');
  }
  return Object.freeze({
    runtime: Object.freeze({
      instanceId: validateCanonicalU64(binding.runtime.instanceId, 'runtime.instanceId'),
      generation: validateCanonicalU64(binding.runtime.generation, 'runtime.generation'),
      controlRevision: validateCanonicalU64(binding.runtime.controlRevision, 'runtime.controlRevision'),
    }),
    context: validateContext(binding.context),
    ...(binding.nextSequence === undefined
      ? {}
      : { nextSequence: validateCanonicalU64(binding.nextSequence, 'nextSequence') }),
  });
}

function validateCanonicalU64(value: string, name: string): string {
  if (typeof value !== 'string' || !/^(?:0|[1-9][0-9]*)$/u.test(value)) {
    throw new TypeError(`${name} must be canonical unsigned decimal text`);
  }
  const parsed = BigInt(value);
  if (parsed > 18_446_744_073_709_551_615n) {
    throw new RangeError(`${name} exceeds u64`);
  }
  return value;
}

function validateContext(value: string): string {
  return validateProductIdentity(value, 'input context');
}

function validateIntent(intent: string): string {
  return validateProductIdentity(intent, 'direct UI intent');
}

function validateProductIdentity(value: string, name: string): string {
  if (typeof value !== 'string' || new TextEncoder().encode(value).byteLength > 128
    || !/^[a-z0-9](?:[a-z0-9]|[._-](?=[a-z0-9]))*$/u.test(value)) {
    throw new TypeError(`${name} must be a 1..128 byte lowercase product identity`);
  }
  return value;
}

function validateIntentValue(
  value: RustyApplicationRuntimeIntentValue,
): RustyApplicationRuntimeIntentValue {
  if (value.kind === 'digital') {
    if (typeof value.active !== 'boolean') throw new TypeError('digital intent claim requires boolean active');
    return Object.freeze({ kind: 'digital', active: value.active });
  }
  if (value.kind === 'axis') {
    if (!Number.isFinite(value.value) || value.value < -1 || value.value > 1) {
      throw new RangeError('axis intent claim value must be finite and within [-1, 1]');
    }
    return Object.freeze({ kind: 'axis', value: value.value });
  }
  if (value.kind === 'product-payload') {
    return Object.freeze({
      kind: 'product-payload',
      contract: validateProductIdentity(value.contract, 'product payload contract'),
      data: normalizeProductPayloadJson(value.data),
    });
  }
  throw new TypeError('direct UI intent claim has an unknown value kind');
}

interface ProductPayloadJsonBudget {
  nodes: number;
  readonly active: WeakSet<object>;
}

/**
 * Clones only bounded plain JSON into an immutable data value. This lives at
 * the browser ingress boundary because `claim` is a public trusted-UI API;
 * Rust validates the same shape and budget again before adapter delivery.
 */
function normalizeProductPayloadJson(value: unknown): RustyApplicationProductPayloadJson {
  const normalized = normalizeProductPayloadJsonValue(value, '$.data', 1, {
    nodes: 0,
    active: new WeakSet<object>(),
  });
  const bytes = new TextEncoder().encode(JSON.stringify(normalized)).byteLength;
  if (bytes > RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_BYTES_MAXIMUM) {
    throw new RangeError('product payload JSON exceeds its Rust-owned byte limit');
  }
  return normalized;
}

function normalizeProductPayloadJsonValue(
  value: unknown,
  path: string,
  depth: number,
  budget: ProductPayloadJsonBudget,
): RustyApplicationProductPayloadJson {
  if (depth > RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_DEPTH_MAXIMUM) {
    throw new RangeError(`product payload JSON depth exceeds its limit at ${path}`);
  }
  budget.nodes += 1;
  if (budget.nodes > RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_NODES_MAXIMUM) {
    throw new RangeError('product payload JSON exceeds its Rust-owned node limit');
  }
  if (value === null || typeof value === 'boolean') return value;
  if (typeof value === 'string') {
    validateProductPayloadString(value, path);
    return value;
  }
  if (typeof value === 'number') {
    if (!Number.isFinite(value) || (Number.isInteger(value)
      && Math.abs(value) > RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_SAFE_INTEGER_MAXIMUM)) {
      throw new TypeError(`product payload JSON number is invalid at ${path}`);
    }
    return Object.is(value, -0) ? 0 : value;
  }
  if (typeof value !== 'object') {
    throw new TypeError(`product payload JSON cannot contain ${typeof value} at ${path}`);
  }
  if (budget.active.has(value)) {
    throw new TypeError(`product payload JSON cannot contain a cycle at ${path}`);
  }
  budget.active.add(value);
  try {
    if (Array.isArray(value)) {
      if (Object.getPrototypeOf(value) !== Array.prototype
        || value.length > RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_COLLECTION_MAXIMUM) {
        throw new TypeError(`product payload JSON array is invalid at ${path}`);
      }
      const output: RustyApplicationProductPayloadJson[] = [];
      for (let index = 0; index < value.length; index += 1) {
        if (!Object.hasOwn(value, index)) {
          throw new TypeError(`product payload JSON arrays cannot contain holes at ${path}`);
        }
        const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
        if (descriptor === undefined || !descriptor.enumerable || !('value' in descriptor)) {
          throw new TypeError(`product payload JSON arrays cannot contain accessors at ${path}`);
        }
        output.push(normalizeProductPayloadJsonValue(value[index], `${path}[${String(index)}]`, depth + 1, budget));
      }
      if (Reflect.ownKeys(value).some((key) => key !== 'length'
        && (typeof key !== 'string' || !isProductPayloadArrayIndex(key, value.length)))) {
        throw new TypeError(`product payload JSON arrays cannot contain extra properties at ${path}`);
      }
      return Object.freeze(output);
    }
    const prototype = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) {
      throw new TypeError(`product payload JSON objects must be plain data at ${path}`);
    }
    const descriptors = Object.getOwnPropertyDescriptors(value);
    const keys = Object.keys(value).sort(compareProductPayloadUtf8);
    if (keys.length > RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_COLLECTION_MAXIMUM
      || Reflect.ownKeys(descriptors).some((key) => typeof key !== 'string'
        || descriptors[key] === undefined || !descriptors[key]!.enumerable
        || !('value' in descriptors[key]!))) {
      throw new TypeError(`product payload JSON objects cannot contain accessors or hidden fields at ${path}`);
    }
    const output: Record<string, RustyApplicationProductPayloadJson> = Object.create(null) as Record<string, RustyApplicationProductPayloadJson>;
    for (const key of keys) {
      validateProductPayloadString(key, `${path}.<key>`);
      Object.defineProperty(output, key, {
        value: normalizeProductPayloadJsonValue((value as Record<string, unknown>)[key], `${path}.${key}`, depth + 1, budget),
        enumerable: true, configurable: false, writable: false,
      });
    }
    return Object.freeze(output);
  } finally {
    budget.active.delete(value);
  }
}

function isProductPayloadArrayIndex(key: string, length: number): boolean {
  if (key !== '0' && !/^[1-9][0-9]*$/u.test(key)) return false;
  const index = Number(key);
  return Number.isSafeInteger(index)
    && index < 4_294_967_295
    && index < length
    && String(index) === key;
}

function validateProductPayloadString(value: string, path: string): void {
  if (new TextEncoder().encode(value).byteLength > RUSTY_APPLICATION_INPUT_PRODUCT_PAYLOAD_STRING_BYTES_MAXIMUM) {
    throw new RangeError(`product payload JSON string exceeds its limit at ${path}`);
  }
  for (const scalar of value) {
    const code = scalar.codePointAt(0) as number;
    if (code >= 0xd800 && code <= 0xdfff) {
      throw new TypeError(`product payload JSON string is not scalar data at ${path}`);
    }
  }
}

function compareProductPayloadUtf8(left: string, right: string): number {
  const leftBytes = new TextEncoder().encode(left);
  const rightBytes = new TextEncoder().encode(right);
  const length = Math.min(leftBytes.length, rightBytes.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (leftBytes[index] as number) - (rightBytes[index] as number);
    if (difference !== 0) return difference;
  }
  return leftBytes.length - rightBytes.length;
}

function validateInputFact(
  value: RustyApplicationRuntimeInputFact,
): RustyApplicationRuntimeInputFact {
  if (typeof value !== 'object' || value === null) {
    throw new TypeError('runtime input fact must be an object');
  }
  switch (value.kind) {
    case 'key': {
      if (!KEYBOARD_CONTROLS.has(value.code) || !isInputEdge(value.edge)) {
        throw new TypeError('key input fact must use one closed keyboard control and edge');
      }
      return Object.freeze({ kind: 'key', code: value.code, edge: value.edge });
    }
    case 'pointer-button': {
      if (!isPointerButton(value.button) || !isInputEdge(value.edge)) {
        throw new TypeError('pointer button input fact must use one closed button and edge');
      }
      return Object.freeze({ kind: 'pointer-button', button: value.button, edge: value.edge });
    }
    case 'pointer-delta':
    case 'wheel': {
      if (!Number.isFinite(value.x) || !Number.isFinite(value.y)) {
        throw new TypeError(`${value.kind} input fact requires finite x and y`);
      }
      return Object.freeze({ kind: value.kind, x: value.x, y: value.y });
    }
    case 'controller-button': {
      if (!isControllerButton(value.button) || !isInputEdge(value.edge)) {
        throw new TypeError('controller button input fact must use one closed button and edge');
      }
      return Object.freeze({ kind: 'controller-button', button: value.button, edge: value.edge });
    }
    case 'controller-axis': {
      if (!isControllerAxis(value.axis) || !Number.isFinite(value.value)
        || value.value < -1 || value.value > 1) {
        throw new TypeError('controller axis input fact requires one closed axis within [-1, 1]');
      }
      return Object.freeze({ kind: 'controller-axis', axis: value.axis, value: value.value });
    }
    case 'clear':
      return Object.freeze({ kind: 'clear', reason: validateClearReason(value.reason) });
    default:
      throw new TypeError('runtime input fact has an unknown kind');
  }
}

function validateClearReason(value: RustyApplicationInputClearReason): RustyApplicationInputClearReason {
  if (INPUT_CLEAR_REASONS.has(value)) return value;
  throw new TypeError('runtime input clear must use one closed reason');
}

function sameBinding(
  left: RustyApplicationRuntimeInputBinding,
  right: RustyApplicationRuntimeInputBinding,
): boolean {
  return left.context === right.context
    && sameRuntime(left.runtime, right.runtime);
}

function sameRuntime(
  left: RustyApplicationRuntimeIdentity,
  right: RustyApplicationRuntimeIdentity,
): boolean {
  return left.instanceId === right.instanceId
    && left.generation === right.generation
    && left.controlRevision === right.controlRevision;
}

function normalizePointerButton(button: number): RustyApplicationPointerButton | null {
  if (button === 0) return 'primary';
  if (button === 1) return 'middle';
  if (button === 2) return 'secondary';
  return null;
}

function isInputEdge(value: unknown): value is RustyApplicationInputEdge {
  return value === 'pressed' || value === 'released';
}

function isPointerButton(value: unknown): value is RustyApplicationPointerButton {
  return value === 'primary' || value === 'secondary' || value === 'middle';
}

function isControllerButton(value: unknown): value is RustyApplicationControllerButton {
  return typeof value === 'string' && /^button-(?:[0-9]|1[0-5])$/u.test(value);
}

function isControllerAxis(value: unknown): value is RustyApplicationControllerAxis {
  return typeof value === 'string' && /^axis-[0-3]$/u.test(value);
}

function controllerButton(index: number): RustyApplicationControllerButton {
  return `button-${String(index)}` as RustyApplicationControllerButton;
}

function controllerAxis(index: number): RustyApplicationControllerAxis {
  return `axis-${String(index)}` as RustyApplicationControllerAxis;
}

function boundedNumber(value: number, maximum: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(-maximum, Math.min(maximum, value));
}

function boundedPositiveInteger(value: number, name: string, maximum: number): number {
  return boundedInteger(value, name, 1, maximum);
}

function boundedInteger(value: number, name: string, minimum: number, maximum: number): number {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new RangeError(`${name} must be a safe integer within [${String(minimum)}, ${String(maximum)}]`);
  }
  return value;
}

const KEYBOARD_CODE_MAP: ReadonlyMap<string, RustyApplicationKeyboardControl> = new Map([
  ['Space', 'space'], ['Enter', 'enter'], ['Escape', 'escape'],
  ['ShiftLeft', 'shift-left'], ['ShiftRight', 'shift-right'],
  ['ControlLeft', 'control-left'], ['ControlRight', 'control-right'],
  ['AltLeft', 'alt-left'], ['AltRight', 'alt-right'],
]);

const KEYBOARD_CONTROLS: ReadonlySet<RustyApplicationKeyboardControl> = new Set([
  ...Array.from({ length: 26 }, (_, index) => `key-${String.fromCharCode(97 + index)}` as RustyApplicationKeyboardControl),
  ...Array.from({ length: 10 }, (_, index) => `digit-${String(index)}` as RustyApplicationKeyboardControl),
  'space', 'enter', 'escape',
  'shift-left', 'shift-right', 'control-left', 'control-right', 'alt-left', 'alt-right',
]);

const INPUT_CLEAR_REASONS: ReadonlySet<RustyApplicationInputClearReason> = new Set([
  'focus-loss', 'ingress-overflow', 'interaction-mode-loss', 'pointer-lock-loss',
  'restart', 'control-revision-change', 'dispose',
]);
