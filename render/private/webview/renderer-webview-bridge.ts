import type {
  CameraBasis,
  PresentationFrameDiff,
  RenderFrameDiff,
  RendererViewComposition,
} from '@rusty-engine/render-contracts';
import {
  RendererAnimationHost,
  RendererAudioHost,
  RendererBillboardHost,
  RendererDomParticleBillboardSink,
  RendererDomTelemetryOverlaySink,
  RendererLiveTelemetryCollector,
  RendererParticleHost,
  RendererPresentationHostSet,
  RendererTelemetryOverlayHost,
  mountRendererSurface,
  mountRendererSurfaceWithResources,
  type RendererSurface,
  type RendererSurfaceCameraPose,
  type RendererSurfacePickRequest,
} from '@rusty-engine/renderer-host';

const BRIDGE_VERSION = 'rusty_renderer_webview_bridge.v1';

interface WryIpc {
  postMessage(message: string): void;
}

interface RendererResourceEntry {
  readonly identity: string;
  readonly contentHash: string;
  readonly bytesBase64: string;
  readonly mediaType: string;
}

interface RendererWebviewConfiguration {
  readonly autoStart: boolean;
  readonly clearColor: number | null;
  readonly pixelRatio: number;
  readonly resources: readonly RendererResourceEntry[];
}

interface RendererWebviewPrivateApi {
  submitFrame(requestId: number, frame: RenderFrameDiff): void;
  submitPresentation(requestId: number, frame: PresentationFrameDiff): void;
  configureViews(requestId: number, composition: RendererViewComposition): void;
  setCameraPose(requestId: number, pose: RendererSurfaceCameraPose, basis?: CameraBasis): void;
  pick(requestId: number, request: RendererSurfacePickRequest): void;
  readState(requestId: number): void;
  readInput(requestId: number): void;
  renderOnce(requestId: number, timeMs?: number): void;
  resumeAudio(requestId: number): void;
  start(requestId: number): void;
  stop(requestId: number): void;
  resize(requestId: number, width: number, height: number, pixelRatio: number): void;
  dispose(requestId: number): void;
}

interface PhysicalInputState {
  readonly pressedCodes: Set<string>;
  pointerX: number;
  pointerY: number;
  pointerButtons: number;
  wheelDeltaX: number;
  wheelDeltaY: number;
}

type RendererWebviewBridgeState = 'mounting' | 'ready' | 'failed' | 'disposed';

declare global {
  // Both names are Engine-private implementation details injected by the Rust adapter.
  // They are deliberately not exported by any renderer package.
  var __rustyEngineRendererConfiguration: unknown;
  var __rustyEnginePrivateRenderer: RendererWebviewPrivateApi | undefined;
}

