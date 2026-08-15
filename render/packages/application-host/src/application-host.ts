import type { PresentationFrameDiff, RenderFrameDiff } from '@rusty-engine/render-contracts';
import {
  RendererAudioHost,
  RendererBillboardHost,
  RendererParticleHost,
  RendererPresentationHostSet,
  createRendererDefaultSurfaceFrame,
  mountRendererSurface,
  type RendererSurface,
  type RendererSurfaceOptions,
  type RendererSurfaceResourceOptions,
} from '@rusty-engine/renderer-host';
import {
  RustyApplicationContentError,
  prepareRustyApplicationContent,
  rustyApplicationAudioResourceResolver,
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
/** A Rust-projected typed presentation diff. Strict decoding remains Engine-owned. */
export type RustyApplicationPresentationFrame = Readonly<Record<string, unknown>>;

export interface RustyApplicationCameraPose {
  readonly position: readonly [number, number, number];
  readonly pitchDegrees: number;
  readonly yawDegrees: number;
}

export type RustyApplicationVoxelSpriteMode =
  | 'sprite'
  | 'relit'
  | 'depth-parallax'
  | 'sprite-splat'
  | 'full-splat';

export interface RustyApplicationVoxelSpriteCaptureSettings {
  readonly resolution: number;
  readonly azimuthDegrees: number;
  readonly elevationDegrees: number;
  readonly near: number;
  readonly far: number;
  /** Defaults to an isolated capture-light rig with readable lighting. */
  readonly lighting?: RustyApplicationVoxelSpriteCaptureLighting;
}

export type RustyApplicationVoxelSpriteCaptureLighting =
  | { readonly mode: 'scene' }
  | {
      readonly mode: 'isolated';
      readonly ambientColor?: readonly [number, number, number];
      readonly ambientIntensity?: number;
      readonly keyDirection?: readonly [number, number, number];
      readonly keyColor?: readonly [number, number, number];
      readonly keyIntensity?: number;
      readonly fillDirection?: readonly [number, number, number];
      readonly fillColor?: readonly [number, number, number];
      readonly fillIntensity?: number;
    };

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
  /** Preserve captured shading or apply the captured normal pass independently of geometry mode. */
  readonly lightingMode: 'captured' | 'normal';
  readonly ambientLight: number;
  readonly diffuseLight: number;
  readonly outputGain: number;
  readonly ambientColor: readonly [number, number, number];
  readonly lightColor: readonly [number, number, number];
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
  readonly depth: { readonly near: number; readonly far: number };
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

export type RustyApplicationVoxelSpriteSource =
  | {
      readonly kind: 'retained';
      readonly handle: number;
      readonly capture: RustyApplicationVoxelSpriteCaptureSettings;
    }
  | {
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
  readonly code:
    | 'disposed'
    | 'duplicate_id'
    | 'invalid_definition'
    | 'missing_source'
    | 'capture_failed'
    | 'unknown_id';
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
  readonly composition:
    | 'opaque-depth-writing-base'
    | 'base-blend-then-depth-writing-splats'
    | 'depth-writing-splats';
  readonly disposed: boolean;
  readonly limitations: readonly [
    'single-capture-view',
    'view-space-normals',
    'rgba8-depth',
    'approximate-splat-orientation',
    'gpu-time-not-measured',
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
  readonly create: (
    definition: RustyApplicationVoxelSpriteDefinition,
  ) => RustyApplicationVoxelSpriteReceipt;
  readonly replace: (
    definition: RustyApplicationVoxelSpriteDefinition,
  ) => RustyApplicationVoxelSpriteReceipt;
  readonly configure: (
    id: string,
    patch: Partial<RustyApplicationVoxelSpriteConfig>,
  ) => RustyApplicationVoxelSpriteReceipt;
  readonly recapture: (
    id: string,
    settings?: RustyApplicationVoxelSpriteCaptureSettings,
  ) => RustyApplicationVoxelSpriteReceipt;
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
  readonly applyPresentation: (
    frame: RustyApplicationPresentationFrame,
  ) => Promise<RustyApplicationPresentationReceipt>;
  /** Replace product content with the Engine-owned empty/default retained frame. */
  readonly clear: () => Promise<void>;
  /** Create an experimental depth-enhanced sprite attachment on the current renderer surface. */
  readonly createVoxelSpriteExperiment: () => RustyApplicationVoxelSpriteExperimentPort;
  readonly renderOnce: (timeMs?: number) => void;
  /** Atomically replace the immutable resource catalog and complete retained frame. */
  readonly replaceContent: (
    content: RustyApplicationContent,
  ) => Promise<RustyApplicationFrameReceipt>;
  /** Prepare and atomically publish a complete Rust-projected retained frame. */
  readonly replaceFrame: (
    frame: RustyApplicationFrame,
  ) => Promise<RustyApplicationFrameReceipt>;
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
export type RustyApplicationUiMount = (
  root: HTMLElement,
  context: RustyApplicationUiContext,
) => void | RustyApplicationUiOwner | Promise<void | RustyApplicationUiOwner>;

export interface RustyApplicationRendererOptions {
  readonly clearColor?: number;
  /** Optional Engine-owned linear fog applied by the mounted renderer surface. */
  readonly fog?: RustyApplicationFogOptions;
  readonly initialContent?: RustyApplicationContent;
  readonly initialFrame?: RustyApplicationFrame;
  readonly pixelRatio?: number;
  /** Gameplay-owned entity positions used only to resolve neutral billboard anchors. */
  readonly resolveIndicatorEntityPosition?: (
    entity: number,
  ) => readonly [number, number, number] | null;
  /** Gameplay-owned entity positions used only to resolve neutral particle anchors. */
  readonly resolveParticleEntityPosition?: (
    entity: number,
  ) => readonly [number, number, number] | null;
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

export class RustyApplicationHostError extends Error {
  readonly code: 'invalid_root' | 'mount_failed' | 'disposed' | 'stale_renderer_port';

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
  let activeAudio: RendererAudioHost | null = null;
  let activeBillboard: RendererBillboardHost | null = null;
  let activeParticle: RendererParticleHost | null = null;
  let activeBillboardUrls = new Set<string>();
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
  const mountSurface = async (
    canvas: HTMLCanvasElement,
    content: PreparedRustyApplicationContent,
  ): Promise<{
    readonly audio: RendererAudioHost | null;
    readonly billboard: RendererBillboardHost;
    readonly particle: RendererParticleHost;
    readonly billboardUrls: Set<string>;
    readonly surface: RendererSurface;
  }> => {
    const mounted = await environment.mountSurface(canvas, {
      autoStart: true,
      controls: { enabled: false },
      frame: content.frame as unknown as RenderFrameDiff,
      ...(options.renderer?.clearColor === undefined
        ? {} : { clearColor: options.renderer.clearColor }),
      ...(options.renderer?.fog === undefined
        ? {} : { fog: options.renderer.fog }),
      ...(options.renderer?.pixelRatio === undefined
        ? {} : { pixelRatio: options.renderer.pixelRatio }),
      ...rustyApplicationSurfaceResourceOptions(content),
    });
    const resolveAudio = rustyApplicationAudioResourceResolver(content);
    const presentationUrls = new Set<string>();
    let particle: RendererParticleHost | null = null;
    try {
      const audio = resolveAudio === null
        ? null
        : new RendererAudioHost({ resolveResource: resolveAudio });
      const resources = new Map(content.resources.map((resource) => [resource.identity, resource]));
      const resourcesByHash = new Map(
        content.resources.map((resource) => [resource.contentHash, resource]),
      );
      const billboard = new RendererBillboardHost({
        container: layout.indicators,
        projectWorld: (position) => ({
          ...mounted.projectWorldPoint(position),
          // The ordinary public host exposes CPU projection but no depth-buffer readback.
          occluded: false,
        }),
        resolveEntityPosition: options.renderer?.resolveIndicatorEntityPosition ?? (() => null),
        resolveResource: async (identity, contentHash) => {
          const resource = resources.get(identity)
            ?? (contentHash === undefined ? undefined : resourcesByHash.get(contentHash));
          if (resource === undefined) return null;
          const bytes = resource.bytes.slice(0);
          if (resource.kind !== 'texture') return { bytes };
          const url = URL.createObjectURL(new Blob([bytes], { type: resource.mediaType }));
          presentationUrls.add(url);
          return { bytes, url };
        },
      });
      particle = new RendererParticleHost({
        resolveEntityPosition: options.renderer?.resolveParticleEntityPosition ?? (() => null),
        resolveResource: async (sprite) => {
          const resource = resourcesByHash.get(sprite.contentHash);
          if (resource?.kind !== 'texture') return null;
          const bytes = resource.bytes.slice(0);
          const url = URL.createObjectURL(new Blob([bytes], { type: resource.mediaType }));
          presentationUrls.add(url);
          return { bytes, url };
        },
        sink: mounted.createParticleSink(),
      });
      mounted.setPresentationHosts(new RendererPresentationHostSet({
        ...(audio === null ? {} : { audio }),
        billboard,
        particle,
      }));
      return {
        audio,
        billboard,
        billboardUrls: presentationUrls,
        particle,
        surface: mounted,
      };
    } catch (cause) {
      particle?.dispose();
      mounted.dispose();
      for (const url of presentationUrls) URL.revokeObjectURL(url);
      throw cause;
    }
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
      const oldAudio = activeAudio;
      const oldBillboard = activeBillboard;
      const oldParticle = activeParticle;
      const oldBillboardUrls = activeBillboardUrls;
      const oldContent = activeContent;
      if (oldSurface === null || oldContent === null || disposed) {
        receipt = replacementFailure(
          new RustyApplicationHostError('disposed', 'Rusty Application Host is disposed'),
        );
        return;
      }
      const oldCanvas = activeCanvas;
      const candidateCanvas = createRendererCanvas(document);
      let candidateSurface: RendererSurface | null = null;
      let candidateAudio: RendererAudioHost | null = null;
      let candidateBillboard: RendererBillboardHost | null = null;
      let candidateParticle: RendererParticleHost | null = null;
      let candidateBillboardUrls = new Set<string>();
      try {
        const candidateContent = candidate();
        const mounted = await mountSurface(candidateCanvas, candidateContent);
        candidateSurface = mounted.surface;
        candidateAudio = mounted.audio;
        candidateBillboard = mounted.billboard;
        candidateParticle = mounted.particle;
        candidateBillboardUrls = mounted.billboardUrls;
        candidateSurface.setCameraPose(oldSurface.cameraPose());
        candidateSurface.renderOnce();
        oldCanvas.replaceWith(candidateCanvas);
        surface = candidateSurface;
        activeAudio = candidateAudio;
        activeBillboard = candidateBillboard;
        activeParticle = candidateParticle;
        activeBillboardUrls = candidateBillboardUrls;
        activeContent = candidateContent;
        contentRevision += 1;
        activeCanvas = candidateCanvas;
        try {
          oldParticle?.dispose();
        } catch {
          // Particle cleanup is best-effort after the replacement transaction commits.
        }
        try {
          oldSurface.dispose();
        } catch {
          // Surface disposal is best-effort after the replacement transaction commits.
        }
        try {
          await oldAudio?.dispose();
        } catch {
          // Audio disposal is best-effort after the replacement commits.
        }
        disposeBillboardOwner(oldBillboard, oldBillboardUrls);
        receipt = Object.freeze({ applied: true, diagnostics: [] });
      } catch (cause) {
        try {
          candidateParticle?.dispose();
        } catch {
          // Preserve the authoritative prior surface if candidate cleanup is noisy.
        }
        try {
          candidateSurface?.dispose();
        } catch {
          // Preserve the authoritative prior surface even if candidate cleanup is noisy.
        }
        try {
          await candidateAudio?.dispose();
        } catch {
          // Preserve the authoritative prior surface if candidate cleanup is noisy.
        }
        disposeBillboardOwner(candidateBillboard, candidateBillboardUrls);
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
    applyPresentation: async (frame: RustyApplicationPresentationFrame) => {
      if (replacementPending > 0) {
        return Object.freeze({
          applied: 0,
          diagnostics: Object.freeze([Object.freeze({
            code: 'content_replacement_in_progress',
            domain: 'application',
            message: 'presentation frames are rejected while complete content replacement is pending',
          })]),
        });
      }
      try {
        const receipt = await requireActive().applyPresentation(
          frame as unknown as PresentationFrameDiff,
        );
        return Object.freeze({
          applied: receipt.applied,
          diagnostics: Object.freeze(receipt.diagnostics.map((diagnostic) => Object.freeze({
            code: diagnostic.code,
            domain: diagnostic.domain,
            message: diagnostic.message,
          }))),
        });
      } catch (cause) {
        return Object.freeze({
          applied: 0,
          diagnostics: Object.freeze([Object.freeze({
            code: 'presentation_frame_rejected',
            domain: 'application',
            message: cause instanceof Error ? cause.message : String(cause),
          })]),
        });
      }
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
    createVoxelSpriteExperiment: () => {
      const owningSurface = requireActive();
      const concrete = owningSurface.createVoxelSpriteExperiment();
      let experimentDisposed = false;
      const requireExperiment = (): typeof concrete => {
        if (experimentDisposed) {
          throw new RustyApplicationHostError(
            'disposed',
            'Rusty Application voxel sprite experiment is disposed',
          );
        }
        if (requireActive() !== owningSurface) {
          throw new RustyApplicationHostError(
            'stale_renderer_port',
            'Rusty Application voxel sprite experiment belongs to a replaced renderer surface',
          );
        }
        return concrete;
      };
      return Object.freeze({
        create: (definition: RustyApplicationVoxelSpriteDefinition) =>
          requireExperiment().create(
            definition as unknown as Parameters<typeof concrete.create>[0],
          ),
        replace: (definition: RustyApplicationVoxelSpriteDefinition) =>
          requireExperiment().replace(
            definition as unknown as Parameters<typeof concrete.replace>[0],
          ),
        configure: (id: string, patch: Partial<RustyApplicationVoxelSpriteConfig>) =>
          requireExperiment().configure(id, patch),
        recapture: (id: string, settings?: RustyApplicationVoxelSpriteCaptureSettings) =>
          requireExperiment().recapture(id, settings),
        destroy: (id: string) => requireExperiment().destroy(id),
        readout: () => requireExperiment().readout(),
        dispose: () => {
          if (experimentDisposed) return;
          experimentDisposed = true;
          if (!closing && !disposed && surface === owningSurface) concrete.dispose();
        },
      });
    },
    renderOnce: (timeMs?: number) => {
      if (timeMs === undefined) requireActive().renderOnce();
      else requireActive().renderOnce(timeMs);
    },
    replaceContent,
    replaceFrame,
    resumeAudio: async () => {
      requireActive();
      if (activeAudio === null) {
        return Object.freeze({
          resumed: false,
          diagnostics: Object.freeze([Object.freeze({
            code: 'audio_host_unavailable',
            message: 'application content has no admitted audio resources',
          })]),
        });
      }
      const diagnostics = await activeAudio.resume();
      return Object.freeze({
        resumed: diagnostics.length === 0,
        diagnostics: Object.freeze(diagnostics.map((diagnostic) => Object.freeze({
          code: diagnostic.code,
          message: diagnostic.message,
        }))),
      });
    },
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
    const surfaceMount = await mountSurface(layout.canvas, initialContent);
    surface = surfaceMount.surface;
    activeAudio = surfaceMount.audio;
    activeBillboard = surfaceMount.billboard;
    activeParticle = surfaceMount.particle;
    activeBillboardUrls = surfaceMount.billboardUrls;
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
      activeAudio,
      activeBillboard,
      activeParticle,
      activeBillboardUrls,
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
          activeAudio,
          activeBillboard,
          activeParticle,
          activeBillboardUrls,
          layout.host,
        );
        uiOwner = null;
        surface = null;
        activeAudio = null;
        activeBillboard = null;
        activeParticle = null;
        activeBillboardUrls = new Set();
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
  readonly indicators: HTMLDivElement;
  readonly loading: HTMLDivElement;
} {
  const host = document.createElement('div');
  host.dataset['rustyApplicationHost'] = RUSTY_APPLICATION_HOST_COMPATIBILITY_VERSION;
  host.style.cssText = 'isolation:isolate;min-height:100dvh;position:relative;width:100%;';

  const canvas = createRendererCanvas(document);

  const indicators = document.createElement('div');
  indicators.dataset['rustyApplicationIndicators'] = 'engine-owned';
  indicators.style.cssText =
    'inset:0;overflow:hidden;pointer-events:none;position:absolute;z-index:1;';

  const ui = document.createElement('div');
  ui.dataset['rustyApplicationUi'] = 'downstream';
  ui.style.cssText = 'min-height:100dvh;position:relative;width:100%;z-index:2;';

  const loading = document.createElement('div');
  loading.dataset['rustyApplicationLoading'] = '';
  loading.setAttribute('role', 'status');
  loading.textContent = loadingLabel;
  loading.style.cssText =
    'align-items:center;background:#071012;color:#d9eee7;display:flex;font:14px system-ui;inset:0;justify-content:center;position:absolute;z-index:2;';

  host.append(canvas, indicators, ui, loading);
  return { host, canvas, indicators, ui, loading };
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
  audio: RendererAudioHost | null,
  billboard: RendererBillboardHost | null,
  particle: RendererParticleHost | null,
  billboardUrls: ReadonlySet<string>,
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
    particle?.dispose();
  } catch (cause) {
    failures.push(cause);
  }
  try {
    surface?.dispose();
  } catch (cause) {
    failures.push(cause);
  }
  try {
    await audio?.dispose();
  } catch (cause) {
    failures.push(cause);
  }
  try {
    disposeBillboardOwner(billboard, billboardUrls);
  } catch (cause) {
    failures.push(cause);
  }
  host.remove();
  return failures;
}

function disposeBillboardOwner(
  billboard: RendererBillboardHost | null,
  urls: ReadonlySet<string>,
): void {
  billboard?.dispose();
  for (const url of urls) URL.revokeObjectURL(url);
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
