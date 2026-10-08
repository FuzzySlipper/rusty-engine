/**
 * Browser-only capture for the public application host. The values emitted here
 * are deliberately host-neutral: DOM events, canvas ownership, and pointer lock
 * do not cross the application-host boundary.
 */

import type {
  ControllerAxis,
  ControllerButton,
  KeyboardControl,
  PointerButton,
  RuntimeInputWireBinding,
  RuntimeInputWireClearReason,
  RuntimeInputWireEvent,
  RuntimeInputWireFact,
  RuntimeInputWireIntentClaim,
  RuntimeInputWireIntentValue,
  RuntimeInputWirePhysical,
  RuntimeInputWirePointerPosition,
} from './generated/contracts.js';

export const RUSTY_APPLICATION_INPUT_QUEUE_MAXIMUM = 1_024;
export const RUSTY_APPLICATION_INPUT_WHEEL_DELTA_MAXIMUM = 256;
export const RUSTY_APPLICATION_INPUT_SELECTED_CONTROLLER_MAXIMUM = 3;
const U64_MAXIMUM = 18_446_744_073_709_551_615n;

/**
 * The runtime binding every wire shape carries, as canonical decimal text.
 * Input, UI projections and host results each declare it in Rust; they are
 * this one shape.
 */
export type RustyApplicationRuntimeIdentity = RuntimeInputWireBinding;

export interface RustyApplicationRuntimeInputBinding {
  readonly runtime: RustyApplicationRuntimeIdentity;
  /** Product-declared input context. The host preserves it but assigns no meaning. */
  readonly context: string;
  /** Engine-published next sequence for this runtime epoch. Generic hosts default to zero. */
  readonly nextSequence?: string;
}

import type { RustyApplicationInterfaceInputObservation } from './product-ui.js';
export type { RustyApplicationInterfaceInputObservation } from './product-ui.js';

export interface RustyApplicationSelectedControllerOptions {
  /** Browser gamepad index. Only one explicitly selected controller is observed. */
  readonly index: number;
}

export interface RustyApplicationRuntimeInputOptions {
  /** Initial runtime binding. Input remains inert until `bindRuntime` when omitted. */
  readonly binding?: RustyApplicationRuntimeInputBinding;
  /** Maximum queued physical facts and direct UI claims, inclusive of the fail-closed clear. */
  readonly maximumQueue?: number;
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
  /**
   * Replaces an uncertain ingress epoch with the current physical held-state
   * baseline. It deliberately discards queued DOM facts and UI claims: only
   * keyboard, pointer-button, and selected-controller state can be observed
   * again without replaying a possibly admitted mutation.
   */
  readonly rebaselineRuntime: (binding: RustyApplicationRuntimeInputBinding) => void;
  /** Change the product input context after clearing the old context's pending facts. */
  readonly setContext: (context: string) => void;
  /** Explicitly clear local held state and queue an ordered lifecycle clear fact. */
  readonly clear: (reason: RuntimeInputWireClearReason) => void;
  /** Drain the combined physical-input and direct-UI-claim lane in observation order. */
  readonly drain: () => readonly RuntimeInputWireEvent[];
  /** Claim one product-declared intent from trusted UI without mutating product state. */
  readonly claim: (intent: string, value: RuntimeInputWireIntentValue) => void;
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
  /** Cursor behavior is selected by the Engine-owned product host configuration. */
  readonly usesPointerLock?: () => boolean;
  readonly active: () => boolean;
  readonly focusGameplay: () => void;
  readonly gamepads: () => readonly (Gamepad | null)[];
  /** Exclusive interface delivery; these observations never enter the gameplay queue. */
  readonly observeInterfaceInput?: (observation: RustyApplicationInterfaceInputObservation) => void;
}

export interface RustyApplicationInputQueue {
  readonly bindRuntime: (binding: RustyApplicationRuntimeInputBinding) => boolean;
  /** Adopts a binding whose Engine lane already performed its mandatory clear. */
  readonly rebaseRuntime: (binding: RustyApplicationRuntimeInputBinding) => boolean;
  readonly setContext: (context: string) => boolean;
  readonly clear: (reason: RuntimeInputWireClearReason) => void;
  /** True means the bounded queue overflowed and now contains only a clear fact. */
  readonly enqueueFact: (fact: RuntimeInputWireFact) => boolean;
  /** True means the bounded queue overflowed and now contains only a clear fact. */
  readonly claim: (intent: string, value: RuntimeInputWireIntentValue) => boolean;
  readonly drain: () => readonly RuntimeInputWireEvent[];
}