export function installRendererWebviewBridge(): void {
  if (globalThis.__rustyEnginePrivateRenderer !== undefined) {
    throw new Error('renderer webview bridge is already installed');
  }

  const input = createInputState();
  let surface: RendererSurface | null = null;
  let audio: RendererAudioHost | null = null;
  let billboard: RendererBillboardHost | null = null;
  let particle: RendererParticleHost | null = null;
  let particleSink: RendererDomParticleBillboardSink | null = null;
  let telemetrySink: RendererDomTelemetryOverlaySink | null = null;
  const objectUrls = new Set<string>();
  let state: RendererWebviewBridgeState = 'mounting';
  let terminalFailure: string | null = null;
  let removeInputListeners: () => void = () => undefined;
  let inputListenersInstalled = false;
  let cleanupStarted = false;

  const post = (message: Readonly<Record<string, unknown>>): void => {
    const encoded = JSON.stringify({ bridgeVersion: BRIDGE_VERSION, ...message });
    readIpc().postMessage(encoded);
  };
  const requireSurface = (): RendererSurface => {
    if (state === 'failed') {
      throw new Error(`renderer webview bridge mount failed: ${terminalFailure ?? 'unknown failure'}`);
    }
    if (state === 'disposed') throw new Error('renderer webview bridge is disposed');
    if (state === 'mounting') throw new Error('renderer webview bridge is still mounting');
    if (surface === null) throw new Error('renderer webview surface is not ready');
    return surface;
  };
  const succeed = (requestId: number, operation: string, value: unknown = null): void => {
    post({ kind: 'operationSucceeded', operation, requestId, value });
  };
  const fail = (requestId: number, operation: string, cause: unknown): void => {
    post({
      kind: 'operationFailed',
      operation,
      requestId,
      message: cause instanceof Error ? cause.message : String(cause),
    });
  };
  const run = (requestId: number, operation: string, action: () => unknown): void => {
    try {
      succeed(requestId, operation, action());
    } catch (cause) {
      fail(requestId, operation, cause);
    }
  };
  const runAsync = (
    requestId: number,
    operation: string,
    action: () => Promise<unknown>,
  ): void => {
    void action().then(
      (value) => succeed(requestId, operation, value),
      (cause: unknown) => fail(requestId, operation, cause),
    );
  };
  const cleanup = async (): Promise<readonly unknown[]> => {
    if (cleanupStarted) return [];
    cleanupStarted = true;
    const failures: unknown[] = [];
    if (inputListenersInstalled) {
      try {
        removeInputListeners();
      } catch (cause) {
        failures.push(cause);
      }
      inputListenersInstalled = false;
    }
    try {
      await audio?.dispose();
    } catch (cause) {
      failures.push(cause);
    }
    const attempt = (ownerCleanup: () => void): void => {
      try {
        ownerCleanup();
      } catch (cause) {
        failures.push(cause);
      }
    };
    attempt(() => billboard?.dispose());
    attempt(() => particle?.dispose());
    attempt(() => particleSink?.dispose());
    attempt(() => telemetrySink?.dispose());
    attempt(() => surface?.dispose());
    audio = null;
    billboard = null;
    particle = null;
    particleSink = null;
    telemetrySink = null;
    surface = null;
    for (const url of objectUrls) URL.revokeObjectURL(url);
    objectUrls.clear();
    return failures;
  };

  const api = Object.freeze<RendererWebviewPrivateApi>({
    submitFrame: (requestId, frame) => run(
      requestId,
      'submitFrame',
      () => requireSurface().applyFrame(frame),
    ),
    submitPresentation: (requestId, frame) => runAsync(
      requestId,
      'submitPresentation',
      () => requireSurface().applyPresentation(frame),
    ),
    configureViews: (requestId, composition) => run(
      requestId,
      'configureViews',
      () => requireSurface().configureViews(composition),
    ),
    setCameraPose: (requestId, pose, basis) => run(requestId, 'setCameraPose', () => {
      requireSurface().setCameraPose(pose, basis);
      return requireSurface().cameraPose();
    }),
    pick: (requestId, request) => run(
      requestId,
      'pick',
      () => requireSurface().pick(request),
    ),
    readState: (requestId) => run(requestId, 'readState', () => surfaceReadout(requireSurface())),
    readInput: (requestId) => run(requestId, 'readInput', () => inputReadout(input)),
    renderOnce: (requestId, timeMs) => run(
      requestId,
      'renderOnce',
      () => requireSurface().renderOnce(timeMs),
    ),
    resumeAudio: (requestId) => runAsync(requestId, 'resumeAudio', async () => {
      requireSurface();
      if (audio === null) throw new Error('audio host is unavailable');
      return audio.resume();
    }),
    start: (requestId) => run(requestId, 'start', () => {
      requireSurface().start();
      return surfaceReadout(requireSurface());
    }),
    stop: (requestId) => run(requestId, 'stop', () => {
      requireSurface().stop();
      return surfaceReadout(requireSurface());
    }),
    resize: (requestId, width, height, pixelRatio) => run(requestId, 'resize', () => {
      assertSurfaceSize(width, height, pixelRatio);
      const current = requireSurface();
      current.canvas.style.width = `${String(width)}px`;
      current.canvas.style.height = `${String(height)}px`;
      current.canvas.width = Math.max(1, Math.round(width * pixelRatio));
      current.canvas.height = Math.max(1, Math.round(height * pixelRatio));
      return current.renderOnce();
    }),
    dispose: (requestId) => runAsync(requestId, 'dispose', async () => {
      requireSurface();
      state = 'disposed';
      const failures = await cleanup();
      if (failures[0] !== undefined) throw failures[0];
      return { disposed: true };
    }),
  });

  Object.defineProperty(globalThis, '__rustyEnginePrivateRenderer', {
    configurable: false,
    enumerable: false,
    value: api,
    writable: false,
  });

  removeInputListeners = installInputListeners(input);
  inputListenersInstalled = true;
  const mount = async (): Promise<void> => {
    const configuration = decodeConfiguration(globalThis.__rustyEngineRendererConfiguration);
    const canvas = requireElement('rusty-renderer-canvas', HTMLCanvasElement);
    const overlays = requireElement('rusty-renderer-overlays', HTMLDivElement);
    const entries = new Map(configuration.resources.map((entry) => [entry.identity, entry]));
    const bytesByHash = new Map(
      configuration.resources.map((entry) => [entry.contentHash, decodeBase64(entry.bytesBase64)]),
    );
    const meshResources = configuration.resources.filter(
      (entry) => entry.identity.startsWith('mesh-resource/'),
    );
    const textureResources = configuration.resources.filter(
      (entry) => entry.identity.startsWith('texture-resource/'),
    );
    const options = {
      autoStart: false,
      ...(configuration.clearColor === null ? {} : { clearColor: configuration.clearColor }),
      pixelRatio: configuration.pixelRatio,
    };
    if (meshResources.length > 0 && textureResources.length > 0) {
      surface = await mountRendererSurfaceWithResources(canvas, {
        ...options,
        meshResourceManifest: meshManifest(meshResources),
        resolveMeshResource: async (descriptor) => resourceBytes(entries, descriptor.resource),
        textureResourceManifest: textureManifest(textureResources),
        resolveTextureResource: async (descriptor) => resourceBytes(entries, descriptor.resource),
      });
    } else if (meshResources.length > 0) {
      surface = await mountRendererSurfaceWithResources(canvas, {
        ...options,
        meshResourceManifest: meshManifest(meshResources),
        resolveMeshResource: async (descriptor) => resourceBytes(entries, descriptor.resource),
      });
    } else if (textureResources.length > 0) {
      surface = await mountRendererSurfaceWithResources(canvas, {
        ...options,
        textureResourceManifest: textureManifest(textureResources),
        resolveTextureResource: async (descriptor) => resourceBytes(entries, descriptor.resource),
      });
    } else {
      surface = mountRendererSurface(canvas, options);
    }

    audio = new RendererAudioHost({
      resolveResource: async (clip) => ({
        bytes: resourceBytesByHash(bytesByHash, clip.contentHash),
        contentHash: clip.contentHash,
      }),
    });
    billboard = new RendererBillboardHost({
      container: overlays,
      projectWorld: (position) => ({ ...requireSurface().projectWorldPoint(position), occluded: false }),
      resolveEntityPosition: () => null,
      resolveResource: async (identity) => {
        const entry = entries.get(identity);
        if (entry === undefined) return null;
        return { bytes: decodeBase64(entry.bytesBase64) };
      },
    });
    particleSink = new RendererDomParticleBillboardSink({
      container: overlays,
      projectWorld: (position) => requireSurface().projectWorldPoint(position),
    });
    particle = new RendererParticleHost({
      resolveEntityPosition: () => null,
      resolveResource: async (sprite) => {
        const bytes = bytesByHash.get(sprite.contentHash);
        if (bytes === undefined) return null;
        const copy = bytes.slice(0);
        const url = URL.createObjectURL(new Blob([copy], { type: 'image/png' }));
        objectUrls.add(url);
        return {
          bytes: copy,
          url,
        };
      },
      sink: particleSink,
    });
    telemetrySink = new RendererDomTelemetryOverlaySink({ container: overlays });
    const telemetry = new RendererTelemetryOverlayHost({
      collector: new RendererLiveTelemetryCollector({ expectedCounters: [] }),
      sink: telemetrySink,
    });
    surface.setPresentationHosts(new RendererPresentationHostSet({
      animation: new RendererAnimationHost(surface.animationProjection),
      audio,
      billboard,
      particle,
      telemetryOverlay: telemetry,
    }));
    if (configuration.autoStart) surface.start();
    state = 'ready';
    post({ kind: 'ready', value: surfaceReadout(surface) });
  };

  const beginMount = (): void => {
    void mount().catch(async (cause: unknown) => {
      terminalFailure = cause instanceof Error ? cause.message : String(cause);
      state = 'failed';
      const cleanupFailures = await cleanup();
      const cleanupMessage = cleanupFailures[0] === undefined
        ? ''
        : `; cleanup also failed: ${cleanupFailures[0] instanceof Error
          ? cleanupFailures[0].message
          : String(cleanupFailures[0])}`;
      post({
        kind: 'mountFailed',
        message: `${terminalFailure}${cleanupMessage}`,
      });
    });
  };
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', beginMount, { once: true });
  } else {
    beginMount();
  }
}

