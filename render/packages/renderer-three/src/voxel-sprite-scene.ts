import * as THREE from 'three';
import * as SkeletonUtils from 'three/examples/jsm/utils/SkeletonUtils.js';
import type { RenderHandle, TextureDescriptor } from '@rusty-engine/render-contracts';

import type { ThreeRenderer } from './three-renderer.js';
import {
  VOXEL_SPRITE_CAPTURE_MAX_RESOLUTION,
  VOXEL_SPRITE_CAPTURE_MIN_RESOLUTION,
  VoxelSpriteFrame,
} from './voxel-sprite-capture.js';
import type { AnimatedMeshCaptureAppearance } from './animated-mesh.js';
import {
  VoxelSpriteEnhancement,
  type VoxelSpriteEnhancementConfig,
  type VoxelSpriteEnhancementMode,
  type VoxelSpriteEnhancementReadout,
} from './voxel-sprite-enhancement.js';
import { VoxelSpriteRuntimeCapture } from './voxel-sprite-capture.js';
import {
  GhostPlateDirectionalPresentation,
  GhostPlatePresentation,
  type GhostPlateConfig,
  type GhostPlateReadout,
  type GhostPlateSectorCount,
  type GhostPlateTransitionMode,
} from './voxel-sprite-ghost-plate.js';

export type RendererThreeVoxelSpriteMode = VoxelSpriteEnhancementMode | 'ghost-plate';
export type RendererThreeVoxelSpriteGhostAnchorPolicy = 'bounds-center' | 'bounds-normalized';
export type RendererThreeVoxelSpriteGhostPlateMapping = 'plate-locked' | 'projective-surface';
export type RendererThreeVoxelSpriteGhostShellMode = 'whole-mesh' | 'strict-source' | 'repaired-source';

export interface RendererThreeVoxelSpriteGhostConfig {
  /** Fraction of source-view depth retained by the crushed display mesh. */
  readonly ghostDepthRetention: number;
  /** Selects the fixed source-view depth around which relief is compressed. */
  readonly ghostAnchorPolicy: RendererThreeVoxelSpriteGhostAnchorPolicy;
  /** Front-to-back interpolation used only by `bounds-normalized`. */
  readonly ghostAnchorValue: number;
  /** Chooses screen-locked plate coordinates or source-projective coordinates. */
  readonly ghostPlateMapping: RendererThreeVoxelSpriteGhostPlateMapping;
  /** Optional source-depth visibility policy; whole-mesh preserves the accepted task-7087 control. */
  readonly ghostShellMode: RendererThreeVoxelSpriteGhostShellMode;
  /** Capture-view depth tolerance in the same units as capture near/far. */
  readonly ghostShellDepthEpsilon: number;
  /** Number of actor-relative azimuth depictions captured from one frozen pose. */
  readonly ghostSectorCount: GhostPlateSectorCount;
  /** Extra angular hold beyond each sector's half-width. */
  readonly ghostSectorHysteresisDegrees: number;
  readonly ghostTransitionMode: GhostPlateTransitionMode;
  readonly ghostTransitionDurationMilliseconds: number;
}

export type RendererThreeVoxelSpriteConfigPatch = Partial<
  VoxelSpriteEnhancementConfig & RendererThreeVoxelSpriteGhostConfig
>;

export interface RendererThreeVoxelSpriteTransform {
  readonly position: readonly [number, number, number];
  readonly width: number;
  readonly height: number;
}

export interface RendererThreeVoxelSpriteCaptureSettings {
  readonly resolution: number;
  readonly azimuthDegrees: number;
  readonly elevationDegrees: number;
  readonly near: number;
  readonly far: number;
  /** Perspective source-view field of view; defaults to the legacy 35 degrees. */
  readonly fieldOfViewDegrees?: number;
  /** Defaults to an isolated, readable capture-light rig. */
  readonly lighting?: RendererThreeVoxelSpriteCaptureLighting;
}

export type RendererThreeVoxelSpriteCaptureLighting =
  | { readonly mode: 'scene' }
  | {
      readonly mode: 'isolated';
      readonly ambientColor?: readonly [number, number, number];
      readonly ambientIntensity?: number;
      /** View-relative direction from the subject toward the key light. */
      readonly keyDirection?: readonly [number, number, number];
      readonly keyColor?: readonly [number, number, number];
      readonly keyIntensity?: number;
      /** View-relative direction from the subject toward the fill light. */
      readonly fillDirection?: readonly [number, number, number];
      readonly fillColor?: readonly [number, number, number];
      readonly fillIntensity?: number;
    };

export interface RendererThreeVoxelSpritePreparedFrame {
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

export type RendererThreeVoxelSpriteSource =
  | {
      readonly kind: 'retained';
      readonly handle: RenderHandle;
      readonly capture: RendererThreeVoxelSpriteCaptureSettings;
    }
  | {
      readonly kind: 'prepared';
      readonly frame: RendererThreeVoxelSpritePreparedFrame;
    };

export interface RendererThreeVoxelSpriteDefinition {
  readonly id: string;
  readonly source: RendererThreeVoxelSpriteSource;
  readonly transform: RendererThreeVoxelSpriteTransform;
  readonly mode: RendererThreeVoxelSpriteMode;
  readonly config?: Omit<RendererThreeVoxelSpriteConfigPatch, 'mode' | 'width' | 'height'>;
}

/** Caller-owned timing policy; the renderer expands it once into exact normalized samples. */
export type RendererThreeHeldAnimationSamplePlan =
  | { readonly kind: 'exact'; readonly normalizedTimes: readonly number[] }
  | { readonly kind: 'cadence'; readonly samplesPerSecond: 8 | 12 | 24; readonly count: number };

/** A backend-local, manually advanced held-pose frame-bank experiment. */
export interface RendererThreeHeldAnimationFrameBankDefinition {
  readonly id: string;
  readonly animatedMesh: RenderHandle;
  readonly clip: string;
  readonly samples: RendererThreeHeldAnimationSamplePlan;
  /** Actor-relative capture directions, built from equal azimuth sectors. */
  readonly sectorCount: 1 | 4 | 8 | 16;
  readonly capture: RendererThreeVoxelSpriteCaptureSettings;
  readonly transform: RendererThreeVoxelSpriteTransform;
  readonly mode: VoxelSpriteEnhancementMode;
  readonly config?: Partial<Omit<VoxelSpriteEnhancementConfig, 'mode' | 'width' | 'height'>>;
}

export interface RendererThreeHeldAnimationFrameBankReadout {
  readonly id: string;
  readonly state: 'preparing' | 'ready';
  readonly key: string;
  readonly generation: number;
  readonly source: {
    readonly asset: string;
    readonly assetGeneration: number;
    readonly handle: number;
    readonly contentHash: string | null;
    readonly clip: string;
    readonly origin: 'embedded' | 'pack';
    readonly pack: { readonly asset: string; readonly contentHash: string | null } | null;
    readonly instanceTransform: {
      readonly position: readonly [number, number, number];
      readonly quaternion: readonly [number, number, number, number];
      readonly scale: readonly [number, number, number];
    };
  };
  readonly frameCount: number;
  readonly directionCount: number;
  readonly capturedFrameCount: number;
  readonly selectedSampleIndex: number | null;
  readonly selectedDirectionIndex: number | null;
  readonly captureCount: number;
  readonly cacheHitCount: number;
  readonly switchCount: number;
  readonly preparationCpuMilliseconds: number | null;
  readonly captureCpuMilliseconds: number | null;
  readonly lastSwitchCpuMilliseconds: number | null;
  readonly estimatedResidentBytes: number;
  readonly estimatedPeakBytes: number;
  readonly gpuTiming: 'not-measured';
  readonly cancelledCount: number;
  readonly replacementFailureCount: number;
}

export interface RendererThreeVoxelSpriteDiagnostic {
  readonly code: 'disposed' | 'duplicate_id' | 'invalid_definition' | 'missing_source' | 'capture_failed' | 'unknown_id'
    | 'frame_bank_busy' | 'frame_bank_cancelled' | 'frame_bank_failed' | 'unknown_frame_bank';
  readonly message: string;
}

export interface RendererThreeVoxelSpriteReceipt {
  readonly applied: boolean;
  readonly diagnostics: readonly RendererThreeVoxelSpriteDiagnostic[];
  readonly readout: RendererThreeVoxelSpriteSceneReadout;
}

export interface RendererThreeVoxelSpriteEntryReadout {
  readonly id: string;
  readonly source: 'retained' | 'prepared';
  readonly sourceHandle: number | null;
  readonly capture: RendererThreeVoxelSpriteCaptureSettings | null;
  readonly fallbackPreservedCount: number;
  readonly presentation: 'enhancement' | 'ghost-plate';
  readonly enhancement: VoxelSpriteEnhancementReadout | null;
  readonly ghostPlate: GhostPlateReadout | null;
}

export interface RendererThreeVoxelSpriteSceneReadout {
  readonly schemaVersion: 1;
  readonly revision: number;
  readonly entries: readonly RendererThreeVoxelSpriteEntryReadout[];
  readonly frameBanks: readonly RendererThreeHeldAnimationFrameBankReadout[];
  /** Candidates are separate so a replacement never ambiguously duplicates a ready bank ID. */
  readonly frameBankCandidates: readonly RendererThreeHeldAnimationFrameBankReadout[];
  readonly frameBankMemory: {
    readonly readyResidentBytes: number;
    readonly candidateResidentBytes: number;
    readonly candidateReservedBytes: number;
    readonly peakBytes: number;
  };
  /** Outcomes remain observable after their candidate was released. */
  readonly frameBankOutcomes: readonly {
    readonly id: string;
    readonly cancelledCount: number;
    readonly replacementFailureCount: number;
  }[];
  readonly disposed: boolean;
}

export interface RendererThreeVoxelSpriteBackend {
  readonly scene: THREE.Scene;
  objectFor(handle: RenderHandle): THREE.Object3D | undefined;
  textureDescriptor(id: string): TextureDescriptor | undefined;
  textureObjectFor(id: string): THREE.Texture | undefined;
  createAnimatedMeshCaptureAppearance?(
    handle: RenderHandle,
    clipId: string,
    normalizedTime: number,
  ): AnimatedMeshCaptureAppearance;
}

interface Entry {
  readonly id: string;
  readonly enhancement: VoxelSpriteEnhancement | null;
  readonly ghostPlate: GhostPlateDirectionalPresentation | null;
  readonly frame: VoxelSpriteFrame;
  readonly source: RendererThreeVoxelSpriteSource;
  readonly transform: RendererThreeVoxelSpriteTransform;
  readonly runtimeCapture: VoxelSpriteRuntimeCapture | null;
  readonly ghostRuntimeCaptures: readonly VoxelSpriteRuntimeCapture[];
  readonly retainedObject: THREE.Object3D | null;
  readonly retainedOriginalVisibility: boolean | null;
  releaseCanonicalSuppression: (() => void) | null;
  captureSettings: RendererThreeVoxelSpriteCaptureSettings | null;
  fallbackPreservedCount: number;
}

interface HeldCaptureFrame {
  readonly capture: VoxelSpriteRuntimeCapture;
  readonly frame: VoxelSpriteFrame;
  readonly sampleIndex: number;
  readonly directionIndex: number;
  readonly captureCpuMilliseconds: number | null;
}

interface HeldFrameBankCandidate {
  readonly definition: RendererThreeHeldAnimationFrameBankDefinition;
  readonly key: string;
  readonly source: AnimatedMeshCaptureAppearance['source'];
  readonly normalizedTimes: readonly number[];
  readonly estimatedBytes: number;
  /** Scene-wide peak reserved when this candidate was admitted. */
  readonly admissionPeakBytes: number;
  readonly startedMilliseconds: number;
  readonly frames: HeldCaptureFrame[];
  nextIndex: number;
  cancelledCount: number;
  replacementFailureCount: number;
}

interface HeldFrameBank {
  readonly definition: RendererThreeHeldAnimationFrameBankDefinition;
  readonly key: string;
  readonly source: AnimatedMeshCaptureAppearance['source'];
  readonly normalizedTimes: readonly number[];
  readonly frames: readonly HeldCaptureFrame[];
  readonly enhancement: VoxelSpriteEnhancement;
  readonly estimatedBytes: number;
  /** Immutable record of the scene-wide peak used to admit this published bank. */
  readonly admissionPeakBytes: number;
  readonly preparationCpuMilliseconds: number;
  captureCpuMilliseconds: number | null;
  selectedSampleIndex: number;
  selectedDirectionIndex: number;
  captureCount: number;
  cacheHitCount: number;
  switchCount: number;
  lastSwitchCpuMilliseconds: number | null;
  cancelledCount: number;
  replacementFailureCount: number;
}

const HELD_ANIMATION_BANK_MAX_SAMPLES = 24;
const HELD_ANIMATION_BANK_MAX_DIRECTIONS = 16;
const HELD_ANIMATION_BANK_MAX_RESOLUTION = 512;
const HELD_ANIMATION_BANK_MAX_FRAMES = 96;
const HELD_ANIMATION_BANK_MAX_PIXELS = 8_388_608;
const HELD_ANIMATION_BANK_MAX_RESIDENT_BYTES = 128 * 1024 * 1024;
const HELD_ANIMATION_BANK_MAX_PEAK_BYTES = 192 * 1024 * 1024;
const HELD_ANIMATION_BANK_MAX_STEP_CAPTURES = 8;
const HELD_ANIMATION_BANK_MAX_RECORDS = 128;
const HELD_ANIMATION_BANK_PERSISTENT_BYTES_PER_PIXEL = 24;
const HELD_ANIMATION_BANK_TRANSIENT_BYTES_PER_PIXEL = 4;

/** Three-local scene attachment behind renderer-host and application-host ports. */
export class RendererThreeVoxelSpriteScene {
  readonly #webgl: THREE.WebGLRenderer;
  readonly #backend: RendererThreeVoxelSpriteBackend;
  readonly #invalidate: () => void;
  readonly #onDispose: (() => void) | null;
  readonly #entries = new Map<string, Entry>();
  readonly #heldFrameBanks = new Map<string, HeldFrameBank>();
  readonly #heldFrameBankCandidates = new Map<string, HeldFrameBankCandidate>();
  readonly #heldFrameBankOutcomes = new Map<string, { cancelledCount: number; replacementFailureCount: number }>();
  readonly #canonicalSuppressions = new Map<THREE.Object3D, {
    count: number;
    readonly layerZeroEnabled: ReadonlyMap<THREE.Object3D, boolean>;
  }>();
  #disposed = false;
  #revision = 0;

