import {
  mountRustyApplicationFrameView,
  type RustyApplicationFrameView,
} from './frame-view.js';
import type {
  ProductDevCursorMode,
  ProductDevRenderOutput,
  RuntimeInputWireIntentValue,
} from './generated/contracts.js';
import {
  resolvePresentationFrameGeometry,
  validatePresentationAspectBounds,
  type RustyApplicationPresentationAspectBounds,
} from './presentation-frame.js';
import {
  createRustyApplicationInputIngress,
  type RustyApplicationInputPort,
  type RustyApplicationInterfaceInputObservation,
  type RustyApplicationManagedInputIngress,
  type RustyApplicationRuntimeInputOptions,
} from './input-ingress.js';
import {
  createRustyApplicationUiProjection,
  type RustyApplicationUiProjectionOptions,
  type RustyApplicationUiProjectionPort,
  type RustyApplicationUiProjectionReadout,
  type RustyApplicationUiProjectionView,
} from './ui-projection.js';

export const RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION =
  'rusty_application_host.v1';
const RUSTY_APPLICATION_INTERACTIVE_UI_SELECTOR =
  'a,button,input,select,textarea,summary,dialog,[contenteditable="true"],[data-rusty-ui-interactive],[role="dialog"],[aria-modal="true"]';

export type RustyApplicationInteractionMode =
  | 'gameplay'
  | 'interface'
  | 'modal';

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
  readonly claim: (
    intent: string,
    value: RuntimeInputWireIntentValue,
  ) => void;
}

export interface RustyApplicationUiContext {
  readonly ui: RustyApplicationUiPort;
  /** Read-only current Product UI projection and subscription view. */
  readonly projection?: RustyApplicationUiProjectionView;
  /** Claim-only adapter for the shared ordered Engine input lane. */
  readonly intents?: RustyApplicationUiIntentsPort;
  /** Read-only controller observations owned exclusively by interface mode. */
  readonly input?: RustyApplicationUiInputPort;
}

export interface RustyApplicationUiInputPort {
  /** Synchronous observation on the host cadence; returns an unsubscribe function. */
  readonly subscribe: (observer: (input: RustyApplicationInterfaceInputObservation) => void) => () => void;
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
export type RustyApplicationUiMount = (
  root: HTMLElement,
  context: RustyApplicationUiContext,
) => void | RustyApplicationUiOwner | Promise<void | RustyApplicationUiOwner>;

export interface RustyApplicationHostOptions {
  readonly root: HTMLElement;
  readonly mountUi: RustyApplicationUiMount;
  /** Where the runtime draws the world: streamed to this page (the default) or to the desktop window under it. */
  readonly output?: ProductDevRenderOutput;
  /** Observe the one page cadence without creating another animation-frame loop. */
  readonly onCadence?: (timeMs: number) => void;
  /** Optional finite inclusive aspect interval for one shared, clipped presentation frame. */
  readonly presentationAspectBounds?: RustyApplicationPresentationAspectBounds;
  readonly loadingLabel?: string;
  readonly failureLabel?: string;
  readonly initialInteractionMode?: RustyApplicationInteractionMode;
  /** Pointer lock is the existing first-person default; unlocked gameplay keeps the browser cursor. */
  readonly gameplayCursorMode?: ProductDevCursorMode;
  /** Optional browser input ingress. Omission leaves DOM capture disabled. */
  readonly runtimeInput?: RustyApplicationRuntimeInputOptions;
  /** Optional strict Product UI projection channel. */
  readonly uiProjection?: RustyApplicationUiProjectionOptions;
}

export interface RustyApplicationHostReadout {
  readonly compatibilityVersion: typeof RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION;
  readonly interactionMode: RustyApplicationInteractionMode;
  readonly pointerLocked: boolean;
  readonly uiProjection?: RustyApplicationUiProjectionReadout;
  readonly state: 'ready' | 'disposed';
}

export interface RustyApplicationHost {
  readonly kind: 'rusty_application_host.v1';
  readonly ui: RustyApplicationUiPort;
  /** Optional ordered physical-input and direct-UI-claim transport lane. */
  readonly input?: RustyApplicationInputPort;
  /** Trusted host/composition-root ingress for Rust Product UI projections. */
  readonly uiProjection?: RustyApplicationUiProjectionPort;
  readonly readout: () => RustyApplicationHostReadout;
  readonly dispose: () => Promise<void>;
}

export class RustyApplicationHostError extends Error {
  readonly code: 'invalid_presentation_aspect_bounds' | 'invalid_root' | 'mount_failed' | 'disposed';