function readIpc(): WryIpc {
  const ipc = (globalThis as unknown as { readonly ipc?: WryIpc }).ipc;
  if (ipc === undefined || typeof ipc.postMessage !== 'function') {
    throw new Error('renderer webview IPC is unavailable');
  }
  return ipc;
}

function decodeConfiguration(value: unknown): RendererWebviewConfiguration {
  if (typeof value !== 'object' || value === null) throw new Error('configuration must be an object');
  const candidate = value as Partial<RendererWebviewConfiguration>;
  if (typeof candidate.autoStart !== 'boolean') throw new Error('configuration.autoStart must be boolean');
  const clearColor = candidate.clearColor;
  const pixelRatio = candidate.pixelRatio;
  if (clearColor === undefined || (clearColor !== null
    && (!Number.isSafeInteger(clearColor) || clearColor < 0 || clearColor > 0xffffff))) {
    throw new Error('configuration.clearColor must be null or an RGB integer');
  }
  if (pixelRatio === undefined || !Number.isFinite(pixelRatio) || pixelRatio <= 0 || pixelRatio > 4) {
    throw new Error('configuration.pixelRatio must be finite and in (0, 4]');
  }
  if (!Array.isArray(candidate.resources) || candidate.resources.length > 1_536) {
    throw new Error('configuration.resources must be a bounded array');
  }
  const identities = new Set<string>();
  const resources = candidate.resources.map((entry, index) => {
    if (typeof entry !== 'object' || entry === null) {
      throw new Error(`configuration.resources[${String(index)}] must be an object`);
    }
    const resource = entry as Partial<RendererResourceEntry>;
    if (typeof resource.identity !== 'string' || resource.identity.length === 0
      || identities.has(resource.identity)) {
      throw new Error(`configuration.resources[${String(index)}].identity is invalid or duplicated`);
    }
    if (typeof resource.contentHash !== 'string' || !/^sha256:[0-9a-f]{64}$/u.test(resource.contentHash)) {
      throw new Error(`configuration.resources[${String(index)}].contentHash is invalid`);
    }
    if (typeof resource.bytesBase64 !== 'string' || resource.bytesBase64.length === 0) {
      throw new Error(`configuration.resources[${String(index)}].bytesBase64 is invalid`);
    }
    if (typeof resource.mediaType !== 'string' || resource.mediaType.length === 0) {
      throw new Error(`configuration.resources[${String(index)}].mediaType is invalid`);
    }
    identities.add(resource.identity);
    return resource as RendererResourceEntry;
  });
  return Object.freeze({
    autoStart: candidate.autoStart,
    clearColor,
    pixelRatio,
    resources: Object.freeze(resources),
  });
}