export interface RustyApplicationManagedInputIngress extends RustyApplicationInputPort {
  /** Clear the old owner and adopt held controller state without replaying its press. */
  readonly interactionModeChanged: () => void;
  /** Application-host lifecycle seam; product callers use the owning host disposal instead. */
  readonly dispose: () => void;
}

interface NormalizedInputOptions {
  readonly initialBinding: RustyApplicationRuntimeInputBinding | null;
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
  const heldKeys = new Set<KeyboardControl>();
  const heldPointerButtons = new Set<PointerButton>();
  const controllerAxes = new Map<ControllerAxis, number>();
  const controllerButtonValues = new Map<ControllerButton, number>();
  const heldControllerButtons = new Set<ControllerButton>();
  let disposed = false;
  let controllerEpoch = 0;
  let controllerSamplingBlocked = false;

  const pointerLocked = (): boolean => environment.document.pointerLockElement === environment.canvas();
  const gameplayFocused = (): boolean => pointerLocked()
    || environment.document.activeElement === environment.canvas();
  const clearLocal = (): void => {
    controllerEpoch += 1;
    heldKeys.clear();
    heldPointerButtons.clear();
    heldControllerButtons.clear();
    controllerAxes.clear();
    controllerButtonValues.clear();
  };
  const clear = (reason: RuntimeInputWireClearReason): void => {
    clearLocal();
    // A held menu button must not become a new press when ownership changes,
    // including the asynchronous pointer-lock loss caused by opening a menu.
    if ((reason === 'pointer-lock-loss' || reason === 'interaction-mode-loss')
      && environment.interactionMode() === 'interface'
      && environment.active() && environment.document.hasFocus?.() !== false) refreshControllerBaseline();
    queue.clear(reason);
    normalized.onAvailable?.();
  };
  const enqueueFact = (fact: RuntimeInputWireFact): boolean => {
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
  // While the pointer is not locked, pointer facts carry where the cursor
  // is on the Engine canvas, normalized and bottom-left based like a camera
  // viewport, so a product can pick with it (a map, a strategy view).
  let lastPosition: RuntimeInputWirePointerPosition | null = null;
  const cursorPosition = (event: PointerEvent): RuntimeInputWirePointerPosition | null => {
    if (environment.usesPointerLock?.() !== false && pointerLocked()) return null;
    const bounds = environment.canvas().getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) return null;
    const x = (event.clientX - bounds.left) / bounds.width;
    const y = (bounds.bottom - event.clientY) / bounds.height;
    return Number.isFinite(x) && Number.isFinite(y) ? { x, y } : null;
  };
  const withPosition = (event: PointerEvent): { position?: RuntimeInputWirePointerPosition } => {
    const position = cursorPosition(event);
    if (position === null) return {};
    lastPosition = position;
    return { position: Object.freeze(position) };
  };
  const onPointerDown = (event: PointerEvent): void => {
    if (!admit(event, false)) return;
    const button = normalizePointerButton(event.button);
    if (button === null) return;
    // A press that takes pointer lock only takes it: the product sees
    // neither it nor its release, which finds no held button. Decided before
    // focusing, since the desktop shell grants a lock synchronously.
    const acquiring = environment.usesPointerLock?.() !== false && !pointerLocked();
    if (!acquiring && !heldPointerButtons.has(button)) {
      heldPointerButtons.add(button);
      enqueueFact(Object.freeze({ kind: 'pointer-button', button, edge: 'pressed', ...withPosition(event) }));
    }
    environment.focusGameplay();
  };
  const onPointerUp = (event: PointerEvent): void => {
    if (!admit(event, true)) return;
    const button = normalizePointerButton(event.button);
    if (button === null || !heldPointerButtons.delete(button)) return;
    enqueueFact(Object.freeze({ kind: 'pointer-button', button, edge: 'released', ...withPosition(event) }));
  };
  const onPointerCancel = (event: PointerEvent): void => {
    environment.allowsGameplayInput(event);
    clear('interaction-mode-loss');
  };
  const onPointerMove = (event: PointerEvent): void => {
    if (!admit(event, false)) return;
    const position = cursorPosition(event);
    if (position !== null) {
      // Only a change is a fact; a pointer resting over the canvas sends none.
      if (lastPosition?.x === position.x && lastPosition?.y === position.y) return;
      lastPosition = position;
      enqueueFact(Object.freeze({ kind: 'pointer-position', x: position.x, y: position.y }));
      return;
    }
    if (environment.usesPointerLock?.() === false || !pointerLocked()) return;
    // Pointer-lock movement is forwarded whole; sensitivity is product policy.
    const x = Number.isFinite(event.movementX) ? event.movementX : 0;
    const y = Number.isFinite(event.movementY) ? event.movementY : 0;
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
    if (environment.usesPointerLock?.() !== false && !pointerLocked()) clear('pointer-lock-loss');
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
    const mode = environment.interactionMode();
    if (!environment.active() || environment.document.hasFocus?.() === false
      || mode === 'modal' || (mode === 'gameplay' && !gameplayFocused())) {
      // Clearing wakes the input pump, which samples again. Publish the loss
      // once per blocked interval rather than creating a self-sustaining loop.
      if (!controllerSamplingBlocked) {
        controllerSamplingBlocked = true;
        clear('interaction-mode-loss');
      }
      return 0;
    }
    controllerSamplingBlocked = false;
    const controller = environment.gamepads()[normalized.selectedController];
    if (controller === null || controller === undefined || !controller.connected) {
      if (heldControllerButtons.size > 0 || controllerAxes.size > 0 || controllerButtonValues.size > 0) clear('interaction-mode-loss');
      return 0;
    }
    const epoch = controllerEpoch;
    const publish = (fact: RustyApplicationInterfaceInputObservation['fact']): boolean => {
      if (mode === 'interface') {
        environment.observeInterfaceInput?.(Object.freeze({ context: 'interface', fact }));
      } else if (enqueueFact(fact)) return true;
      // UI callbacks may close/open a panel synchronously. Never deliver the
      // rest of that physical sample to the next owner.
      return disposed || controllerEpoch !== epoch || environment.interactionMode() !== mode;
    };
    let observed = 0;
    for (let index = 0; index < 4; index += 1) {
      const axis = controllerAxis(index);
      const value = boundedNumber(controller.axes[index] ?? 0, 1);
      const prior = controllerAxes.get(axis) ?? 0;
      if (value === prior) continue;
      controllerAxes.set(axis, value);
      if (publish(Object.freeze({ kind: 'controller-axis', axis, value }))) return observed;
      observed += 1;
    }
    for (let index = 0; index < 16; index += 1) {
      const button = controllerButton(index);
      const value = Math.max(0, boundedNumber(controller.buttons[index]?.value ?? 0, 1));
      if (value !== (controllerButtonValues.get(button) ?? 0)) {
        controllerButtonValues.set(button, value);
        if (publish(Object.freeze({ kind: 'controller-button-value', button, value }))) return observed;
        observed += 1;
      }
      const pressed = controller.buttons[index]?.pressed === true;
      const wasPressed = heldControllerButtons.has(button);
      if (pressed === wasPressed) continue;
      if (pressed) heldControllerButtons.add(button);
      else heldControllerButtons.delete(button);
      if (publish(Object.freeze({
        kind: 'controller-button', button, edge: pressed ? 'pressed' : 'released',
      }))) return observed;
      observed += 1;
    }
    return observed;
  };