  constructor(
    code: RustyApplicationHostError['code'],
    message: string,
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = 'RustyApplicationHostError';
    this.code = code;
  }
}

const failureFrameResizeCleanups = new WeakMap<HTMLElement, () => void>();

export async function mountRustyApplication(
  options: RustyApplicationHostOptions,
): Promise<RustyApplicationHost> {
  const { root } = options;
  let presentationAspectBounds: RustyApplicationPresentationAspectBounds | undefined;
  try {
    presentationAspectBounds = validatePresentationAspectBounds(options.presentationAspectBounds);
  } catch (cause) {
    throw new RustyApplicationHostError(
      'invalid_presentation_aspect_bounds',
      cause instanceof Error ? cause.message : String(cause),
      { cause },
    );
  }
  clearPreviousFailure(root);
  if (root.childNodes.length > 0) {
    throw new RustyApplicationHostError(
      'invalid_root',
      'Rusty Application Host requires an empty downstream mount root',
    );
  }

  const projectionBinding = options.uiProjection?.binding
    ?? (options.uiProjection === undefined ? undefined : options.runtimeInput?.binding?.runtime);
  const uiProjection = options.uiProjection === undefined
    ? null
    : createRustyApplicationUiProjection(
      options.uiProjection.binding === undefined && projectionBinding !== undefined
        ? { ...options.uiProjection, binding: projectionBinding }
        : options.uiProjection,
    );
  const projectionView: RustyApplicationUiProjectionView | null = uiProjection === null
    ? null
    : Object.freeze({
      current: uiProjection.current,
      subscribe: uiProjection.subscribe,
    });

  const document = root.ownerDocument;
  const layout = createLayout(
    document,
    options.loadingLabel ?? 'Starting application…',
    presentationAspectBounds,
  );
  root.append(layout.host);
  root.dataset['rustyApplicationState'] = 'mounting';

  const { canvas } = layout;
  let frames: RustyApplicationFrameView | null = null;
  let input: RustyApplicationManagedInputIngress | null = null;
  let uiOwner: RustyApplicationUiOwner | null = null;
  let removeListeners = (): void => undefined;
  let disposed = false;
  let closing = false;
  let disposal: Promise<void> | null = null;
  let interactionMode = options.initialInteractionMode ?? 'interface';
  const gameplayCursorMode = options.gameplayCursorMode ?? 'pointer-lock';
  let intents: RustyApplicationUiIntentsPort | null = null;
  const interfaceInputObservers = new Set<(input: RustyApplicationInterfaceInputObservation) => void>();
  const removePresentationResizeListener = installPresentationFrameSizing(
    root,
    layout.host,
    layout.frame,
    presentationAspectBounds,
  );

  const pointerLocked = (): boolean => document.pointerLockElement === canvas;
  const releaseInput = (): void => {
    if (pointerLocked()) document.exitPointerLock();
  };
  const setInteractionMode = (mode: RustyApplicationInteractionMode): void => {
    if (disposed) {
      throw new RustyApplicationHostError('disposed', 'Rusty Application Host is disposed');
    }
    const changed = interactionMode !== mode;
    interactionMode = mode;
    layout.host.dataset['interactionMode'] = mode;
    if (mode !== 'gameplay') {
      releaseInput();
    }
    if (changed) input?.interactionModeChanged();
  };
  const focusGameplay = (): void => {
    if (interactionMode !== 'gameplay') return;
    if (closing || disposed) {
      throw new RustyApplicationHostError('disposed', 'Rusty Application Host is disposed');
    }
    canvas.focus({ preventScroll: true });
    if (gameplayCursorMode === 'pointer-lock') requestPointerLock(canvas);
  };
  const ui: RustyApplicationUiPort = Object.freeze({
    active: () => !closing && !disposed,
    allowsGameplayInput: (event: Event) =>
      !closing &&
      !disposed &&
      !event.defaultPrevented &&
      interactionMode === 'gameplay' &&
      isEventWithinPresentationFrame(event, layout.frame) &&
      !isInteractiveUiEvent(event, layout.ui),
    focusGameplay,
    interactionMode: () => interactionMode,
    setInteractionMode,
  });

  try {
    frames = mountRustyApplicationFrameView(
      canvas,
      options.output ?? 'stream',
      (timeMs) => options.onCadence?.(timeMs),
    );
    if (options.runtimeInput !== undefined) {
      input = createRustyApplicationInputIngress(options.runtimeInput, {
        active: () => !closing && !disposed,
        allowsGameplayInput: (event) => ui.allowsGameplayInput(event),
        canvas: () => canvas,
        document,
        eventTarget: layout.host,
        focusGameplay,
        gamepads: () => document.defaultView?.navigator.getGamepads?.() ?? [],
        interactionMode: () => interactionMode,
        usesPointerLock: () => gameplayCursorMode === 'pointer-lock',
        observeInterfaceInput: (observation) => {
          for (const observer of [...interfaceInputObservers]) {
            if (closing || disposed || interactionMode !== 'interface') break;
            if (interfaceInputObservers.has(observer)) observer(observation);
          }
        },
      });
      intents = Object.freeze({
        claim: (intent: string, value: RuntimeInputWireIntentValue): void => {
          input?.claim(intent, value);
        },
      });
    }
    removeListeners = installInputArbitration(
      layout.host,
      layout.ui,
      canvas,
      releaseInput,
      () => interactionMode,
      focusGameplay,
      input === null,
      (event) => {
        ui.allowsGameplayInput(event);
        input?.clear('focus-loss');
      },
    );
    setInteractionMode(interactionMode);
    const uiContext: RustyApplicationUiContext = Object.freeze({
      ui,
      ...(projectionView === null ? {} : { projection: projectionView }),
      ...(intents === null ? {} : { intents }),
      ...(input === null ? {} : { input: Object.freeze({
        subscribe: (observer: (input: RustyApplicationInterfaceInputObservation) => void) => {
          if (closing || disposed) return () => undefined;
          interfaceInputObservers.add(observer);
          return () => { interfaceInputObservers.delete(observer); };
        },
      }) }),
    });
    const mounted = await options.mountUi(layout.ui, uiContext);
    uiOwner = mounted ?? null;
    layout.loading.remove();
    layout.host.dataset['state'] = 'ready';
    root.dataset['rustyApplicationState'] = 'ready';
  } catch (cause) {
    disposed = true;
    interfaceInputObservers.clear();
    const cleanupFailures = await cleanupApplicationOwners(
      uiOwner,
      input,
      uiProjection,
      removeListeners,
      frames,
      layout.host,
      removePresentationResizeListener,
    );
    delete root.dataset['rustyApplicationState'];
    const failure = cause instanceof Error ? cause : new Error(String(cause));
    renderFailure(
      root,
      options.failureLabel ?? 'Application failed to start',
      failure.message,
      presentationAspectBounds,
    );
    throw new RustyApplicationHostError(
      'mount_failed',
      cleanupFailures.length === 0
        ? `Rusty Application Host mount failed: ${failure.message}`
        : `Rusty Application Host mount failed: ${failure.message}; cleanup also failed`,
      { cause: failure },
    );
  }

  return Object.freeze({
    kind: 'rusty_application_host.v1' as const,
    ui,
    ...(input === null ? {} : { input: input as RustyApplicationInputPort }),
    ...(uiProjection === null ? {} : { uiProjection }),
    readout: () => Object.freeze({
      compatibilityVersion: RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION,
      interactionMode,
      pointerLocked: pointerLocked(),
      ...(uiProjection === null ? {} : { uiProjection: uiProjection.readout() }),
      state: disposed ? 'disposed' as const : 'ready' as const,
    }),
    dispose: async () => {
      if (disposal !== null) return disposal;
      closing = true;
      disposal = (async () => {
        disposed = true;
        interfaceInputObservers.clear();
        const cleanupFailures = await cleanupApplicationOwners(
          uiOwner,
          input,
          uiProjection,
          removeListeners,
          frames,
          layout.host,
          removePresentationResizeListener,
        );
        uiOwner = null;
        input = null;
        frames = null;
        delete root.dataset['rustyApplicationState'];
        if (cleanupFailures.length > 0) {
          throw new AggregateError(cleanupFailures, 'Rusty Application Host disposal failed');
        }
      })();
      return disposal;
    },
  });
}

function createLayout(
  document: Document,
  loadingLabel: string,
  presentationAspectBounds: RustyApplicationPresentationAspectBounds | undefined,
): {
  readonly host: HTMLDivElement;
  readonly canvas: HTMLCanvasElement;
  readonly ui: HTMLDivElement;
  readonly loading: HTMLDivElement;
  readonly frame: HTMLDivElement | null;
} {
  const host = document.createElement('div');
  host.dataset['rustyApplicationHost'] = RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION;
  host.style.cssText = presentationAspectBounds === undefined
    ? 'isolation:isolate;min-height:100dvh;position:relative;width:100%;'
    : 'height:100%;isolation:isolate;min-height:0;overflow:hidden;position:relative;width:100%;';

  const canvas = document.createElement('canvas');
  canvas.dataset['rustyApplicationRenderer'] = 'engine-owned';
  canvas.setAttribute('aria-label', 'Engine-rendered game world');
  canvas.tabIndex = 0;
  canvas.style.cssText =
    'display:block;height:100%;inset:0;position:absolute;touch-action:none;width:100%;z-index:0;';

  const ui = document.createElement('div');
  ui.dataset['rustyApplicationUi'] = 'downstream';
  ui.style.cssText = presentationAspectBounds === undefined
    ? 'min-height:100dvh;pointer-events:none;position:relative;width:100%;z-index:2;'
    : 'height:100%;min-height:0;overflow:hidden;pointer-events:none;position:relative;width:100%;z-index:2;';

  const uiInteractionStyle = document.createElement('style');
  uiInteractionStyle.dataset['rustyApplicationUiHitTesting'] = 'engine-owned';
  uiInteractionStyle.textContent =
    `[data-rusty-application-ui="downstream"] :is(${RUSTY_APPLICATION_INTERACTIVE_UI_SELECTOR}){pointer-events:auto;}`;
  host.append(uiInteractionStyle);

  const loading = document.createElement('div');
  loading.dataset['rustyApplicationLoading'] = '';
  loading.setAttribute('role', 'status');
  loading.textContent = loadingLabel;
  loading.style.cssText =
    'align-items:center;background:#071012;color:#d9eee7;display:flex;font:14px system-ui;inset:0;justify-content:center;position:absolute;z-index:2;';

  if (presentationAspectBounds === undefined) {
    host.append(canvas, ui, loading);
    return { host, canvas, ui, loading, frame: null };
  }

  const frame = document.createElement('div');
  frame.dataset['rustyApplicationPresentationFrame'] = 'bounded';
  frame.style.cssText =
    'contain:layout paint;flex:none;height:0;overflow:hidden;position:relative;width:0;';
  host.style.display = 'flex';
  host.style.alignItems = 'center';
  host.style.justifyContent = 'center';
  frame.append(canvas, ui, loading);
  host.append(frame);
  return { host, canvas, ui, loading, frame };
}

function installPresentationFrameSizing(
  root: HTMLElement,
  container: HTMLElement,
  frame: HTMLElement | null,
  presentationAspectBounds: RustyApplicationPresentationAspectBounds | undefined,
): () => void {
  if (presentationAspectBounds === undefined || frame === null) return () => undefined;
  let active = true;
  const update = (): void => {
    if (!active) return;
    const geometry = resolvePresentationFrameGeometry(
      container.clientWidth,
      container.clientHeight,
      presentationAspectBounds,
    );
    frame.style.width = `${String(geometry.width)}px`;
    frame.style.height = `${String(geometry.height)}px`;
  };
  const ResizeObserverConstructor = root.ownerDocument.defaultView?.ResizeObserver;
  if (ResizeObserverConstructor !== undefined) {
    const observer = new ResizeObserverConstructor(update);
    observer.observe(root);
    update();
    return () => {
      active = false;
      observer.disconnect();
    };
  }
  const window = root.ownerDocument.defaultView;
  const onResize = (): void => update();
  window?.addEventListener('resize', onResize);
  update();
  return () => {
    active = false;
    window?.removeEventListener('resize', onResize);
  };
}

function installInputArbitration(
  host: HTMLElement,
  uiRoot: HTMLElement,
  canvas: HTMLCanvasElement,
  releaseInput: () => void,
  interactionMode: () => RustyApplicationInteractionMode,
  focusGameplay: () => void,
  coreOwnsPrimaryFocus: boolean,
  clearRuntimeInputForFocus: (event: FocusEvent) => void,
): () => void {
  const document = host.ownerDocument;
  const onPointerDown = (event: PointerEvent): void => {
    if (!isArbitratedHostPointerEvent(event, uiRoot, canvas)) return;
    if (isInteractiveUiEvent(event, uiRoot)) {
      releaseInput();
      return;
    }
    if (interactionMode() === 'gameplay' && (coreOwnsPrimaryFocus || event.button !== 0)) {
      focusGameplay();
    }
  };
  const onFocusIn = (event: FocusEvent): void => {
    if (!isUiTarget(event.target, uiRoot) || !isTextEntry(event.target)) return;
    releaseInput();
    clearRuntimeInputForFocus(event);
  };
  const onPointerLockChange = (): void => {
    host.dataset['pointerLocked'] = String(document.pointerLockElement === canvas);
  };
  const onBlur = (): void => releaseInput();

  host.addEventListener('pointerdown', onPointerDown, true);
  host.addEventListener('focusin', onFocusIn, true);
  document.addEventListener('pointerlockchange', onPointerLockChange);
  document.defaultView?.addEventListener('blur', onBlur);
  onPointerLockChange();
  return () => {
    host.removeEventListener('pointerdown', onPointerDown, true);
    host.removeEventListener('focusin', onFocusIn, true);
    document.removeEventListener('pointerlockchange', onPointerLockChange);
    document.defaultView?.removeEventListener('blur', onBlur);
  };
}

function isInteractiveUiEvent(event: Event, uiRoot: HTMLElement): boolean {
  return event.composedPath().some((target) => isInteractiveUiTarget(target, uiRoot));
}

function isArbitratedHostPointerEvent(
  event: Event,
  uiRoot: HTMLElement,
  canvas: HTMLCanvasElement,
): boolean {
  return event.composedPath().some((target) => target === uiRoot || target === canvas);
}

/**
 * Coordinate-bearing physical events must land inside the bounded shared frame.
 * Keyboard and other non-coordinate events keep their existing host behavior.
 */
function isEventWithinPresentationFrame(event: Event, frame: HTMLElement | null): boolean {
  if (frame === null) return true;
  const point = eventClientPoint(event);
  if (point === null) return true;
  if (point === 'malformed') return false;
  const bounds = frame.getBoundingClientRect();
  return point.x >= bounds.left && point.x < bounds.right
    && point.y >= bounds.top && point.y < bounds.bottom;
}

type EventClientPoint = { readonly x: number; readonly y: number } | 'malformed' | null;

function eventClientPoint(event: Event): EventClientPoint {
  const direct = coordinatePoint(event);
  if (direct !== null) return direct;
  const candidate = event as Event & Record<string, unknown>;
  const touches = firstCoordinatePoint(candidate['touches']);
  if (touches !== null) return touches;
  return firstCoordinatePoint(candidate['changedTouches']);
}

function firstCoordinatePoint(value: unknown): EventClientPoint {
  if (typeof value !== 'object' || value === null) return null;
  const list = value as Record<string, unknown>;
  const indexed = coordinatePoint(list['0']);
  if (indexed !== null) return indexed;
  const item = list['item'];
  if (typeof item !== 'function') return null;
  try {
    return coordinatePoint(item.call(value, 0));
  } catch {
    return null;
  }
}

function coordinatePoint(value: unknown): EventClientPoint {
  if (typeof value !== 'object' || value === null) return null;
  const candidate = value as Record<string, unknown>;
  const hasClientX = 'clientX' in candidate;
  const hasClientY = 'clientY' in candidate;
  if (!hasClientX && !hasClientY) return null;
  const x = candidate['clientX'];
  const y = candidate['clientY'];
  if (typeof x !== 'number' || typeof y !== 'number' || !Number.isFinite(x) || !Number.isFinite(y)) {
    return 'malformed';
  }
  return { x, y };
}

function isInteractiveUiTarget(target: EventTarget | null, uiRoot: HTMLElement): boolean {
  if (!isUiTarget(target, uiRoot)) return false;
  return target.closest(RUSTY_APPLICATION_INTERACTIVE_UI_SELECTOR) !== null;
}

function isUiTarget(target: EventTarget | null, uiRoot: HTMLElement): target is Element {
  return target instanceof Element && uiRoot.contains(target);
}

function isTextEntry(target: EventTarget | null): boolean {
  return target instanceof HTMLInputElement
    || target instanceof HTMLTextAreaElement
    || target instanceof HTMLSelectElement
    || (target instanceof HTMLElement && target.isContentEditable);
}

function requestPointerLock(canvas: HTMLCanvasElement): void {
  try {
    void canvas.requestPointerLock().catch(() => undefined);
  } catch {
    // Pointer lock can be rejected by host policy or a missing user gesture.
  }
}

async function cleanupApplicationOwners(
  uiOwner: RustyApplicationUiOwner | null,
  input: RustyApplicationManagedInputIngress | null,
  uiProjection: RustyApplicationUiProjectionPort | null,
  removeListeners: () => void,
  frames: RustyApplicationFrameView | null,
  host: HTMLElement,
  removePresentationResizeListener: () => void,
): Promise<readonly unknown[]> {
  const failures: unknown[] = [];
  const attempt = async (cleanup: () => void | Promise<void>): Promise<void> => {
    try {
      await cleanup();
    } catch (cause) {
      failures.push(cause);
    }
  };
  await attempt(() => uiOwner?.dispose());
  await attempt(() => input?.dispose());
  await attempt(() => uiProjection?.dispose());
  await attempt(removeListeners);
  await attempt(() => frames?.dispose());
  await attempt(removePresentationResizeListener);
  host.remove();
  return failures;
}

function clearPreviousFailure(root: HTMLElement): void {
  const failureLayout = root.querySelector<HTMLElement>(
    ':scope > [data-rusty-application-failure-layout]',
  );
  if (failureLayout !== null) {
    failureFrameResizeCleanups.get(failureLayout)?.();
    failureFrameResizeCleanups.delete(failureLayout);
    failureLayout.remove();
    return;
  }
  root.querySelector(':scope > [data-rusty-application-failure]')?.remove();
}

function renderFailure(
  root: HTMLElement,
  label: string,
  message: string,
  presentationAspectBounds: RustyApplicationPresentationAspectBounds | undefined,
): void {
  const failure = root.ownerDocument.createElement('section');
  failure.dataset['rustyApplicationFailure'] = '';
  failure.setAttribute('role', 'alert');
  failure.style.cssText = presentationAspectBounds === undefined
    ? 'background:#1b0b0d;color:#ffe8e8;font:14px system-ui;margin:0;min-height:100dvh;padding:2rem;'
    : 'background:#1b0b0d;box-sizing:border-box;color:#ffe8e8;font:14px system-ui;height:100%;margin:0;overflow:auto;padding:2rem;width:100%;';
  const heading = root.ownerDocument.createElement('h1');
  heading.textContent = label;
  const detail = root.ownerDocument.createElement('p');
  detail.textContent = message;
  failure.append(heading, detail);
  if (presentationAspectBounds === undefined) {
    root.append(failure);
    return;
  }
  const failureLayout = root.ownerDocument.createElement('div');
  failureLayout.dataset['rustyApplicationFailureLayout'] = '';
  failureLayout.style.cssText =
    'align-items:center;display:flex;height:100%;isolation:isolate;justify-content:center;min-height:0;overflow:hidden;position:relative;width:100%;';
  const frame = root.ownerDocument.createElement('div');
  frame.dataset['rustyApplicationPresentationFrame'] = 'bounded';
  frame.style.cssText = 'contain:layout paint;flex:none;overflow:hidden;position:relative;';
  frame.append(failure);
  failureLayout.append(frame);
  root.append(failureLayout);
  failureFrameResizeCleanups.set(
    failureLayout,
    installPresentationFrameSizing(root, failureLayout, frame, presentationAspectBounds),
  );
}