function meshManifest(resources: readonly RendererResourceEntry[]): {
  readonly kind: 'rusty_renderer_mesh_resources.v1';
  readonly resources: readonly ReturnType<typeof resourceDescriptor>[];
} {
  return { kind: 'rusty_renderer_mesh_resources.v1', resources: resources.map(resourceDescriptor) };
}

function textureManifest(resources: readonly RendererResourceEntry[]): {
  readonly kind: 'rusty_renderer_texture_resources.v1';
  readonly resources: readonly ReturnType<typeof resourceDescriptor>[];
} {
  return {
    kind: 'rusty_renderer_texture_resources.v1',
    resources: resources.map(resourceDescriptor),
  };
}

function resourceDescriptor(entry: RendererResourceEntry): {
  readonly resource: string;
  readonly contentHash: string;
  readonly byteLength: number;
} {
  return {
    resource: entry.identity,
    contentHash: entry.contentHash,
    byteLength: decodeBase64(entry.bytesBase64).byteLength,
  };
}

function resourceBytes(entries: ReadonlyMap<string, RendererResourceEntry>, identity: string): ArrayBuffer {
  const entry = entries.get(identity);
  if (entry === undefined) throw new Error(`resource ${identity} is unavailable`);
  return decodeBase64(entry.bytesBase64);
}

