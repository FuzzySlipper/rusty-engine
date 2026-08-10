import type { RenderFrameDiff } from '@rusty-engine/render-contracts';
import {
  createRendererDefaultSurfaceFrame,
  mountRendererSurface,
  type RendererSurface,
  type RendererSurfaceOptions,
  type RendererSurfaceResourceOptions,
} from '@rusty-engine/renderer-host';
import {
  RustyApplicationContentError,
  prepareRustyApplicationContent,
  rustyApplicationSurfaceResourceOptions,
  type PreparedRustyApplicationContent,
  type RustyApplicationContent,
} from './application-content.js';

export const RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION =
  'rusty_application_host.v1';

export type RustyApplicationInteractionMode =
  | 'gameplay'
  | 'interface'
  | 'modal';

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
  /** Atomically replace the immutable resource catalog and complete retained frame. */
  readonly replaceContent: (
    content: RustyApplicationContent,
  ) => Promise<RustyApplicationFrameReceipt>;
  /** Prepare and atomically publish a complete Rust-projected retained frame. */
  readonly replaceFrame: (
    frame: RustyApplicationFrame,
  ) => Promise<RustyApplicationFrameReceipt>;
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
export type RustyApplicationUiMount = (
  root: HTMLElement,
  context: RustyApplicationUiContext,
) => void | RustyApplicationUiOwner | Promise<void | RustyApplicationUiOwner>;