  constructor(options: {
    readonly webgl: THREE.WebGLRenderer;
    readonly backend: ThreeRenderer | RendererThreeVoxelSpriteBackend;
    readonly invalidate?: () => void;
    readonly onDispose?: () => void;
  }) {
    this.#webgl = options.webgl;
    this.#backend = options.backend;
    this.#invalidate = options.invalidate ?? (() => undefined);
    this.#onDispose = options.onDispose ?? null;
  }

  create(definition: RendererThreeVoxelSpriteDefinition): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    if (this.#entries.has(definition.id)) return this.#rejected('duplicate_id', `duplicate voxel sprite id ${definition.id}`);
    let entry: Entry;
    try {
      entry = this.#buildEntry(validatedDefinition(definition));
    } catch (cause) {
      return this.#rejected(classifyCause(cause), messageFrom(cause));
    }
    this.#entries.set(entry.id, entry);
    this.#backend.scene.add(presentationObject(entry));
    if (entry.ghostPlate !== null && entry.retainedObject !== null) {
      entry.releaseCanonicalSuppression = this.#acquireCanonicalSuppression(entry.retainedObject);
    }
    if (entry.enhancement !== null && entry.retainedObject !== null) entry.retainedObject.visible = false;
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  /** Build first and publish second so a failed source/config replacement preserves the live entry. */
  replace(definition: RendererThreeVoxelSpriteDefinition): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const previous = this.#entries.get(definition.id);
    if (previous === undefined) {
      return this.#rejected('unknown_id', `unknown voxel sprite id ${definition.id}`);
    }
    let candidate: Entry;
    try {
      candidate = this.#buildEntry(validatedDefinition(definition));
    } catch (cause) {
      return this.#rejected(classifyCause(cause), messageFrom(cause));
    }

    this.#disposeEntry(previous);
    this.#entries.set(candidate.id, candidate);
    this.#backend.scene.add(presentationObject(candidate));
    if (candidate.ghostPlate !== null && candidate.retainedObject !== null) {
      candidate.releaseCanonicalSuppression = this.#acquireCanonicalSuppression(candidate.retainedObject);
    }
    if (candidate.enhancement !== null && candidate.retainedObject !== null) candidate.retainedObject.visible = false;
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  configure(
    id: string,
    patch: RendererThreeVoxelSpriteConfigPatch,
  ): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const entry = this.#entries.get(id);
    if (entry === undefined) return this.#rejected('unknown_id', `unknown voxel sprite id ${id}`);
    try {
      if (entry.ghostPlate !== null) entry.ghostPlate.configure(ghostConfigPatch(patch));
      else entry.enhancement!.configure(enhancementConfigPatch(patch));
    } catch (cause) {
      return this.#rejected('invalid_definition', messageFrom(cause));
    }
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  recapture(
    id: string,
    settings?: RendererThreeVoxelSpriteCaptureSettings,
  ): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const entry = this.#entries.get(id);
    if (entry === undefined) return this.#rejected('unknown_id', `unknown voxel sprite id ${id}`);
    if (entry.source.kind !== 'retained'
      || entry.runtimeCapture === null
      || entry.retainedObject === null) {
      return this.#rejected('invalid_definition', `voxel sprite ${id} is not a retained capture source`);
    }
    let captureSettings: RendererThreeVoxelSpriteCaptureSettings;
    try {
      captureSettings = validatedCapture(settings ?? entry.captureSettings!);
    } catch (cause) {
      return this.#rejected('invalid_definition', messageFrom(cause));
    }
    if (entry.ghostPlate !== null) {
      let candidate: Entry;
      try {
        const ghost = entry.ghostPlate.readout();
        candidate = this.#buildEntry({
          id,
          source: { ...entry.source, capture: captureSettings },
          transform: entry.transform,
          mode: 'ghost-plate',
          config: {
            ghostDepthRetention: ghost.depthRetention,
            ghostAnchorPolicy: ghost.anchorPolicy,
            ghostAnchorValue: ghost.anchorValue,
            ghostPlateMapping: ghost.plateMapping,
            ghostShellMode: ghost.shellMode,
            ghostShellDepthEpsilon: ghost.shellDepthEpsilon,
            ghostSectorCount: ghost.sectorCount,
            ghostSectorHysteresisDegrees: ghost.sectorHysteresisDegrees,
            ghostTransitionMode: ghost.transitionMode,
            ghostTransitionDurationMilliseconds: ghost.transitionDurationMilliseconds,
          },
        });
      } catch (cause) {
        entry.fallbackPreservedCount += 1;
        return this.#rejected(classifyCause(cause), messageFrom(cause));
      }
      this.#disposeEntry(entry);
      this.#entries.set(id, candidate);
      this.#backend.scene.add(presentationObject(candidate));
      candidate.releaseCanonicalSuppression = this.#acquireCanonicalSuppression(candidate.retainedObject!);
      this.#revision += 1;
      this.#invalidate();
      return this.#applied();
    }

    const receipt = this.#captureRetained(entry.runtimeCapture, entry.retainedObject, captureSettings);
    if (!receipt.applied || receipt.frame === null) {
      entry.fallbackPreservedCount += 1;
      return this.#rejected('capture_failed', receipt.diagnostics[0]?.message ?? 'runtime recapture failed');
    }
    entry.enhancement!.replaceSource({
      frame: receipt.frame,
      captureCpuSubmissionMilliseconds: receipt.readout.cpuSubmissionMilliseconds,
    });
    entry.captureSettings = captureSettings;
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  /**
   * Validate and reserve a manually advanced candidate. This allocates no GPU
   * capture targets; callers explicitly drive bounded capture steps below.
   */
  beginHeldAnimationFrameBank(
    definition: RendererThreeHeldAnimationFrameBankDefinition,
  ): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    if (this.#heldFrameBankCandidates.size > 0 && !this.#heldFrameBankCandidates.has(definition.id)) {
      return this.#rejected('frame_bank_busy', 'only one held-animation frame-bank preparation may run at a time');
    }
    let candidate: HeldFrameBankCandidate;
    try {
      candidate = this.#heldCandidate(validatedHeldFrameBankDefinition(definition));
    } catch (cause) {
      return this.#rejected(classifyCause(cause), messageFrom(cause));
    }
    const prior = this.#heldFrameBankCandidates.get(candidate.definition.id);
    if (prior !== undefined) this.#disposeHeldCandidate(prior);
    this.#heldFrameBankCandidates.set(candidate.definition.id, candidate);
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  /** Capture at most a caller-selected bounded number of unique pose×direction frames. */
  prepareHeldAnimationFrameBank(
    id: string,
    maximumCaptures = 1,
  ): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const candidate = this.#heldFrameBankCandidates.get(id);
    if (candidate === undefined) return this.#rejected('unknown_frame_bank', `unknown preparing frame bank ${id}`);
    if (!Number.isInteger(maximumCaptures) || maximumCaptures < 1 || maximumCaptures > HELD_ANIMATION_BANK_MAX_STEP_CAPTURES) {
      return this.#rejected('invalid_definition', `frame-bank step captures must be an integer from 1 to ${String(HELD_ANIMATION_BANK_MAX_STEP_CAPTURES)}`);
    }
    try {
      const finalCapture = Math.min(candidate.frames.length + maximumCaptures, candidate.normalizedTimes.length * candidate.definition.sectorCount);
      while (candidate.frames.length < finalCapture) this.#captureHeldCandidateFrame(candidate);
      if (candidate.frames.length === candidate.normalizedTimes.length * candidate.definition.sectorCount) {
        this.#publishHeldCandidate(candidate);
      }
    } catch (cause) {
      candidate.replacementFailureCount += 1;
      this.#heldOutcome(id).replacementFailureCount += 1;
      const published = this.#heldFrameBanks.get(id);
      if (published !== undefined) published.replacementFailureCount += 1;
      this.#disposeHeldCandidate(candidate);
      this.#heldFrameBankCandidates.delete(id);
      this.#revision += 1;
      this.#invalidate();
      return this.#rejected('frame_bank_failed', messageFrom(cause));
    }
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  cancelHeldAnimationFrameBank(id: string): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const candidate = this.#heldFrameBankCandidates.get(id);
    if (candidate === undefined) return this.#rejected('unknown_frame_bank', `unknown preparing frame bank ${id}`);
    candidate.cancelledCount += 1;
    this.#heldOutcome(id).cancelledCount += 1;
    this.#disposeHeldCandidate(candidate);
    this.#heldFrameBankCandidates.delete(id);
    this.#revision += 1;
    this.#invalidate();
    return this.#rejected('frame_bank_cancelled', `frame bank ${id} preparation cancelled`);
  }

  /** Switches only resident source textures through the enhancement seam; it never captures. */
  selectHeldAnimationFrameBank(
    id: string,
    sampleIndex: number,
    directionIndex: number,
  ): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const bank = this.#heldFrameBanks.get(id);
    if (bank === undefined) return this.#rejected('unknown_frame_bank', `unknown ready frame bank ${id}`);
    if (!Number.isInteger(sampleIndex) || sampleIndex < 0 || sampleIndex >= bank.normalizedTimes.length
      || !Number.isInteger(directionIndex) || directionIndex < 0 || directionIndex >= bank.definition.sectorCount) {
      return this.#rejected('invalid_definition', `frame bank ${id} selection is out of range`);
    }
    const frame = bank.frames.find((candidate) => candidate.sampleIndex === sampleIndex && candidate.directionIndex === directionIndex);
    if (frame === undefined) return this.#rejected('frame_bank_failed', `frame bank ${id} has no resident selected frame`);
    const started = nowMilliseconds();
    if (bank.selectedSampleIndex === sampleIndex && bank.selectedDirectionIndex === directionIndex) {
      bank.cacheHitCount += 1;
    } else {
      bank.enhancement.replaceSource({ frame: frame.frame, captureCpuSubmissionMilliseconds: frame.captureCpuMilliseconds });
      bank.selectedSampleIndex = sampleIndex;
      bank.selectedDirectionIndex = directionIndex;
      bank.switchCount += 1;
    }
    bank.lastSwitchCpuMilliseconds = nowMilliseconds() - started;
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  destroyHeldAnimationFrameBank(id: string): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const candidate = this.#heldFrameBankCandidates.get(id);
    const bank = this.#heldFrameBanks.get(id);
    if (candidate === undefined && bank === undefined) return this.#rejected('unknown_frame_bank', `unknown frame bank ${id}`);
    if (candidate !== undefined) {
      this.#disposeHeldCandidate(candidate);
      this.#heldFrameBankCandidates.delete(id);
    }
    if (bank !== undefined) {
      this.#disposeHeldFrameBank(bank);
      this.#heldFrameBanks.delete(id);
    }
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  destroy(id: string): RendererThreeVoxelSpriteReceipt {
    if (this.#disposed) return this.#rejected('disposed', 'voxel sprite scene is disposed');
    const entry = this.#entries.get(id);
    if (entry === undefined) return this.#rejected('unknown_id', `unknown voxel sprite id ${id}`);
    this.#disposeEntry(entry);
    this.#entries.delete(id);
    this.#revision += 1;
    this.#invalidate();
    return this.#applied();
  }

  prepare(camera: THREE.Camera): void {
    if (this.#disposed) return;
    for (const entry of this.#entries.values()) {
      if (entry.ghostPlate !== null) {
        entry.ghostPlate.prepare(camera);
        if (entry.ghostPlate.advancing()) this.#invalidate();
      } else entry.enhancement!.prepare(camera);
    }
    for (const bank of this.#heldFrameBanks.values()) bank.enhancement.prepare(camera);
  }

  recordCpuSubmission(milliseconds: number): void {
    if (this.#disposed) return;
    for (const entry of this.#entries.values()) {
      entry.enhancement?.recordSteadyStateFrame(milliseconds);
    }
    for (const bank of this.#heldFrameBanks.values()) bank.enhancement.recordSteadyStateFrame(milliseconds);
  }

  readout(): RendererThreeVoxelSpriteSceneReadout {
    return Object.freeze({
      schemaVersion: 1,
      revision: this.#revision,
      entries: Object.freeze([...this.#entries.values()]
        .map((entry) => Object.freeze({
          id: entry.id,
          source: entry.source.kind,
          sourceHandle: entry.source.kind === 'retained' ? entry.source.handle : null,
          capture: entry.captureSettings === null ? null : Object.freeze({ ...entry.captureSettings }),
          fallbackPreservedCount: entry.fallbackPreservedCount,
          presentation: entry.ghostPlate === null ? 'enhancement' as const : 'ghost-plate' as const,
          enhancement: entry.enhancement?.readout() ?? null,
          ghostPlate: entry.ghostPlate?.readout() ?? null,
        }))
        .sort((left, right) => codeUnitCompare(left.id, right.id))),
      frameBanks: Object.freeze([...this.#heldFrameBanks.values()]
        .map((bank) => heldFrameBankReadout(bank, 'ready', bank.admissionPeakBytes))
        .sort((left, right) => codeUnitCompare(left.id, right.id))),
      frameBankCandidates: Object.freeze([...this.#heldFrameBankCandidates.values()]
        .map((candidate) => heldCandidateReadout(candidate, candidate.admissionPeakBytes))
        .sort((left, right) => codeUnitCompare(left.id, right.id))),
      frameBankMemory: Object.freeze(this.#frameBankMemoryReadout()),
      frameBankOutcomes: Object.freeze([...this.#heldFrameBankOutcomes.entries()]
        .map(([id, outcome]) => Object.freeze({ id, ...outcome }))
        .sort((left, right) => codeUnitCompare(left.id, right.id))),
      disposed: this.#disposed,
    });
  }

  dispose(): void {
    if (this.#disposed) return;
    for (const entry of this.#entries.values()) this.#disposeEntry(entry);
    this.#entries.clear();
    for (const candidate of this.#heldFrameBankCandidates.values()) this.#disposeHeldCandidate(candidate);
    this.#heldFrameBankCandidates.clear();
    for (const bank of this.#heldFrameBanks.values()) this.#disposeHeldFrameBank(bank);
    this.#heldFrameBanks.clear();
    this.#disposed = true;
    this.#revision += 1;
    this.#invalidate();
    this.#onDispose?.();
  }

  #heldCandidate(definition: RendererThreeHeldAnimationFrameBankDefinition): HeldFrameBankCandidate {
    const createAppearance = this.#backend.createAnimatedMeshCaptureAppearance;
    if (createAppearance === undefined) throw new MissingSourceError('backend does not expose independently posed animated capture appearances');
    // The probe is CPU-only: it validates the admitted clip and gives cadence
    // expansion its actual decoded duration before any capture target exists.
    const probe = createAppearance.call(this.#backend, definition.animatedMesh, definition.clip, 0);
    let normalizedTimes: readonly number[];
    try {
      normalizedTimes = expandHeldAnimationSamples(definition.samples, probe.source.durationSeconds);
    } finally {
      probe.dispose();
    }
    const frameCount = normalizedTimes.length * definition.sectorCount;
    const estimatedBytes = frameCount * definition.capture.resolution * definition.capture.resolution
      * HELD_ANIMATION_BANK_PERSISTENT_BYTES_PER_PIXEL;
    const readyResidentBytes = this.#readyResidentBytes();
    const transientBytes = this.#transientBytes(definition);
    validateHeldBankQuota(
      definition,
      normalizedTimes,
      frameCount,
      estimatedBytes,
      readyResidentBytes,
      transientBytes,
    );
    const key = heldFrameBankKey(definition, normalizedTimes, probe.source);
    return {
      definition,
      key,
      source: probe.source,
      normalizedTimes,
      estimatedBytes,
      admissionPeakBytes: readyResidentBytes + estimatedBytes + transientBytes,
      startedMilliseconds: nowMilliseconds(),
      frames: [],
      nextIndex: 0,
      cancelledCount: this.#heldOutcome(definition.id).cancelledCount,
      replacementFailureCount: this.#heldOutcome(definition.id).replacementFailureCount,
    };
  }

  #captureHeldCandidateFrame(candidate: HeldFrameBankCandidate): void {
    const totalDirections = candidate.definition.sectorCount;
    const sampleIndex = Math.floor(candidate.nextIndex / totalDirections);
    const directionIndex = candidate.nextIndex % totalDirections;
    const normalizedTime = candidate.normalizedTimes[sampleIndex];
    if (normalizedTime === undefined) throw new CaptureSourceError('frame bank capture index is out of range');
    const createAppearance = this.#backend.createAnimatedMeshCaptureAppearance;
    if (createAppearance === undefined) throw new MissingSourceError('backend does not expose independently posed animated capture appearances');
    const appearance = createAppearance.call(this.#backend, candidate.definition.animatedMesh, candidate.definition.clip, normalizedTime);
    const expected = candidate.source;
    if (appearance.source.asset !== expected.asset
      || appearance.source.generation !== expected.generation
      || appearance.source.handle !== expected.handle
      || appearance.source.contentHash !== expected.contentHash
      || appearance.source.clip !== expected.clip
      || appearance.source.origin !== expected.origin
      || appearance.source.pack?.asset !== expected.pack?.asset
      || appearance.source.pack?.contentHash !== expected.pack?.contentHash
      || !sameTransform(appearance.source.instanceTransform, expected.instanceTransform)) {
      appearance.dispose();
      throw new CaptureSourceError('animated source changed while frame-bank preparation was in progress');
    }
    const captureScene = new THREE.Scene();
    const capture = new VoxelSpriteRuntimeCapture(this.#webgl);
    let disposeLighting: () => void = () => undefined;
    try {
      captureScene.add(appearance.object);
      // The temporary appearance is a renderer-owned capture lease. Ensure
      // its drawable descendants reach the capture camera's default layer;
      // this does not alter the canonical instance or its authored layers.
      appearance.object.traverse((object) => {
        // The asset template is intentionally detached; retain no inherited
        // visibility suppression from another instance when this private
        // appearance is rendered into a held frame.
        object.visible = true;
        if (isRenderable(object)) object.layers.enable(0);
      });
      appearance.object.updateWorldMatrix(true, true);
      const bounds = new THREE.Box3().setFromObject(appearance.object, true);
      if (bounds.isEmpty()) throw new CaptureSourceError('animated capture appearance bounds are empty');
      const center = bounds.getCenter(new THREE.Vector3());
      const size = bounds.getSize(new THREE.Vector3());
      const captureSettings = {
        ...candidate.definition.capture,
        azimuthDegrees: normalizedCaptureAzimuth(
          candidate.definition.capture.azimuthDegrees + directionIndex * 360 / totalDirections,
        ),
      };
      const camera = captureCamera(captureSettings, center, size);
      disposeLighting = captureSettings.lighting?.mode === 'scene'
        ? addClonedSceneLights(this.#backend.scene, captureScene)
        : addStudioRig(captureScene, camera, center, size, studioLighting(captureSettings.lighting));
      const receipt = capture.capture({
        scene: captureScene,
        camera,
        width: captureSettings.resolution,
        height: captureSettings.resolution,
        bounds,
      });
      if (!receipt.applied || receipt.frame === null) {
        throw new CaptureSourceError(receipt.diagnostics[0]?.message ?? 'held frame capture failed');
      }
      candidate.frames.push({
        capture,
        frame: receipt.frame,
        sampleIndex,
        directionIndex,
        captureCpuMilliseconds: receipt.readout.cpuSubmissionMilliseconds,
      });
      candidate.nextIndex += 1;
    } catch (cause) {
      capture.dispose();
      throw cause;
    } finally {
      disposeLighting();
      captureScene.remove(appearance.object);
      appearance.dispose();
    }
  }

  #publishHeldCandidate(candidate: HeldFrameBankCandidate): void {
    const first = candidate.frames[0];
    if (first === undefined) throw new CaptureSourceError('frame bank has no captured frames to publish');
    let enhancement: VoxelSpriteEnhancement;
    try {
      enhancement = new VoxelSpriteEnhancement(
        { frame: first.frame, captureCpuSubmissionMilliseconds: first.captureCpuMilliseconds },
        {
          ...candidate.definition.config,
          mode: candidate.definition.mode,
          width: candidate.definition.transform.width,
          height: candidate.definition.transform.height,
        },
      );
    } catch (cause) {
      throw new CaptureSourceError(messageFrom(cause));
    }
    enhancement.object.position.set(...candidate.definition.transform.position);
    const bank: HeldFrameBank = {
      definition: candidate.definition,
      key: candidate.key,
      source: candidate.source,
      normalizedTimes: candidate.normalizedTimes,
      frames: Object.freeze([...candidate.frames]),
      enhancement,
      estimatedBytes: candidate.estimatedBytes,
      admissionPeakBytes: candidate.admissionPeakBytes,
      preparationCpuMilliseconds: nowMilliseconds() - candidate.startedMilliseconds,
      captureCpuMilliseconds: sumCaptureMilliseconds(candidate.frames),
      selectedSampleIndex: 0,
      selectedDirectionIndex: 0,
      captureCount: candidate.frames.length,
      cacheHitCount: 0,
      switchCount: 0,
      lastSwitchCpuMilliseconds: null,
      cancelledCount: candidate.cancelledCount,
      replacementFailureCount: candidate.replacementFailureCount,
    };
    const previous = this.#heldFrameBanks.get(candidate.definition.id);
    this.#backend.scene.add(enhancement.object);
    this.#heldFrameBanks.set(candidate.definition.id, bank);
    this.#heldFrameBankCandidates.delete(candidate.definition.id);
    this.#disposeHeldFrameBank(previous);
  }

  #disposeHeldCandidate(candidate: HeldFrameBankCandidate): void {
    for (const frame of candidate.frames) frame.capture.dispose();
    candidate.frames.length = 0;
  }

  #disposeHeldFrameBank(bank: HeldFrameBank | undefined): void {
    if (bank === undefined) return;
    this.#backend.scene.remove(bank.enhancement.object);
    bank.enhancement.dispose();
    for (const frame of bank.frames) frame.capture.dispose();
  }

  #heldOutcome(id: string): { cancelledCount: number; replacementFailureCount: number } {
    let outcome = this.#heldFrameBankOutcomes.get(id);
    if (outcome === undefined) {
      if (this.#heldFrameBankOutcomes.size >= HELD_ANIMATION_BANK_MAX_RECORDS) {
        const disposableOutcome = [...this.#heldFrameBankOutcomes.keys()].find((candidateId) =>
          !this.#heldFrameBanks.has(candidateId) && !this.#heldFrameBankCandidates.has(candidateId));
        if (disposableOutcome === undefined) {
          throw new RangeError(`frame bank records exceed ${String(HELD_ANIMATION_BANK_MAX_RECORDS)}`);
        }
        this.#heldFrameBankOutcomes.delete(disposableOutcome);
      }
      outcome = { cancelledCount: 0, replacementFailureCount: 0 };
      this.#heldFrameBankOutcomes.set(id, outcome);
    }
    return outcome;
  }

  #readyResidentBytes(): number {
    return [...this.#heldFrameBanks.values()].reduce((total, bank) => total + bank.estimatedBytes, 0);
  }

  #transientBytes(definition: RendererThreeHeldAnimationFrameBankDefinition): number {
    return definition.capture.resolution * definition.capture.resolution * HELD_ANIMATION_BANK_TRANSIENT_BYTES_PER_PIXEL;
  }

  #frameBankMemoryReadout(): { readyResidentBytes: number; candidateResidentBytes: number; candidateReservedBytes: number; peakBytes: number } {
    const candidate = [...this.#heldFrameBankCandidates.values()][0];
    const readyResidentBytes = this.#readyResidentBytes();
    const candidateResidentBytes = candidate === undefined ? 0
      : candidate.frames.length * candidate.definition.capture.resolution * candidate.definition.capture.resolution
        * HELD_ANIMATION_BANK_PERSISTENT_BYTES_PER_PIXEL;
    const candidateReservedBytes = candidate?.estimatedBytes ?? 0;
    return {
      readyResidentBytes,
      candidateResidentBytes,
      candidateReservedBytes,
      peakBytes: candidate === undefined ? readyResidentBytes : candidate.admissionPeakBytes,
    };
  }

  #buildEntry(definition: RendererThreeVoxelSpriteDefinition): Entry {
    if (definition.mode === 'ghost-plate') return this.#buildGhostEntry(definition);
    let frame: VoxelSpriteFrame;
    let runtimeCapture: VoxelSpriteRuntimeCapture | null = null;
    let retainedObject: THREE.Object3D | null = null;
    let retainedOriginalVisibility: boolean | null = null;
    let captureCpuSubmissionMilliseconds: number | null = null;
    if (definition.source.kind === 'prepared') {
      frame = this.#preparedFrame(definition.source.frame);
    } else {
      retainedObject = this.#backend.objectFor(definition.source.handle) ?? null;
      if (retainedObject === null) throw new MissingSourceError(`retained handle ${String(definition.source.handle)} is unavailable`);
      retainedOriginalVisibility = retainedObject.visible;
      runtimeCapture = new VoxelSpriteRuntimeCapture(this.#webgl);
      const receipt = this.#captureRetained(runtimeCapture, retainedObject, definition.source.capture);
      if (!receipt.applied || receipt.frame === null) {
        runtimeCapture.dispose();
        throw new CaptureSourceError(receipt.diagnostics[0]?.message ?? 'runtime capture failed');
      }
      frame = receipt.frame;
      captureCpuSubmissionMilliseconds = receipt.readout.cpuSubmissionMilliseconds;
    }
    let enhancement: VoxelSpriteEnhancement;
    try {
      enhancement = new VoxelSpriteEnhancement(
        { frame, captureCpuSubmissionMilliseconds },
        {
          ...enhancementConfigPatch(definition.config ?? {}),
          mode: definition.mode,
          width: definition.transform.width,
          height: definition.transform.height,
        },
      );
    } catch (cause) {
      runtimeCapture?.dispose();
      if (runtimeCapture === null) frame.dispose();
      throw cause;
    }
    enhancement.object.position.set(...definition.transform.position);
    return {
      id: definition.id,
      enhancement,
      ghostPlate: null,
      frame,
      source: definition.source,
      transform: definition.transform,
      runtimeCapture,
      ghostRuntimeCaptures: Object.freeze([]),
      retainedObject,
      retainedOriginalVisibility,
      releaseCanonicalSuppression: null,
      captureSettings: definition.source.kind === 'retained' ? definition.source.capture : null,
      fallbackPreservedCount: 0,
    };
  }

  #buildGhostEntry(definition: RendererThreeVoxelSpriteDefinition): Entry {
    if (definition.source.kind !== 'retained') {
      throw new TypeError('ghost-plate requires a retained source; prepared frames remain available to ordinary proxy modes');
    }
    const retainedObject = this.#backend.objectFor(definition.source.handle) ?? null;
    if (retainedObject === null) {
      throw new MissingSourceError(`retained handle ${String(definition.source.handle)} is unavailable`);
    }
    const runtimeCaptures: VoxelSpriteRuntimeCapture[] = [];
    let appearanceRoot = SkeletonUtils.clone(retainedObject);
    let ownedGhostGeometries: readonly THREE.BufferGeometry[] = [];
    let presentation: GhostPlateDirectionalPresentation | null = null;
    const plates: GhostPlatePresentation[] = [];
    try {
      retainedObject.updateWorldMatrix(true, true);
      const clonedLights: THREE.Light[] = [];
      appearanceRoot.traverse((object) => {
        if (object instanceof THREE.Light) clonedLights.push(object);
      });
      for (const light of clonedLights) light.removeFromParent();
      appearanceRoot.matrix.copy(retainedObject.matrixWorld);
      appearanceRoot.matrixAutoUpdate = false;
      appearanceRoot.visible = true;
      appearanceRoot.traverse((object) => {
        if (isRenderable(object)) object.layers.enable(0);
      });
      appearanceRoot.updateWorldMatrix(true, true);
      const frozenAppearance = freezeSkinnedMeshes(appearanceRoot);
      appearanceRoot = frozenAppearance.root;
      ownedGhostGeometries = frozenAppearance.ownedGeometries;
      appearanceRoot.updateWorldMatrix(true, true);
      const bounds = new THREE.Box3().setFromObject(appearanceRoot, true);
      if (bounds.isEmpty()) throw new CaptureSourceError('retained source bounds are empty');
      const center = bounds.getCenter(new THREE.Vector3());
      const size = bounds.getSize(new THREE.Vector3());
      const config = validatedGhostConfig(definition.config ?? {});
      const started = nowMilliseconds();
      for (let sector = 0; sector < config.sectorCount; sector += 1) {
        const sectorAppearance = cloneFrozenAppearance(appearanceRoot);
        const captureScene = new THREE.Scene();
        captureScene.add(sectorAppearance.root);
        const captureSettings = {
          ...definition.source.capture,
          azimuthDegrees: normalizedCaptureAzimuth(
            definition.source.capture.azimuthDegrees + sector * 360 / config.sectorCount,
          ),
        };
        const camera = captureCamera(captureSettings, center, size);
        const disposeLighting = captureSettings.lighting?.mode === 'scene'
          ? addClonedSceneLights(this.#backend.scene, captureScene)
          : addStudioRig(captureScene, camera, center, size, studioLighting(captureSettings.lighting));
        const runtimeCapture = new VoxelSpriteRuntimeCapture(this.#webgl);
        runtimeCaptures.push(runtimeCapture);
        const receipt = runtimeCapture.capture({
          scene: captureScene,
          camera,
          width: captureSettings.resolution,
          height: captureSettings.resolution,
          bounds,
        });
        disposeLighting();
        captureScene.remove(sectorAppearance.root);
        if (!receipt.applied || receipt.frame === null) {
          for (const geometry of sectorAppearance.ownedGeometries) geometry.dispose();
          throw new CaptureSourceError(receipt.diagnostics[0]?.message ?? 'runtime capture failed');
        }
        plates.push(new GhostPlatePresentation({
          appearanceRoot: sectorAppearance.root,
          ownedGeometries: sectorAppearance.ownedGeometries,
          colorTexture: receipt.frame.descriptor.textures.color,
          coverageTexture: receipt.frame.descriptor.textures.coverage,
          depthTexture: receipt.frame.descriptor.textures.depth,
          textureWidth: receipt.frame.descriptor.width,
          textureHeight: receipt.frame.descriptor.height,
          captureNear: receipt.frame.descriptor.depth.near,
          captureFar: receipt.frame.descriptor.depth.far,
          projectionKind: camera instanceof THREE.PerspectiveCamera ? 'perspective' : 'orthographic',
          ghostCameraWorld: camera.matrixWorld.clone(),
          ghostProjection: camera.projectionMatrix.clone(),
          bounds,
          transform: definition.transform,
          config,
        }));
      }
      for (const geometry of ownedGhostGeometries) geometry.dispose();
      ownedGhostGeometries = [];
      disposeClonedSkeletons(appearanceRoot);
      presentation = new GhostPlateDirectionalPresentation({
        plates,
        config,
        baseAzimuthDegrees: definition.source.capture.azimuthDegrees,
        preparationCpuMilliseconds: nowMilliseconds() - started,
      });
      return {
        id: definition.id,
        enhancement: null,
        ghostPlate: presentation,
        frame: runtimeCaptures[0]!.currentFrame()!,
        source: definition.source,
        transform: definition.transform,
        runtimeCapture: runtimeCaptures[0]!,
        ghostRuntimeCaptures: Object.freeze(runtimeCaptures),
        retainedObject,
        retainedOriginalVisibility: null,
        releaseCanonicalSuppression: null,
        captureSettings: definition.source.capture,
        fallbackPreservedCount: 0,
      };
    } catch (cause) {
      presentation?.dispose();
      if (presentation === null) {
        for (const plate of plates) plate.dispose();
        disposeClonedSkeletons(appearanceRoot);
        for (const geometry of ownedGhostGeometries) geometry.dispose();
      }
      for (const capture of runtimeCaptures) capture.dispose();
      throw cause;
    }
  }

  #preparedFrame(input: RendererThreeVoxelSpritePreparedFrame): VoxelSpriteFrame {
    const color = this.#backend.textureObjectFor(input.textures.color);
    const depth = this.#backend.textureObjectFor(input.textures.depth);
    const normal = this.#backend.textureObjectFor(input.textures.normal);
    const coverage = this.#backend.textureObjectFor(input.textures.coverage);
    if (color === undefined || depth === undefined || normal === undefined || coverage === undefined) {
      throw new MissingSourceError('one or more prepared voxel sprite textures are unavailable');
    }
    const descriptors = {
      color: this.#backend.textureDescriptor(input.textures.color),
      depth: this.#backend.textureDescriptor(input.textures.depth),
      normal: this.#backend.textureDescriptor(input.textures.normal),
      coverage: this.#backend.textureDescriptor(input.textures.coverage),
    };
    if (Object.values(descriptors).some((descriptor) => descriptor === undefined)) {
      throw new MissingSourceError('one or more prepared voxel sprite texture descriptors are unavailable');
    }
    for (const [channel, descriptor] of Object.entries(descriptors)) {
      if (descriptor!.width !== input.width || descriptor!.height !== input.height) {
        throw new TypeError(`prepared ${channel} texture dimensions do not match frame dimensions`);
      }
    }
    if (descriptors.color!.payload?.colorSpace !== 'srgb') {
      throw new TypeError('prepared color texture must use sRGB color space');
    }
    for (const [channel, descriptor] of Object.entries({
      depth: descriptors.depth!,
      normal: descriptors.normal!,
      coverage: descriptors.coverage!,
    })) {
      if (descriptor.payload?.colorSpace !== 'linear') {
        throw new TypeError(`prepared ${channel} texture must use linear color space`);
      }
    }
    return VoxelSpriteFrame.borrowed({
      width: input.width,
      height: input.height,
      textures: { color, depth, normal, coverage },
      depth: { encoding: 'linear-view-depth-unorm8', ...input.depth },
      normalSpace: 'view',
      capture: {
        projection: input.capture.projection,
        basis: {
          position: input.capture.position,
          right: input.capture.right,
          up: input.capture.up,
          forward: input.capture.forward,
        },
        bounds: {
          minimum: input.capture.boundsMinimum,
          maximum: input.capture.boundsMaximum,
        },
      },
    });
  }

  #captureRetained(
    capture: VoxelSpriteRuntimeCapture,
    source: THREE.Object3D,
    settings: RendererThreeVoxelSpriteCaptureSettings,
  ): ReturnType<VoxelSpriteRuntimeCapture['capture']> {
    const visibility = new Map<THREE.Object3D, boolean>();
    const lightVisibility = new Map<THREE.Light, boolean>();
    const sourceLayers = new Map<THREE.Object3D, number>();
    this.#backend.scene.traverse((object) => {
      if (isRenderable(object)) visibility.set(object, object.visible);
      if (object instanceof THREE.Light) lightVisibility.set(object, object.visible);
    });
    const sourceVisibility = source.visible;
    let disposeStudioRig: () => void = () => undefined;
    try {
      for (const object of visibility.keys()) {
        if (!isDescendantOrSelf(object, source)) object.visible = false;
      }
      source.traverse((object) => {
        if (!isRenderable(object)) return;
        sourceLayers.set(object, object.layers.mask);
        object.layers.enable(0);
      });
      source.visible = true;
      source.updateWorldMatrix(true, true);
      const bounds = new THREE.Box3().setFromObject(source, true);
      if (bounds.isEmpty()) throw new CaptureSourceError('retained source bounds are empty');
      const center = bounds.getCenter(new THREE.Vector3());
      const size = bounds.getSize(new THREE.Vector3());
      const camera = captureCamera(settings, center, size);
      if (settings.lighting?.mode !== 'scene') {
        for (const light of lightVisibility.keys()) light.visible = false;
        disposeStudioRig = addStudioRig(
          this.#backend.scene,
          camera,
          center,
          size,
          studioLighting(settings.lighting),
        );
      }
      return capture.capture({
        scene: this.#backend.scene,
        camera,
        width: settings.resolution,
        height: settings.resolution,
        bounds,
      });
    } finally {
      disposeStudioRig();
      for (const [light, visible] of lightVisibility) light.visible = visible;
      for (const [object, visible] of visibility) object.visible = visible;
      for (const [object, mask] of sourceLayers) object.layers.mask = mask;
      source.visible = sourceVisibility;
    }
  }

  #disposeEntry(entry: Entry): void {
    this.#backend.scene.remove(presentationObject(entry));
    entry.enhancement?.dispose();
    entry.ghostPlate?.dispose();
    if (entry.ghostRuntimeCaptures.length > 0) {
      for (const capture of entry.ghostRuntimeCaptures) capture.dispose();
    } else {
      entry.runtimeCapture?.dispose();
    }
    entry.releaseCanonicalSuppression?.();
    entry.releaseCanonicalSuppression = null;
    if (entry.runtimeCapture === null) entry.frame.dispose();
    if (entry.retainedObject !== null && entry.retainedOriginalVisibility !== null) {
      entry.retainedObject.visible = entry.retainedOriginalVisibility;
    }
  }

  #acquireCanonicalSuppression(source: THREE.Object3D): () => void {
    const existing = this.#canonicalSuppressions.get(source);
    if (existing !== undefined) {
      existing.count += 1;
      return () => this.#releaseCanonicalSuppression(source);
    }
    const layerZeroEnabled = new Map<THREE.Object3D, boolean>();
    source.traverse((object) => {
      if (!isRenderable(object)) return;
      layerZeroEnabled.set(object, (object.layers.mask & 1) !== 0);
      object.layers.disable(0);
    });
    this.#canonicalSuppressions.set(source, { count: 1, layerZeroEnabled });
    return () => this.#releaseCanonicalSuppression(source);
  }

  #releaseCanonicalSuppression(source: THREE.Object3D): void {
    const suppression = this.#canonicalSuppressions.get(source);
    if (suppression === undefined) return;
    suppression.count -= 1;
    if (suppression.count > 0) return;
    for (const [object, enabled] of suppression.layerZeroEnabled) {
      if (enabled) object.layers.enable(0);
      else object.layers.disable(0);
    }
    this.#canonicalSuppressions.delete(source);
  }

  #applied(): RendererThreeVoxelSpriteReceipt {
    return Object.freeze({ applied: true, diagnostics: Object.freeze([]), readout: this.readout() });
  }

  #rejected(
    code: RendererThreeVoxelSpriteDiagnostic['code'],
    message: string,
  ): RendererThreeVoxelSpriteReceipt {
    return Object.freeze({
      applied: false,
      diagnostics: Object.freeze([Object.freeze({ code, message })]),
      readout: this.readout(),
    });
  }
}

class MissingSourceError extends Error {}
class CaptureSourceError extends Error {}

function validatedHeldFrameBankDefinition(
  input: RendererThreeHeldAnimationFrameBankDefinition,
): RendererThreeHeldAnimationFrameBankDefinition {
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,63}$/.test(input.id)) throw new TypeError('frame bank id is invalid');
  if (![1, 4, 8, 16].includes(input.sectorCount)) {
    throw new RangeError('frame bank sector count must be one of 1, 4, 8, or 16');
  }
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,127}$/.test(input.clip)) throw new TypeError('frame bank clip id is invalid');
  finiteTuple(input.transform.position, 'frame bank transform position');
  bounded(input.transform.width, 0.05, 64, 'frame bank transform width');
  bounded(input.transform.height, 0.05, 64, 'frame bank transform height');
  const capture = validatedCapture(input.capture);
  if (capture.resolution > HELD_ANIMATION_BANK_MAX_RESOLUTION) {
    throw new RangeError(`frame bank capture resolution must not exceed ${String(HELD_ANIMATION_BANK_MAX_RESOLUTION)}`);
  }
  if (input.samples.kind === 'exact') {
    if (input.samples.normalizedTimes.length < 1 || input.samples.normalizedTimes.length > HELD_ANIMATION_BANK_MAX_SAMPLES) {
      throw new RangeError(`frame bank exact samples must contain 1 to ${String(HELD_ANIMATION_BANK_MAX_SAMPLES)} values`);
    }
    for (const time of input.samples.normalizedTimes) {
      if (!Number.isFinite(time) || time < 0 || time > 1) throw new RangeError('frame bank exact normalized times must be finite and between 0 and 1');
    }
    if (new Set(input.samples.normalizedTimes).size !== input.samples.normalizedTimes.length) {
      throw new TypeError('frame bank exact normalized times must be unique');
    }
  } else if (input.samples.kind === 'cadence') {
    if (![8, 12, 24].includes(input.samples.samplesPerSecond)) {
      throw new RangeError('frame bank cadence samples per second must be 8, 12, or 24');
    }
    if (!Number.isInteger(input.samples.count) || input.samples.count < 1 || input.samples.count > HELD_ANIMATION_BANK_MAX_SAMPLES) {
      throw new RangeError(`frame bank cadence count must be an integer from 1 to ${String(HELD_ANIMATION_BANK_MAX_SAMPLES)}`);
    }
  } else throw new TypeError('frame bank sample plan is invalid');
  VoxelSpriteEnhancement.validateConfig({
    ...input.config,
    mode: input.mode,
    width: input.transform.width,
    height: input.transform.height,
  });
  return Object.freeze({ ...input, capture });
}