  const rebaselineRuntime = (binding: RustyApplicationRuntimeInputBinding): void => {
    if (disposed || !queue.rebaseRuntime(binding)) return;
    if (environment.interactionMode() !== 'gameplay') {
      clearLocal();
      refreshControllerBaseline();
      return;
    }
    // Runtime control replacement already rebinds its lane with sequence-zero
    // clear. Begin at its published cursor with only state that remains
    // physically held; never mirror or resend the uncertain browser batch.
    refreshControllerBaseline();
    for (const code of [...heldKeys].sort()) {
      enqueueFact(Object.freeze({ kind: 'key', code, edge: 'pressed' }));
    }
    for (const button of [...heldPointerButtons].sort()) {
      enqueueFact(Object.freeze({ kind: 'pointer-button', button, edge: 'pressed' }));
    }
    for (const axis of [...controllerAxes.keys()].sort()) {
      const value = controllerAxes.get(axis)!;
      if (value !== 0) enqueueFact(Object.freeze({ kind: 'controller-axis', axis, value }));
    }
    for (const button of [...heldControllerButtons].sort()) {
      enqueueFact(Object.freeze({ kind: 'controller-button', button, edge: 'pressed' }));
    }
    for (const button of [...controllerButtonValues.keys()].sort()) {
      const value = controllerButtonValues.get(button)!;
      if (value !== 0) enqueueFact(Object.freeze({ kind: 'controller-button-value', button, value }));
    }
    normalized.onAvailable?.();
  };