function resourceBytesByHash(entries: ReadonlyMap<string, ArrayBuffer>, hash: string): ArrayBuffer {
  const bytes = entries.get(hash);
  if (bytes === undefined) throw new Error(`resource ${hash} is unavailable`);
  return bytes.slice(0);
}

function decodeBase64(value: string): ArrayBuffer {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes.buffer;
}

function requireElement<T extends Element>(id: string, constructor: new () => T): T {
  const element = document.getElementById(id);
  if (!(element instanceof constructor)) throw new Error(`required element #${id} is unavailable`);
  return element;
}

function surfaceReadout(surface: RendererSurface): Readonly<Record<string, unknown>> {
  return Object.freeze({
    kind: surface.kind,
    backend: surface.backend,
    camera: surface.cameraPose(),
    input: surface.inputReadout(),
    lighting: surface.lightingReadout(),
    movement: surface.movementState(),
    pointerLocked: surface.pointerLocked(),
    submission: surface.submission(),
    timing: surface.timing(),
    views: surface.viewCompositionReadout(),
    visibility: surface.visibilityReadout(),
  });
}

function createInputState(): PhysicalInputState {
  return {
    pressedCodes: new Set(),
    pointerX: 0,
    pointerY: 0,
    pointerButtons: 0,
    wheelDeltaX: 0,
    wheelDeltaY: 0,
  };
}

function installInputListeners(input: PhysicalInputState): () => void {
  const keyDown = (event: KeyboardEvent): void => { input.pressedCodes.add(event.code); };
  const keyUp = (event: KeyboardEvent): void => { input.pressedCodes.delete(event.code); };
  const blur = (): void => { input.pressedCodes.clear(); input.pointerButtons = 0; };
  const pointer = (event: PointerEvent): void => {
    input.pointerX = event.clientX;
    input.pointerY = event.clientY;
    input.pointerButtons = event.buttons;
  };
  const wheel = (event: WheelEvent): void => {
    input.wheelDeltaX += event.deltaX;
    input.wheelDeltaY += event.deltaY;
  };
  globalThis.addEventListener('keydown', keyDown);
  globalThis.addEventListener('keyup', keyUp);
  globalThis.addEventListener('blur', blur);
  globalThis.addEventListener('pointerdown', pointer);
  globalThis.addEventListener('pointermove', pointer);
  globalThis.addEventListener('pointerup', pointer);
  globalThis.addEventListener('wheel', wheel, { passive: true });
  return () => {
    globalThis.removeEventListener('keydown', keyDown);
    globalThis.removeEventListener('keyup', keyUp);
    globalThis.removeEventListener('blur', blur);
    globalThis.removeEventListener('pointerdown', pointer);
    globalThis.removeEventListener('pointermove', pointer);
    globalThis.removeEventListener('pointerup', pointer);
    globalThis.removeEventListener('wheel', wheel);
  };
}

function inputReadout(input: PhysicalInputState): Readonly<Record<string, unknown>> {
  const readout = Object.freeze({
    pressedCodes: Object.freeze([...input.pressedCodes].sort()),
    pointer: Object.freeze({
      xPixels: input.pointerX,
      yPixels: input.pointerY,
      buttons: input.pointerButtons,
    }),
    wheel: Object.freeze({ deltaX: input.wheelDeltaX, deltaY: input.wheelDeltaY }),
  });
  input.wheelDeltaX = 0;
  input.wheelDeltaY = 0;
  return readout;
}

function assertSurfaceSize(width: number, height: number, pixelRatio: number): void {
  if (!Number.isSafeInteger(width) || width <= 0 || width > 16_384
    || !Number.isSafeInteger(height) || height <= 0 || height > 16_384
    || !Number.isFinite(pixelRatio) || pixelRatio <= 0 || pixelRatio > 4) {
    throw new Error('surface size or pixel ratio is invalid');
  }
}