function expandHeldAnimationSamples(
  plan: RendererThreeHeldAnimationSamplePlan,
  durationSeconds: number,
): readonly number[] {
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) throw new TypeError('frame bank clip duration is invalid');
  const samples = plan.kind === 'exact'
    ? [...plan.normalizedTimes]
    : Array.from({ length: plan.count }, (_, index) => index / (plan.samplesPerSecond * durationSeconds));
  if (samples.some((time) => !Number.isFinite(time) || time < 0 || time > 1)) {
    throw new RangeError('frame bank cadence extends beyond the admitted clip duration');
  }
  if (new Set(samples).size !== samples.length) throw new RangeError('frame bank cadence expansion produced duplicate normalized times');
  return Object.freeze(samples);
}

function validateHeldBankQuota(
  definition: RendererThreeHeldAnimationFrameBankDefinition,
  normalizedTimes: readonly number[],
  frameCount: number,
  estimatedBytes: number,
  readyResidentBytes: number,
  transientBytes: number,
): void {
  if (normalizedTimes.length > HELD_ANIMATION_BANK_MAX_SAMPLES || definition.sectorCount > HELD_ANIMATION_BANK_MAX_DIRECTIONS
    || frameCount > HELD_ANIMATION_BANK_MAX_FRAMES) {
    throw new RangeError(`frame bank exceeds ${String(HELD_ANIMATION_BANK_MAX_FRAMES)} pose-direction frames`);
  }
  const pixels = frameCount * definition.capture.resolution * definition.capture.resolution;
  if (pixels > HELD_ANIMATION_BANK_MAX_PIXELS) {
    throw new RangeError(`frame bank exceeds ${String(HELD_ANIMATION_BANK_MAX_PIXELS)} capture pixels`);
  }
  if (readyResidentBytes + estimatedBytes > HELD_ANIMATION_BANK_MAX_RESIDENT_BYTES) {
    throw new RangeError(`frame bank scene exceeds ${String(HELD_ANIMATION_BANK_MAX_RESIDENT_BYTES)} resident bytes`);
  }
  if (readyResidentBytes + estimatedBytes + transientBytes > HELD_ANIMATION_BANK_MAX_PEAK_BYTES) {
    throw new RangeError(`frame bank scene exceeds ${String(HELD_ANIMATION_BANK_MAX_PEAK_BYTES)} peak ready-plus-candidate bytes`);
  }
}