  function refreshControllerBaseline(): void {
    if (normalized.selectedController === null) return;
    const controller = environment.gamepads()[normalized.selectedController];
    heldControllerButtons.clear();
    controllerAxes.clear();
    controllerButtonValues.clear();
    if (controller === null || controller === undefined || !controller.connected) return;
    for (let index = 0; index < 4; index += 1) {
      const value = boundedNumber(controller.axes[index] ?? 0, 1);
      if (value !== 0) controllerAxes.set(controllerAxis(index), value);
    }
    for (let index = 0; index < 16; index += 1) {
      if (controller.buttons[index]?.pressed === true) heldControllerButtons.add(controllerButton(index));
      const value = Math.max(0, boundedNumber(controller.buttons[index]?.value ?? 0, 1));
      if (value !== 0) controllerButtonValues.set(controllerButton(index), value);
    }
  }

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
    interactionModeChanged: () => {
      if (disposed) return;
      clear('interaction-mode-loss');
      refreshControllerBaseline();
    },
    bindRuntime: (binding: RustyApplicationRuntimeInputBinding) => {
      if (disposed) return;
      if (queue.bindRuntime(binding)) clearLocal();
    },
    synchronizeRuntime: (binding: RustyApplicationRuntimeInputBinding) => {
      if (disposed) return;
      if (queue.bindRuntime(binding)) clearLocal();
    },
    rebaselineRuntime,
    setContext: (context: string) => {
      if (disposed) return;
      if (queue.setContext(context)) clearLocal();
    },
    clear: (reason: RuntimeInputWireClearReason) => {
      if (!disposed) clear(reason);
    },
    drain: () => queue.drain(),
    claim: (intent: string, value: RuntimeInputWireIntentValue) => {
      if (!disposed) {
        if (queue.claim(intent, value)) clearLocal();
        normalized.onAvailable?.();
      }
    },
    sampleController,
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
): KeyboardControl | null {
  const alpha = /^Key([A-Z])$/u.exec(code);
  if (alpha !== null) return `key-${alpha[1]!.toLowerCase()}` as KeyboardControl;
  const digit = /^Digit([0-9])$/u.exec(code);
  if (digit !== null) return `digit-${digit[1]!}` as KeyboardControl;
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
  if (initialSequence < 0n || initialSequence > U64_MAXIMUM) {
    throw new RangeError('initial input sequence must fit u64');
  }
  let binding: RustyApplicationRuntimeInputBinding | null = null;
  let sequence = initialSequence;
  let initialBinding = true;
  let terminal = false;
  let entries: RuntimeInputWireEvent[] = [];
  const nextSequence = (): string | null => {
    if (terminal || sequence >= U64_MAXIMUM) return null;
    const result = sequence.toString(10);
    sequence += 1n;
    return result;
  };
  const terminalClear = (): void => {
    if (binding === null || terminal) return;
    terminal = true;
    const firstDiscarded = entries[0];
    const terminalSequence = firstDiscarded?.sequence
      ?? U64_MAXIMUM.toString(10);
    if (firstDiscarded !== undefined) sequence = BigInt(terminalSequence) + 1n;
    entries = [freezeIngress(
      binding,
      terminalSequence,
      Object.freeze({ kind: 'clear', reason: 'ingress-overflow' }),
    )];
  };
  const replaceQueuedWithClear = (reason: RuntimeInputWireClearReason): void => {
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
  const appendFact = (fact: RuntimeInputWireFact): boolean => {
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
  const appendClaim = (intent: string, value: RuntimeInputWireIntentValue): boolean => {
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
    bindRuntime: (normalized) => {
      const previous = binding;
      if (previous !== null && sameBinding(previous, normalized)) {
        // A binding observation can also carry the Engine's authoritative
        // next-input cursor. Reconcile only when it is present; ordinary
        // same-binding observations must not clear browser held state.
        if (terminal || normalized.nextSequence === undefined) return false;
        synchronizeSameBindingCursor(BigInt(normalized.nextSequence));
        // This is cursor reconciliation, not an epoch/context rebind. The
        // managed ingress uses `true` to clear held DOM state, which would
        // lose a later release for a key the Engine already accepted.
        return false;
      }
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
      const reason: RuntimeInputWireClearReason = previous.runtime.instanceId !== normalized.runtime.instanceId
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
    rebaseRuntime: (normalized) => {
      const previous = binding;
      if (previous !== null && sameBinding(previous, normalized)) return false;
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
          && BigInt(normalized.runtime.controlRevision) <= BigInt(previous.runtime.controlRevision)) {
          throw new RangeError('runtime control revision must advance for a rebaseline');
        }
      }
      binding = normalized;
      sequence = normalized.nextSequence === undefined ? 0n : BigInt(normalized.nextSequence);
      initialBinding = false;
      terminal = false;
      entries = [];
      return true;
    },
    setContext: (context) => {
      if (binding === null || terminal || binding.context === context) return false;
      binding = Object.freeze({ runtime: binding.runtime, context });
      replaceQueuedWithClear('interaction-mode-loss');
      return true;
    },
    clear: (reason) => {
      if (reason === 'interaction-mode-loss') {
        // Same-context physical loss must not discard an accepted DOM action.
        // Keep ordering (including any earlier context transition), and coalesce
        // repeated disallowed pointer events instead of filling the queue.
        const last = entries.at(-1);
        if (last !== undefined && 'fact' in last && last.fact.kind === 'clear'
          && last.fact.reason === reason) return;
        appendFact(Object.freeze({ kind: 'clear', reason }));
      } else {
        replaceQueuedWithClear(reason);
      }
    },
    enqueueFact: appendFact,
    claim: appendClaim,
    drain: () => {
      const drained = entries;
      entries = [];
      return Object.freeze(drained);
    },
  };

  function synchronizeSameBindingCursor(authoritative: bigint): void {
    // A terminal epoch has already published its final clear and is handled
    // by the caller. Keep this guard local as well so future call paths cannot
    // reopen an exhausted queue.
    if (terminal) return;
    entries = entries.filter((entry) => BigInt(entry.sequence) >= authoritative);
    if (sequence < authoritative) sequence = authoritative;
  }
}

function freezeIngress(
  binding: RustyApplicationRuntimeInputBinding,
  sequence: string,
  fact: RuntimeInputWireFact,
): RuntimeInputWirePhysical {
  return Object.freeze({
    runtime: binding.runtime,
    sequence,
    context: binding.context,
    fact: Object.freeze({ ...fact }) as RuntimeInputWireFact,
  });
}

function freezeClaim(
  binding: RustyApplicationRuntimeInputBinding,
  sequence: string,
  intent: string,
  value: RuntimeInputWireIntentValue,
): RuntimeInputWireIntentClaim {
  // The claim is sent later: a copy keeps the product's later edits out of it.
  return Object.freeze({
    runtime: binding.runtime,
    sequence,
    context: binding.context,
    intent,
    value: structuredClone(value),
  });
}

function normalizeOptions(options: RustyApplicationRuntimeInputOptions): NormalizedInputOptions {
  return Object.freeze({
    initialBinding: options.binding ?? null,
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

function normalizePointerButton(button: number): PointerButton | null {
  if (button === 0) return 'primary';
  if (button === 1) return 'middle';
  if (button === 2) return 'secondary';
  return null;
}

function controllerButton(index: number): ControllerButton {
  return `button-${String(index)}` as ControllerButton;
}

function controllerAxis(index: number): ControllerAxis {
  return `axis-${String(index)}` as ControllerAxis;
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

const KEYBOARD_CODE_MAP: ReadonlyMap<string, KeyboardControl> = new Map([
  ['Space', 'space'], ['Enter', 'enter'], ['Escape', 'escape'],
  ['ShiftLeft', 'shift-left'], ['ShiftRight', 'shift-right'],
  ['ControlLeft', 'control-left'], ['ControlRight', 'control-right'],
  ['AltLeft', 'alt-left'], ['AltRight', 'alt-right'],
  ['ArrowUp', 'arrow-up'], ['ArrowDown', 'arrow-down'],
  ['ArrowLeft', 'arrow-left'], ['ArrowRight', 'arrow-right'],
]);