export interface RustyApplicationRendererOptions {
  readonly clearColor?: number;
  readonly initialContent?: RustyApplicationContent;
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

export class RustyApplicationHostError extends Error {
  readonly code: 'invalid_root' | 'mount_failed' | 'disposed';

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

interface RustyApplicationHostEnvironment {
  readonly mountSurface: (
    canvas: HTMLCanvasElement,
    options: RendererSurfaceOptions | RendererSurfaceResourceOptions,
  ) => RendererSurface | Promise<RendererSurface>;
}

const BROWSER_ENVIRONMENT: RustyApplicationHostEnvironment = {
  mountSurface: mountRendererSurface,
};

export async function mountRustyApplication(
  options: RustyApplicationHostOptions,
): Promise<RustyApplicationHost> {
  return mountRustyApplicationWithEnvironment(options, BROWSER_ENVIRONMENT);
}

async function mountRustyApplicationWithEnvironment(
  options: RustyApplicationHostOptions,
  environment: RustyApplicationHostEnvironment,
): Promise<RustyApplicationHost> {
  const { root } = options;
  clearPreviousFailure(root);
  if (root.childNodes.length > 0) {
    throw new RustyApplicationHostError(
      'invalid_root',
      'Rusty Application Host requires an empty downstream mount root',
    );
  }

  const document = root.ownerDocument;
  const layout = createLayout(document, options.loadingLabel ?? 'Starting application…');
  root.append(layout.host);
  root.dataset['rustyApplicationState'] = 'mounting';

  let surface: RendererSurface | null = null;
  let uiOwner: RustyApplicationUiOwner | null = null;
  let removeListeners = (): void => undefined;
  let disposed = false;
  let closing = false;
  let disposal: Promise<void> | null = null;
  let interactionMode = options.initialInteractionMode ?? 'interface';
  let activeCanvas = layout.canvas;
  let activeContent: PreparedRustyApplicationContent | null = null;
  let contentRevision = 0;
  let replacementPending = 0;
  let replacementQueue: Promise<void> = Promise.resolve();

  const requireActive = (): RendererSurface => {
    if (closing || disposed || surface === null) {
      throw new RustyApplicationHostError('disposed', 'Rusty Application Host is disposed');
    }
    return surface;
  };
  const releaseInput = (): void => {
    surface?.releaseInput();
  };
  const setInteractionMode = (mode: RustyApplicationInteractionMode): void => {
    if (disposed) {
      throw new RustyApplicationHostError('disposed', 'Rusty Application Host is disposed');
    }
    interactionMode = mode;
    layout.host.dataset['interactionMode'] = mode;
    if (mode !== 'gameplay') releaseInput();
  };
  const focusGameplay = (): void => {
    if (interactionMode !== 'gameplay') return;
    const activeSurface = requireActive();
    activeSurface.canvas.focus({ preventScroll: true });
    requestPointerLock(activeSurface.canvas);
  };
  const mountSurface = (
    canvas: HTMLCanvasElement,
    content: PreparedRustyApplicationContent,
  ) => {
    return environment.mountSurface(canvas, {
      autoStart: true,
      controls: { enabled: false },
      frame: content.frame as unknown as RenderFrameDiff,
      ...(options.renderer?.clearColor === undefined
        ? {} : { clearColor: options.renderer.clearColor }),
      ...(options.renderer?.pixelRatio === undefined
        ? {} : { pixelRatio: options.renderer.pixelRatio }),
      ...rustyApplicationSurfaceResourceOptions(content),
    });
  };
  const enqueueReplacement = (
    candidate: () => PreparedRustyApplicationContent,
  ): Promise<RustyApplicationFrameReceipt> => {
    requireActive();
    replacementPending += 1;
    let receipt: RustyApplicationFrameReceipt = Object.freeze({
      applied: false,
      diagnostics: [],
    });
    replacementQueue = replacementQueue.then(async () => {
      const oldSurface = surface;
      const oldContent = activeContent;
      if (oldSurface === null || oldContent === null || disposed) {
        receipt = replacementFailure(
          new RustyApplicationHostError('disposed', 'Rusty Application Host is disposed'),
        );
        return;
      }
      const oldCanvas = activeCanvas;
      const candidateCanvas = createRendererCanvas(document);
      layout.host.insertBefore(candidateCanvas, layout.ui);
      let candidateSurface: RendererSurface | null = null;
      let candidateRemoveListeners: (() => void) | null = null;
      try {
        const candidateContent = candidate();
        candidateSurface = await mountSurface(candidateCanvas, candidateContent);
        candidateSurface.setCameraPose(oldSurface.cameraPose());
        candidateRemoveListeners = installInputArbitration(
          layout.host,
          layout.ui,
          () => candidateSurface as RendererSurface,
          () => interactionMode,
          focusGameplay,
        );
        surface = candidateSurface;
        activeContent = candidateContent;
        contentRevision += 1;
        activeCanvas = candidateCanvas;
        const retiredRemoveListeners = removeListeners;
        removeListeners = candidateRemoveListeners;
        candidateRemoveListeners = null;
        try {
          retiredRemoveListeners();
        } catch {
          // The newly published owner remains authoritative even if retirement is noisy.
        }
        try {
          oldSurface.dispose();
        } catch {
          // Disposal is best-effort after the replacement transaction has committed.
        }
        oldCanvas.remove();
        receipt = Object.freeze({ applied: true, diagnostics: [] });
      } catch (cause) {
        try {
          candidateRemoveListeners?.();
        } catch {
          // A failed candidate never replaces the prior listener owner.
        }
        try {
          candidateSurface?.dispose();
        } catch {
          // Preserve the authoritative prior surface even if candidate cleanup is noisy.
        }
        candidateCanvas.remove();
        receipt = replacementFailure(cause);
      }
    });
    return replacementQueue.then(() => receipt).finally(() => {
      replacementPending -= 1;
    });
  };
  const replaceContent = (
    content: RustyApplicationContent,
  ): Promise<RustyApplicationFrameReceipt> => {
    requireActive();
    let prepared: PreparedRustyApplicationContent;
    try {
      prepared = prepareRustyApplicationContent(content);
    } catch (cause) {
      return Promise.resolve(replacementFailure(cause));
    }
    return enqueueReplacement(() => prepared);
  };
  const replaceFrame = (
    frame: RustyApplicationFrame,
  ): Promise<RustyApplicationFrameReceipt> => {
    requireActive();
    let snapshot: RustyApplicationFrame;
    try {
      snapshot = prepareRustyApplicationContent({ frame }).frame;
    } catch (cause) {
      return Promise.resolve(replacementFailure(cause));
    }
    return enqueueReplacement(() => {
      const current = activeContent;
      if (current === null) {
        throw new RustyApplicationHostError('disposed', 'Rusty Application Host is disposed');
      }
      return Object.freeze({
        frame: snapshot,
        resources: current.resources,
        resourceBytes: current.resourceBytes,
      });
    });
  };

  const renderer: RustyApplicationRendererPort = Object.freeze({
    applyFrame: (frame: RustyApplicationFrame) => {
      if (replacementPending > 0) {
        return Object.freeze({
          applied: false,
          diagnostics: Object.freeze([Object.freeze({
            code: 'content_replacement_in_progress',
            message:
              'incremental frames are rejected while complete content replacement is pending',
          })]),
        });
      }
      const receipt = requireActive().applyFrame(frame as unknown as RenderFrameDiff);
      return Object.freeze({
        applied: receipt.applied,
        diagnostics: Object.freeze(receipt.diagnostics.map((diagnostic) => Object.freeze({
          code: diagnostic.code,
          message: diagnostic.message,
        }))),
      });
    },
    clear: async () => {
      const receipt = await replaceContent({
        frame: createRendererDefaultSurfaceFrame() as unknown as RustyApplicationFrame,
        resources: [],
      });
      if (!receipt.applied) {
        throw new Error(
          `Engine default renderer frame was rejected: ${receipt.diagnostics
            .map((diagnostic) => diagnostic.message)
            .join('; ')}`,
        );
      }
    },
    renderOnce: (timeMs?: number) => {
      if (timeMs === undefined) requireActive().renderOnce();
      else requireActive().renderOnce(timeMs);
    },
    replaceContent,
    replaceFrame,
    setCameraPose: (pose: RustyApplicationCameraPose) => requireActive().setCameraPose(pose),
  });
  const ui: RustyApplicationUiPort = Object.freeze({
    active: () => !closing && !disposed,
    allowsGameplayInput: (event: Event) =>
      !closing &&
      !disposed &&
      !event.defaultPrevented &&
      interactionMode === 'gameplay' &&
      !isInteractiveUiEvent(event, layout.ui),
    focusGameplay,
    interactionMode: () => interactionMode,
    setInteractionMode,
  });

  try {
    if (options.renderer?.initialContent !== undefined
      && options.renderer.initialFrame !== undefined) {
      throw new RustyApplicationContentError(
        'content_invalid',
        null,
        'initialContent and initialFrame are mutually exclusive',
      );
    }
    const initialContent = prepareRustyApplicationContent(
      options.renderer?.initialContent ?? {
        frame: options.renderer?.initialFrame
          ?? createRendererDefaultSurfaceFrame() as unknown as RustyApplicationFrame,
        resources: [],
      },
    );
    surface = await mountSurface(layout.canvas, initialContent);
    activeContent = initialContent;
    contentRevision = 1;
    removeListeners = installInputArbitration(
      layout.host,
      layout.ui,
      () => requireActive(),
      () => interactionMode,
      focusGameplay,
    );
    setInteractionMode(interactionMode);
    const mounted = await options.mountUi(layout.ui, { renderer, ui });
    uiOwner = mounted ?? null;
    layout.loading.remove();
    layout.host.dataset['state'] = 'ready';
    root.dataset['rustyApplicationState'] = 'ready';
  } catch (cause) {
    disposed = true;
    const cleanupFailures = await cleanupApplicationOwners(
      uiOwner,
      removeListeners,
      surface,
      layout.host,
    );
    delete root.dataset['rustyApplicationState'];
    const failure = cause instanceof Error ? cause : new Error(String(cause));
    renderFailure(
      root,
      options.failureLabel ?? 'Application failed to start',
      failure.message,
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
    renderer,
    ui,
    readout: () => Object.freeze({
      compatibilityVersion: RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION,
      contentRevision,
      interactionMode,
      pointerLocked: surface?.pointerLocked() ?? false,
      resourceBytes: activeContent?.resourceBytes ?? 0,
      resourceCount: activeContent?.resources.length ?? 0,
      state: disposed ? 'disposed' as const : 'ready' as const,
    }),
    dispose: async () => {
      if (disposal !== null) return disposal;
      closing = true;
      disposal = (async () => {
        await replacementQueue;
        disposed = true;
        const cleanupFailures = await cleanupApplicationOwners(
          uiOwner,
          removeListeners,
          surface,
          layout.host,
        );
        uiOwner = null;
        surface = null;
        delete root.dataset['rustyApplicationState'];
        if (cleanupFailures.length > 0) {
          throw new AggregateError(cleanupFailures, 'Rusty Application Host disposal failed');
        }
      })();
      return disposal;
    },
  });
}

function replacementFailure(cause: unknown): RustyApplicationFrameReceipt {
  return Object.freeze({
    applied: false,
    diagnostics: Object.freeze([Object.freeze({
      code: replacementDiagnosticCode(cause),
      message: cause instanceof Error ? cause.message : String(cause),
    })]),
  });
}

function replacementDiagnosticCode(cause: unknown): string {
  if (cause instanceof RustyApplicationContentError) return cause.code;
  if (typeof cause === 'object' && cause !== null && 'code' in cause
    && typeof cause.code === 'string' && cause.code.includes('resource')) {
    return 'resource_admission_failed';
  }
  if (cause instanceof Error && cause.message.toLowerCase().includes('resource')) {
    return 'resource_admission_failed';
  }
  return 'retained_frame_replacement_failed';
}

function createLayout(document: Document, loadingLabel: string): {
  readonly host: HTMLDivElement;
  readonly canvas: HTMLCanvasElement;
  readonly ui: HTMLDivElement;
  readonly loading: HTMLDivElement;
} {
  const host = document.createElement('div');
  host.dataset['rustyApplicationHost'] = RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION;
  host.style.cssText = 'isolation:isolate;min-height:100dvh;position:relative;width:100%;';

  const canvas = createRendererCanvas(document);

  const ui = document.createElement('div');
  ui.dataset['rustyApplicationUi'] = 'downstream';
  ui.style.cssText = 'min-height:100dvh;position:relative;width:100%;z-index:1;';

  const loading = document.createElement('div');
  loading.dataset['rustyApplicationLoading'] = '';
  loading.setAttribute('role', 'status');
  loading.textContent = loadingLabel;
  loading.style.cssText =
    'align-items:center;background:#071012;color:#d9eee7;display:flex;font:14px system-ui;inset:0;justify-content:center;position:absolute;z-index:2;';

  host.append(canvas, ui, loading);
  return { host, canvas, ui, loading };
}

function createRendererCanvas(document: Document): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.dataset['rustyApplicationRenderer'] = 'engine-owned';
  canvas.setAttribute('aria-label', 'Engine-rendered game world');
  canvas.style.cssText =
    'display:block;height:100%;inset:0;position:absolute;width:100%;z-index:0;';
  return canvas;
}

function installInputArbitration(
  host: HTMLElement,
  uiRoot: HTMLElement,
  surface: () => RendererSurface,
  interactionMode: () => RustyApplicationInteractionMode,
  focusGameplay: () => void,
): () => void {
  const document = host.ownerDocument;
  const onPointerDown = (event: PointerEvent): void => {
    if (isInteractiveUiEvent(event, uiRoot)) {
      surface().releaseInput();
      return;
    }
    if (interactionMode() === 'gameplay') focusGameplay();
  };
  const onFocusIn = (event: FocusEvent): void => {
    if (isTextEntry(event.target)) surface().releaseInput();
  };
  const onPointerLockChange = (): void => {
    host.dataset['pointerLocked'] = String(document.pointerLockElement === surface().canvas);
  };
  const onBlur = (): void => surface().releaseInput();

  uiRoot.addEventListener('pointerdown', onPointerDown, true);
  uiRoot.addEventListener('focusin', onFocusIn, true);
  document.addEventListener('pointerlockchange', onPointerLockChange);
  document.defaultView?.addEventListener('blur', onBlur);
  onPointerLockChange();
  return () => {
    uiRoot.removeEventListener('pointerdown', onPointerDown, true);
    uiRoot.removeEventListener('focusin', onFocusIn, true);
    document.removeEventListener('pointerlockchange', onPointerLockChange);
    document.defaultView?.removeEventListener('blur', onBlur);
  };
}

function isInteractiveUiEvent(event: Event, uiRoot: HTMLElement): boolean {
  return event.composedPath().some((target) => isInteractiveUiTarget(target, uiRoot));
}

function isInteractiveUiTarget(target: EventTarget | null, uiRoot: HTMLElement): boolean {
  if (!(target instanceof Element) || !uiRoot.contains(target)) return false;
  return target.closest(
    'a,button,input,select,textarea,summary,[contenteditable="true"],[data-rusty-ui-interactive],[role="dialog"]',
  ) !== null;
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
  removeListeners: () => void,
  surface: RendererSurface | null,
  host: HTMLElement,
): Promise<readonly unknown[]> {
  const failures: unknown[] = [];
  try {
    await uiOwner?.dispose();
  } catch (cause) {
    failures.push(cause);
  }
  try {
    removeListeners();
  } catch (cause) {
    failures.push(cause);
  }
  try {
    surface?.dispose();
  } catch (cause) {
    failures.push(cause);
  }
  host.remove();
  return failures;
}

function clearPreviousFailure(root: HTMLElement): void {
  const failure = root.querySelector(':scope > [data-rusty-application-failure]');
  failure?.remove();
}

function renderFailure(root: HTMLElement, label: string, message: string): void {
  const failure = root.ownerDocument.createElement('section');
  failure.dataset['rustyApplicationFailure'] = '';
  failure.setAttribute('role', 'alert');
  failure.style.cssText =
    'background:#1b0b0d;color:#ffe8e8;font:14px system-ui;margin:0;min-height:100dvh;padding:2rem;';
  const heading = root.ownerDocument.createElement('h1');
  heading.textContent = label;
  const detail = root.ownerDocument.createElement('p');
  detail.textContent = message;
  failure.append(heading, detail);
  root.append(failure);
}