function heldFrameBankKey(
  definition: RendererThreeHeldAnimationFrameBankDefinition,
  normalizedTimes: readonly number[],
  source: AnimatedMeshCaptureAppearance['source'],
): string {
  return `held-animation-frame-bank.v1:${canonicalJson([
    ['asset', source.asset, source.generation, source.handle, source.contentHash, source.instanceTransform],
    ['pack', source.pack?.asset ?? null, source.pack?.contentHash ?? null],
    ['clip', source.clip, source.origin],
    ['samples', normalizedTimes],
    ['sectors', definition.sectorCount],
    ['capture', definition.capture],
    ['resolution', definition.capture.resolution],
    ['dimensions', definition.transform.width, definition.transform.height],
    ['enhancement', definition.mode, definition.config ?? {}],
  ])}`;
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    const record = value as Record<string, unknown>;
    return `{${Object.keys(record).sort(codeUnitCompare).map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

function heldFrameBankReadout(bank: HeldFrameBank, state: 'ready', estimatedPeakBytes: number): RendererThreeHeldAnimationFrameBankReadout {
  return Object.freeze({
    id: bank.definition.id,
    state,
    key: bank.key,
    generation: bank.source.generation,
    source: Object.freeze({
      asset: bank.source.asset,
      assetGeneration: bank.source.generation,
      handle: bank.source.handle,
      contentHash: bank.source.contentHash,
      clip: bank.source.clip,
      origin: bank.source.origin,
      pack: bank.source.pack,
      instanceTransform: bank.source.instanceTransform,
    }),
    frameCount: bank.frames.length,
    directionCount: bank.definition.sectorCount,
    capturedFrameCount: bank.frames.length,
    selectedSampleIndex: bank.selectedSampleIndex,
    selectedDirectionIndex: bank.selectedDirectionIndex,
    captureCount: bank.captureCount,
    cacheHitCount: bank.cacheHitCount,
    switchCount: bank.switchCount,
    preparationCpuMilliseconds: bank.preparationCpuMilliseconds,
    captureCpuMilliseconds: bank.captureCpuMilliseconds,
    lastSwitchCpuMilliseconds: bank.lastSwitchCpuMilliseconds,
    estimatedResidentBytes: bank.estimatedBytes,
    estimatedPeakBytes,
    gpuTiming: 'not-measured',
    cancelledCount: bank.cancelledCount,
    replacementFailureCount: bank.replacementFailureCount,
  });
}

function heldCandidateReadout(candidate: HeldFrameBankCandidate, estimatedPeakBytes: number): RendererThreeHeldAnimationFrameBankReadout {
  return Object.freeze({
    id: candidate.definition.id,
    state: 'preparing',
    key: candidate.key,
    generation: candidate.source.generation,
    source: Object.freeze({
      asset: candidate.source.asset,
      assetGeneration: candidate.source.generation,
      handle: candidate.source.handle,
      contentHash: candidate.source.contentHash,
      clip: candidate.source.clip,
      origin: candidate.source.origin,
      pack: candidate.source.pack,
      instanceTransform: candidate.source.instanceTransform,
    }),
    frameCount: candidate.normalizedTimes.length * candidate.definition.sectorCount,
    directionCount: candidate.definition.sectorCount,
    capturedFrameCount: candidate.frames.length,
    selectedSampleIndex: null,
    selectedDirectionIndex: null,
    captureCount: candidate.frames.length,
    cacheHitCount: 0,
    switchCount: 0,
    preparationCpuMilliseconds: null,
    captureCpuMilliseconds: sumCaptureMilliseconds(candidate.frames),
    lastSwitchCpuMilliseconds: null,
    estimatedResidentBytes: candidate.frames.length * candidate.definition.capture.resolution * candidate.definition.capture.resolution * HELD_ANIMATION_BANK_PERSISTENT_BYTES_PER_PIXEL,
    estimatedPeakBytes,
    gpuTiming: 'not-measured',
    cancelledCount: candidate.cancelledCount,
    replacementFailureCount: candidate.replacementFailureCount,
  });
}

function sumCaptureMilliseconds(frames: readonly HeldCaptureFrame[]): number | null {
  const durations = frames.map((frame) => frame.captureCpuMilliseconds).filter((value): value is number => value !== null);
  return durations.length === 0 ? null : durations.reduce((total, value) => total + value, 0);
}

function sameTransform(
  left: AnimatedMeshCaptureAppearance['source']['instanceTransform'],
  right: AnimatedMeshCaptureAppearance['source']['instanceTransform'],
): boolean {
  const leftValues = [...left.position, ...left.quaternion, ...left.scale];
  const rightValues = [...right.position, ...right.quaternion, ...right.scale];
  return leftValues.every((value, index) => value === rightValues[index]);
}

function codeUnitCompare(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function validatedDefinition(input: RendererThreeVoxelSpriteDefinition): RendererThreeVoxelSpriteDefinition {
  if (!/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,63}$/.test(input.id)) throw new TypeError('voxel sprite id is invalid');
  finiteTuple(input.transform.position, 'transform position');
  bounded(input.transform.width, 0.05, 64, 'transform width');
  bounded(input.transform.height, 0.05, 64, 'transform height');
  if (input.source.kind === 'retained') {
    return Object.freeze({
      ...input,
      source: Object.freeze({ ...input.source, capture: validatedCapture(input.source.capture) }),
    });
  }
  return input;
}

function validatedCapture(input: RendererThreeVoxelSpriteCaptureSettings): RendererThreeVoxelSpriteCaptureSettings {
  if (!Number.isInteger(input.resolution)
    || input.resolution < VOXEL_SPRITE_CAPTURE_MIN_RESOLUTION
    || input.resolution > VOXEL_SPRITE_CAPTURE_MAX_RESOLUTION) {
    throw new RangeError(
      `capture resolution must be an integer from ${String(VOXEL_SPRITE_CAPTURE_MIN_RESOLUTION)}`
      + ` to ${String(VOXEL_SPRITE_CAPTURE_MAX_RESOLUTION)}`,
    );
  }
  bounded(input.azimuthDegrees, -360, 360, 'capture azimuth');
  bounded(input.elevationDegrees, -89, 89, 'capture elevation');
  bounded(input.near, 0.001, 100, 'capture near');
  bounded(input.far, input.near + 0.001, 10_000, 'capture far');
  bounded(input.fieldOfViewDegrees ?? 35, 10, 120, 'capture field of view');
  const lighting = input.lighting?.mode === 'scene'
    ? Object.freeze({ mode: 'scene' as const })
    : studioLighting(input.lighting);
  return Object.freeze({ ...input, lighting });
}

interface NormalizedStudioLighting {
  readonly mode: 'isolated';
  readonly ambientColor: readonly [number, number, number];
  readonly ambientIntensity: number;
  readonly keyDirection: readonly [number, number, number];
  readonly keyColor: readonly [number, number, number];
  readonly keyIntensity: number;
  readonly fillDirection: readonly [number, number, number];
  readonly fillColor: readonly [number, number, number];
  readonly fillIntensity: number;
}

function studioLighting(
  input: Exclude<RendererThreeVoxelSpriteCaptureLighting, { readonly mode: 'scene' }> | undefined,
): NormalizedStudioLighting {
  return Object.freeze({
    mode: 'isolated',
    ambientColor: colorTuple(input?.ambientColor ?? [1, 1, 1], 'capture ambientColor'),
    ambientIntensity: boundedValue(input?.ambientIntensity ?? 1.1, 0, 8, 'capture ambientIntensity'),
    keyDirection: normalizedTuple(input?.keyDirection ?? [0.55, 0.8, 1], 'capture keyDirection'),
    keyColor: colorTuple(input?.keyColor ?? [1, 0.95, 0.85], 'capture keyColor'),
    keyIntensity: boundedValue(input?.keyIntensity ?? 2.4, 0, 8, 'capture keyIntensity'),
    fillDirection: normalizedTuple(input?.fillDirection ?? [-0.7, 0.25, 0.65], 'capture fillDirection'),
    fillColor: colorTuple(input?.fillColor ?? [0.55, 0.7, 1], 'capture fillColor'),
    fillIntensity: boundedValue(input?.fillIntensity ?? 1, 0, 8, 'capture fillIntensity'),
  });
}

function addStudioRig(
  scene: THREE.Scene,
  camera: THREE.Camera,
  center: THREE.Vector3,
  size: THREE.Vector3,
  lighting: NormalizedStudioLighting,
): () => void {
  camera.updateMatrixWorld(true);
  const distance = Math.max(2, size.length() * 2);
  const ambient = new THREE.AmbientLight(
    new THREE.Color().setRGB(...lighting.ambientColor),
    lighting.ambientIntensity,
  );
  const key = studioDirectionalLight(
    camera,
    center,
    distance,
    lighting.keyDirection,
    lighting.keyColor,
    lighting.keyIntensity,
  );
  const fill = studioDirectionalLight(
    camera,
    center,
    distance,
    lighting.fillDirection,
    lighting.fillColor,
    lighting.fillIntensity,
  );
  scene.add(ambient, key.light, key.target, fill.light, fill.target);
  return () => scene.remove(ambient, key.light, key.target, fill.light, fill.target);
}

function studioDirectionalLight(
  camera: THREE.Camera,
  center: THREE.Vector3,
  distance: number,
  direction: readonly [number, number, number],
  color: readonly [number, number, number],
  intensity: number,
): { readonly light: THREE.DirectionalLight; readonly target: THREE.Object3D } {
  const towardLight = new THREE.Vector3(...direction).applyQuaternion(camera.quaternion).normalize();
  const light = new THREE.DirectionalLight(new THREE.Color().setRGB(...color), intensity);
  const target = new THREE.Object3D();
  target.position.copy(center);
  light.position.copy(center).addScaledVector(towardLight, distance);
  light.target = target;
  return { light, target };
}

function boundedValue(value: number, minimum: number, maximum: number, name: string): number {
  bounded(value, minimum, maximum, name);
  return value;
}

function normalizedTuple(
  value: readonly [number, number, number],
  name: string,
): readonly [number, number, number] {
  finiteTuple(value, name);
  const vector = new THREE.Vector3(...value);
  if (vector.lengthSq() < 1e-8) throw new RangeError(`${name} must be nonzero`);
  vector.normalize();
  return Object.freeze(vector.toArray()) as unknown as readonly [number, number, number];
}

function colorTuple(
  value: readonly [number, number, number],
  name: string,
): readonly [number, number, number] {
  finiteTuple(value, name);
  if (value.some((component) => component < 0 || component > 1)) {
    throw new RangeError(`${name} values must be from 0 to 1`);
  }
  return Object.freeze([...value]) as unknown as readonly [number, number, number];
}

function captureCamera(
  settings: RendererThreeVoxelSpriteCaptureSettings,
  center: THREE.Vector3,
  size: THREE.Vector3,
): THREE.PerspectiveCamera {
  const camera = new THREE.PerspectiveCamera(
    settings.fieldOfViewDegrees ?? 35,
    1,
    settings.near,
    settings.far,
  );
  const azimuth = THREE.MathUtils.degToRad(settings.azimuthDegrees);
  const elevation = THREE.MathUtils.degToRad(settings.elevationDegrees);
  const radius = Math.max(size.length() * 1.7, 1);
  camera.position.set(
    center.x + Math.sin(azimuth) * Math.cos(elevation) * radius,
    center.y + Math.sin(elevation) * radius,
    center.z + Math.cos(azimuth) * Math.cos(elevation) * radius,
  );
  camera.lookAt(center);
  camera.updateMatrixWorld(true);
  return camera;
}

function presentationObject(entry: Entry): THREE.Object3D {
  return entry.ghostPlate?.object ?? entry.enhancement!.object;
}

/**
 * Ghost plates own one frozen appearance snapshot. Baking skinned vertices into ordinary meshes
 * makes that snapshot independent from bind matrices and root transforms that no longer match
 * after the captured hierarchy is normalized into the plate's display space.
 */
function freezeSkinnedMeshes(root: THREE.Object3D): {
  readonly root: THREE.Object3D;
  readonly ownedGeometries: readonly THREE.BufferGeometry[];
} {
  const skinnedMeshes: THREE.SkinnedMesh[] = [];
  const clonedSkeletons = new Set<THREE.Skeleton>();
  const ownedGeometries: THREE.BufferGeometry[] = [];
  let frozenRoot = root;
  root.traverse((object) => {
    if (object instanceof THREE.SkinnedMesh) skinnedMeshes.push(object);
  });
  try {
    for (const skinned of skinnedMeshes) {
      const parent = skinned.parent;
      clonedSkeletons.add(skinned.skeleton);
      skinned.skeleton.update();
      const geometry = skinned.geometry.clone();
      ownedGeometries.push(geometry);
      const sourcePosition = skinned.geometry.getAttribute('position');
      const frozenPosition = sourcePosition.clone();
      const vertex = new THREE.Vector3();
      for (let index = 0; index < sourcePosition.count; index += 1) {
        skinned.getVertexPosition(index, vertex);
        frozenPosition.setXYZ(index, vertex.x, vertex.y, vertex.z);
      }
      frozenPosition.needsUpdate = true;
      geometry.setAttribute('position', frozenPosition);
      geometry.deleteAttribute('skinIndex');
      geometry.deleteAttribute('skinWeight');
      geometry.morphAttributes = {};
      geometry.morphTargetsRelative = false;
      geometry.computeBoundingBox();
      geometry.computeBoundingSphere();

      const frozen = new THREE.Mesh(geometry, skinned.material);
      frozen.copy(skinned, false);
      frozen.geometry = geometry;
      frozen.material = skinned.material;
      for (const child of [...skinned.children]) frozen.add(child);
      if (parent === null) {
        frozenRoot = frozen;
      } else {
        const childIndex = parent.children.indexOf(skinned);
        parent.remove(skinned);
        parent.add(frozen);
        parent.children.splice(parent.children.indexOf(frozen), 1);
        parent.children.splice(childIndex, 0, frozen);
      }
    }
  } catch (cause) {
    for (const geometry of ownedGeometries) geometry.dispose();
    throw cause;
  } finally {
    for (const skeleton of clonedSkeletons) skeleton.dispose();
  }
  return { root: frozenRoot, ownedGeometries };
}

function cloneFrozenAppearance(root: THREE.Object3D): {
  readonly root: THREE.Object3D;
  readonly ownedGeometries: readonly THREE.BufferGeometry[];
} {
  const clone = root.clone(true);
  const ownedGeometries: THREE.BufferGeometry[] = [];
  clone.traverse((object) => {
    if (!(object instanceof THREE.Mesh)) return;
    object.geometry = object.geometry.clone();
    ownedGeometries.push(object.geometry);
  });
  clone.updateWorldMatrix(true, true);
  return { root: clone, ownedGeometries };
}

function normalizedCaptureAzimuth(value: number): number {
  const normalized = ((value + 180) % 360 + 360) % 360 - 180;
  return normalized === -180 ? 180 : normalized;
}

function nowMilliseconds(): number {
  return globalThis.performance?.now() ?? Date.now();
}

function enhancementConfigPatch(
  patch: RendererThreeVoxelSpriteConfigPatch,
): Partial<VoxelSpriteEnhancementConfig> {
  const {
    ghostDepthRetention: _depthRetention,
    ghostAnchorPolicy: _anchorPolicy,
    ghostAnchorValue: _anchorValue,
    ghostPlateMapping: _plateMapping,
    ghostShellMode: _shellMode,
    ghostShellDepthEpsilon: _shellDepthEpsilon,
    ghostSectorCount: _sectorCount,
    ghostSectorHysteresisDegrees: _sectorHysteresis,
    ghostTransitionMode: _transitionMode,
    ghostTransitionDurationMilliseconds: _transitionDuration,
    ...enhancement
  } = patch;
  return enhancement;
}

function ghostConfigPatch(patch: RendererThreeVoxelSpriteConfigPatch): Partial<GhostPlateConfig> {
  const allowed = new Set([
    'ghostDepthRetention',
    'ghostAnchorPolicy',
    'ghostAnchorValue',
    'ghostPlateMapping',
    'ghostShellMode',
    'ghostShellDepthEpsilon',
    'ghostSectorCount',
    'ghostSectorHysteresisDegrees',
    'ghostTransitionMode',
    'ghostTransitionDurationMilliseconds',
  ]);
  const unsupported = Object.keys(patch).filter((key) => !allowed.has(key));
  if (unsupported.length > 0) {
    throw new TypeError(`ghost-plate configure does not accept ${unsupported.join(', ')}`);
  }
  return {
    ...(patch.ghostDepthRetention === undefined ? {} : { depthRetention: patch.ghostDepthRetention }),
    ...(patch.ghostAnchorPolicy === undefined ? {} : { anchorPolicy: patch.ghostAnchorPolicy }),
    ...(patch.ghostAnchorValue === undefined ? {} : { anchorValue: patch.ghostAnchorValue }),
    ...(patch.ghostPlateMapping === undefined ? {} : { plateMapping: patch.ghostPlateMapping }),
    ...(patch.ghostShellMode === undefined ? {} : { shellMode: patch.ghostShellMode }),
    ...(patch.ghostShellDepthEpsilon === undefined ? {} : { shellDepthEpsilon: patch.ghostShellDepthEpsilon }),
    ...(patch.ghostSectorCount === undefined ? {} : { sectorCount: patch.ghostSectorCount }),
    ...(patch.ghostSectorHysteresisDegrees === undefined ? {} : { sectorHysteresisDegrees: patch.ghostSectorHysteresisDegrees }),
    ...(patch.ghostTransitionMode === undefined ? {} : { transitionMode: patch.ghostTransitionMode }),
    ...(patch.ghostTransitionDurationMilliseconds === undefined ? {} : { transitionDurationMilliseconds: patch.ghostTransitionDurationMilliseconds }),
  };
}

function validatedGhostConfig(patch: RendererThreeVoxelSpriteConfigPatch): GhostPlateConfig {
  const config: GhostPlateConfig = {
    depthRetention: patch.ghostDepthRetention ?? 0.12,
    anchorPolicy: patch.ghostAnchorPolicy ?? 'bounds-center',
    anchorValue: patch.ghostAnchorValue ?? 0.5,
    plateMapping: patch.ghostPlateMapping ?? 'plate-locked',
    shellMode: patch.ghostShellMode ?? 'whole-mesh',
    shellDepthEpsilon: patch.ghostShellDepthEpsilon ?? 0.12,
    sectorCount: patch.ghostSectorCount ?? 1,
    sectorHysteresisDegrees: patch.ghostSectorHysteresisDegrees ?? 3,
    transitionMode: patch.ghostTransitionMode ?? 'hard-cut',
    transitionDurationMilliseconds: patch.ghostTransitionDurationMilliseconds ?? 180,
  };
  ghostConfigPatch(patch);
  return config;
}

function addClonedSceneLights(source: THREE.Scene, target: THREE.Scene): () => void {
  const clones: THREE.Object3D[] = [];
  source.updateWorldMatrix(true, true);
  source.traverse((object) => {
    if (!(object instanceof THREE.Light) || !object.visible) return;
    const clone = object.clone(false) as THREE.Light;
    object.matrixWorld.decompose(clone.position, clone.quaternion, clone.scale);
    clone.matrixAutoUpdate = true;
    target.add(clone);
    clones.push(clone);
    if ((object instanceof THREE.DirectionalLight || object instanceof THREE.SpotLight)
      && (clone instanceof THREE.DirectionalLight || clone instanceof THREE.SpotLight)) {
      object.target.updateWorldMatrix(true, false);
      const targetClone = new THREE.Object3D();
      object.target.matrixWorld.decompose(targetClone.position, targetClone.quaternion, targetClone.scale);
      clone.target = targetClone;
      target.add(targetClone);
      clones.push(targetClone);
    }
  });
  return () => target.remove(...clones);
}

function disposeClonedSkeletons(root: THREE.Object3D): void {
  const skeletons = new Set<THREE.Skeleton>();
  root.traverse((object) => {
    if (object instanceof THREE.SkinnedMesh) skeletons.add(object.skeleton);
  });
  for (const skeleton of skeletons) skeleton.dispose();
}

function isRenderable(object: THREE.Object3D): boolean {
  return object instanceof THREE.Mesh
    || object instanceof THREE.Line
    || object instanceof THREE.Points
    || object instanceof THREE.Sprite;
}

function isDescendantOrSelf(object: THREE.Object3D, ancestor: THREE.Object3D): boolean {
  let current: THREE.Object3D | null = object;
  while (current !== null) {
    if (current === ancestor) return true;
    current = current.parent;
  }
  return false;
}

function finiteTuple(value: readonly number[], name: string): void {
  if (value.length !== 3 || value.some((component) => !Number.isFinite(component))) {
    throw new TypeError(`${name} must contain three finite values`);
  }
}

function bounded(value: number, minimum: number, maximum: number, name: string): void {
  if (!Number.isFinite(value) || value < minimum || value > maximum) {
    throw new RangeError(`${name} must be finite from ${String(minimum)} to ${String(maximum)}`);
  }
}

function classifyCause(cause: unknown): RendererThreeVoxelSpriteDiagnostic['code'] {
  if (cause instanceof MissingSourceError) return 'missing_source';
  if (cause instanceof CaptureSourceError) return 'capture_failed';
  return 'invalid_definition';
}

function messageFrom(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}
