import type {
  AnimationControllerProjectionState,
  AnimationProjectionHandle,
  ResolvedAnimationMotion,
  PresentationFrameDiff,
  PresentationOp,
  RenderHandle,
} from '@rusty-engine/render-contracts';
import type {
  AnimationProjectionDiagnostic,
  AnimationProjectionReadout,
} from './host-types.js';
import type {
  RendererAnimatedMeshProjection,
  RendererAnimationControllerClip,
} from './animated-mesh-host.js';

type AnimationPresentationOp = Extract<PresentationOp, { readonly domain: 'animation' }>;

export type RendererAnimationCueSignalDomain = 'audio' | 'particle';

export interface RendererAnimationClipCueDefinition {
  readonly cueId: string;
  readonly asset: string;
  readonly clip: string;
  readonly atSeconds: number;
  readonly signal: {
    readonly domain: RendererAnimationCueSignalDomain;
    readonly id: string;
  };
}

export interface RendererAnimationSampledCue {
  readonly kind: 'rusty.animation.sampled_cue.v1';
  readonly cueId: string;
  readonly handle: AnimationProjectionHandle;
  readonly target: RenderHandle;
  readonly asset: string;
  readonly clip: string;
  readonly markerSeconds: number;
  readonly sampledAtSeconds: number;
  readonly signal: RendererAnimationClipCueDefinition['signal'];
}

export interface RendererAnimationHostOptions {
  readonly cues?: readonly RendererAnimationClipCueDefinition[];
}

export interface RendererAnimationFrameReceipt {
  readonly applied: number;
  readonly diagnostics: readonly AnimationProjectionDiagnostic[];
  readonly cues: readonly RendererAnimationSampledCue[];
  readonly readout: AnimationProjectionReadout;
}

interface AnimationControllerRealization {
  readonly handle: AnimationProjectionHandle;
  readonly target: RenderHandle;
  readonly asset: string;
  readonly tickDurationSeconds: number;
  controller: AnimationControllerProjectionState;
  presented: readonly RendererAnimationControllerClip[];
  interpolation: AnimationWeightInterpolation | null;
  readonly clipSampleSeconds: Map<string, number>;
  readonly emittedCueKeys: Set<string>;
}

interface AnimationWeightInterpolation {
  readonly from: readonly RendererAnimationControllerClip[];
  readonly to: readonly RendererAnimationControllerClip[];
  readonly durationSeconds: number;
  elapsedSeconds: number;
}

export class RendererAnimationHost {
  readonly #projection: RendererAnimatedMeshProjection;
  readonly #cues: readonly RendererAnimationClipCueDefinition[];
  readonly #controllers = new Map<AnimationProjectionHandle, AnimationControllerRealization>();
  readonly #diagnostics: AnimationProjectionDiagnostic[] = [];
  #sampledFrames = 0;
  #compatibilityFallbacks = 0;

  constructor(
    projection: RendererAnimatedMeshProjection,
    options: RendererAnimationHostOptions = {},
  ) {
    this.#projection = projection;
    this.#cues = validateCueDefinitions(options.cues ?? []);
  }

  applyPresentation(frame: PresentationFrameDiff): RendererAnimationFrameReceipt {
    const diagnostics: AnimationProjectionDiagnostic[] = [];
    let applied = 0;
    for (const operation of frame.ops) {
      if (operation.domain !== 'animation') {
        continue;
      }
      const diagnostic = this.#applyOperation(operation);
      if (diagnostic === null) {
        applied += 1;
      } else {
        diagnostics.push(diagnostic);
        this.#diagnostics.push(diagnostic);
      }
    }
    return { applied, diagnostics, cues: [], readout: this.readout() };
  }

  advance(deltaSeconds: number): RendererAnimationFrameReceipt {
    if (!Number.isFinite(deltaSeconds) || deltaSeconds < 0) {
      throw new Error('animation host deltaSeconds must be finite and non-negative');
    }
    const diagnostics: AnimationProjectionDiagnostic[] = [];
    const cues: RendererAnimationSampledCue[] = [];
    for (const realization of this.#controllers.values()) {
      const interpolation = realization.interpolation;
      if (interpolation !== null) {
        interpolation.elapsedSeconds = Math.min(
          interpolation.durationSeconds,
          interpolation.elapsedSeconds + deltaSeconds,
        );
        const progress = interpolation.durationSeconds === 0
          ? 1
          : interpolation.elapsedSeconds / interpolation.durationSeconds;
        realization.presented = interpolateWeights(interpolation.from, interpolation.to, progress);
        try {
          this.#projection.setAnimationControllerWeights(
            realization.target,
            realization.presented,
          );
        } catch (cause) {
          const diagnostic = animationDiagnostic(
            'hostFailure',
            0,
            realization.handle,
            realization.target,
            errorMessage(cause),
          );
          diagnostics.push(diagnostic);
          this.#diagnostics.push(diagnostic);
        }
        if (progress === 1) {
          realization.interpolation = null;
        }
      }
      cues.push(...sampleAnimationCues(realization, this.#cues, deltaSeconds));
    }
    this.#projection.advance(deltaSeconds);
    this.#sampledFrames += 1;
    return { applied: this.#controllers.size, diagnostics, cues, readout: this.readout() };
  }

  readout(): AnimationProjectionReadout {
    return {
      activeControllers: this.#controllers.size,
      sampledFrames: this.#sampledFrames,
      compatibilityFallbacks: this.#compatibilityFallbacks,
      diagnostics: [...this.#diagnostics],
    };
  }

  cleanup(): RendererAnimationFrameReceipt {
    const diagnostics: AnimationProjectionDiagnostic[] = [];
    let applied = 0;
    for (const realization of this.#controllers.values()) {
      try {
        this.#projection.clearAnimationControllerWeights(realization.target);
        applied += 1;
      } catch (cause) {
        const diagnostic = animationDiagnostic(
          'hostFailure',
          0,
          realization.handle,
          realization.target,
          errorMessage(cause),
        );
        diagnostics.push(diagnostic);
        this.#diagnostics.push(diagnostic);
      }
    }
    this.#controllers.clear();
    return { applied, diagnostics, cues: [], readout: this.readout() };
  }

  #applyOperation(operation: AnimationPresentationOp): AnimationProjectionDiagnostic | null {
    const { op, meta } = operation;
    if (op.op === 'create') {
      if (this.#controllers.has(op.handle)) {
        return animationDiagnostic('duplicateHandle', meta.sequence, op.handle, op.descriptor.target, 'animation handle already exists');
      }
      const validation = validateController(op.descriptor.controller);
      if (validation !== null || op.descriptor.tickDurationMillis === 0) {
        return animationDiagnostic('invalidDescriptor', meta.sequence, op.handle, op.descriptor.target, validation ?? 'tick duration must be non-zero');
      }
      if (!this.#projection.hasAnimationTarget(op.descriptor.target)) {
        return animationDiagnostic('unknownTarget', meta.sequence, op.handle, op.descriptor.target, 'animation target is unavailable');
      }
      const playback = this.#projection.playback(op.descriptor.target);
      if (playback.asset === null) {
        return animationDiagnostic('assetMissing', meta.sequence, op.handle, op.descriptor.target, 'animation target has no loaded asset');
      }
      if (playback.asset !== op.descriptor.asset) {
        return animationDiagnostic('incompatibleRig', meta.sequence, op.handle, op.descriptor.target, 'animation descriptor asset does not match the target rig');
      }
      if (playback.contentHash !== op.descriptor.contentHash) {
        return animationDiagnostic(
          'contentHashMismatch',
          meta.sequence,
          op.handle,
          op.descriptor.target,
          'animation descriptor content hash does not match the loaded target rig',
        );
      }
      const weights = controllerWeights(op.descriptor.controller);
      if (!this.#projection.hasAnimationClips(op.descriptor.target, weights.map((clip) => clip.clip))) {
        return animationDiagnostic('clipMissing', meta.sequence, op.handle, op.descriptor.target, 'controller references an unavailable clip');
      }
      try {
        this.#projection.setAnimationControllerWeights(op.descriptor.target, weights);
      } catch (cause) {
        return hostDiagnostic(cause, meta.sequence, op.handle, op.descriptor.target);
      }
      this.#controllers.set(op.handle, {
        handle: op.handle,
        target: op.descriptor.target,
        asset: op.descriptor.asset,
        tickDurationSeconds: op.descriptor.tickDurationMillis / 1_000,
        controller: op.descriptor.controller,
        presented: weights,
        interpolation: null,
        clipSampleSeconds: new Map(),
        emittedCueKeys: new Set(),
      });
      return null;
    }
    const realization = this.#controllers.get(op.handle);
    if (realization === undefined) {
      return animationDiagnostic('unknownHandle', meta.sequence, op.handle, null, 'animation handle is unavailable');
    }
    if (op.op === 'destroy') {
      try {
        this.#projection.clearAnimationControllerWeights(realization.target);
      } catch (cause) {
        return hostDiagnostic(cause, meta.sequence, op.handle, realization.target);
      }
      this.#controllers.delete(op.handle);
      return null;
    }
    const validation = validateController(op.controller);
    if (validation !== null) {
      return animationDiagnostic('invalidDescriptor', meta.sequence, op.handle, realization.target, validation);
    }
    if (op.controller.revision < realization.controller.revision) {
      return animationDiagnostic('staleRevision', meta.sequence, op.handle, realization.target, 'controller revision moved backward');
    }
    if (
      op.controller.revision === realization.controller.revision
      && !isMonotonicSameRevisionUpdate(realization.controller, op.controller)
    ) {
      return animationDiagnostic('staleRevision', meta.sequence, op.handle, realization.target, 'controller state or transition progress moved backward without a new revision');
    }
    const target = controllerWeights(op.controller);
    if (!this.#projection.hasAnimationClips(realization.target, target.map((clip) => clip.clip))) {
      return animationDiagnostic('clipMissing', meta.sequence, op.handle, realization.target, 'controller references an unavailable clip');
    }
    realization.controller = op.controller;
    realization.interpolation = {
      from: realization.presented,
      to: target,
      durationSeconds: realization.tickDurationSeconds,
      elapsedSeconds: 0,
    };
    return null;
  }
}

function validateCueDefinitions(
  definitions: readonly RendererAnimationClipCueDefinition[],
): readonly RendererAnimationClipCueDefinition[] {
  const keys = new Set<string>();
  return definitions.map((definition) => {
    if (
      definition.cueId.trim().length === 0
      || definition.asset.trim().length === 0
      || definition.clip.trim().length === 0
      || definition.signal.id.trim().length === 0
      || !Number.isFinite(definition.atSeconds)
      || definition.atSeconds < 0
    ) {
      throw new Error('animation cue definitions require non-empty identifiers and a finite non-negative marker');
    }
    const key = animationCueKey(definition);
    if (keys.has(key)) {
      throw new Error(`duplicate animation cue definition ${key}`);
    }
    keys.add(key);
    return definition;
  });
}

function sampleAnimationCues(
  realization: AnimationControllerRealization,
  definitions: readonly RendererAnimationClipCueDefinition[],
  deltaSeconds: number,
): readonly RendererAnimationSampledCue[] {
  const activeClips = new Set(
    realization.presented.filter((clip) => clip.weight > 0).map((clip) => clip.clip),
  );
  for (const clip of realization.clipSampleSeconds.keys()) {
    if (!activeClips.has(clip)) {
      realization.clipSampleSeconds.delete(clip);
      for (const definition of definitions) {
        if (definition.asset === realization.asset && definition.clip === clip) {
          realization.emittedCueKeys.delete(animationCueKey(definition));
        }
      }
    }
  }

  const sampled: RendererAnimationSampledCue[] = [];
  for (const clip of realization.presented) {
    if (clip.weight <= 0) {
      continue;
    }
    const prior = realization.clipSampleSeconds.get(clip.clip);
    const sampledAtSeconds = (prior ?? 0) + deltaSeconds * clip.speed;
    realization.clipSampleSeconds.set(clip.clip, sampledAtSeconds);
    for (const definition of definitions) {
      if (definition.asset !== realization.asset || definition.clip !== clip.clip) {
        continue;
      }
      const key = animationCueKey(definition);
      const crossedMarker = prior === undefined
        ? definition.atSeconds <= sampledAtSeconds
        : prior < definition.atSeconds && definition.atSeconds <= sampledAtSeconds;
      if (!crossedMarker || realization.emittedCueKeys.has(key)) {
        continue;
      }
      realization.emittedCueKeys.add(key);
      sampled.push({
        kind: 'rusty.animation.sampled_cue.v1',
        cueId: definition.cueId,
        handle: realization.handle,
        target: realization.target,
        asset: realization.asset,
        clip: definition.clip,
        markerSeconds: definition.atSeconds,
        sampledAtSeconds,
        signal: definition.signal,
      });
    }
  }
  return sampled;
}

function animationCueKey(definition: RendererAnimationClipCueDefinition): string {
  return `${definition.asset}:${definition.clip}:${definition.cueId}`;
}

function isMonotonicSameRevisionUpdate(
  previous: AnimationControllerProjectionState,
  next: AnimationControllerProjectionState,
): boolean {
  if (
    previous.graphId !== next.graphId
    || previous.graphVersion !== next.graphVersion
    || previous.stateId !== next.stateId
    || next.controllerTick < previous.controllerTick
  ) {
    return false;
  }
  if (previous.transition === null) {
    return true;
  }
  if (next.transition === null) {
    return false;
  }
  return previous.transition.transitionId === next.transition.transitionId
    && previous.transition.fromStateId === next.transition.fromStateId
    && previous.transition.toStateId === next.transition.toStateId
    && previous.transition.durationTicks === next.transition.durationTicks
    && next.transition.elapsedTicks >= previous.transition.elapsedTicks;
}

function validateController(controller: AnimationControllerProjectionState): string | null {
  const motions = [controller.motion, controller.transition?.targetMotion].filter(
    (motion): motion is ResolvedAnimationMotion => motion !== undefined,
  );
  for (const motion of motions) {
    if (
      motion.clipA.length === 0
      || motion.blendWeightMilli < 0
      || motion.blendWeightMilli > 1_000
      || motion.speedMilli <= 0
      || (motion.clipB === null && motion.blendWeightMilli !== 0)
    ) {
      return 'controller motion is invalid';
    }
  }
  const transition = controller.transition;
  if (
    transition !== null
    && (transition.durationTicks === 0 || transition.elapsedTicks > transition.durationTicks)
  ) {
    return 'controller transition progress is invalid';
  }
  return null;
}

function controllerWeights(
  controller: AnimationControllerProjectionState,
): readonly RendererAnimationControllerClip[] {
  const transition = controller.transition;
  if (transition === null) {
    return motionWeights(controller.motion);
  }
  const progress = transition.elapsedTicks / transition.durationTicks;
  return mergeWeights([
    ...motionWeights(controller.motion).map((clip) => ({ ...clip, weight: clip.weight * (1 - progress) })),
    ...motionWeights(transition.targetMotion).map((clip) => ({ ...clip, weight: clip.weight * progress })),
  ]);
}

function motionWeights(motion: ResolvedAnimationMotion): readonly RendererAnimationControllerClip[] {
  const highWeight = motion.clipB === null ? 0 : motion.blendWeightMilli / 1_000;
  const clips: RendererAnimationControllerClip[] = [{
    clip: motion.clipA,
    weight: 1 - highWeight,
    speed: motion.speedMilli / 1_000,
  }];
  if (motion.clipB !== null && highWeight > 0) {
    clips.push({ clip: motion.clipB, weight: highWeight, speed: motion.speedMilli / 1_000 });
  }
  return clips;
}

function mergeWeights(
  clips: readonly RendererAnimationControllerClip[],
): readonly RendererAnimationControllerClip[] {
  const merged = new Map<string, RendererAnimationControllerClip>();
  for (const clip of clips) {
    if (clip.weight <= 0) {
      continue;
    }
    const prior = merged.get(clip.clip);
    merged.set(clip.clip, {
      clip: clip.clip,
      weight: (prior?.weight ?? 0) + clip.weight,
      speed: clip.speed,
    });
  }
  return [...merged.values()].sort((left, right) => left.clip.localeCompare(right.clip));
}

function interpolateWeights(
  from: readonly RendererAnimationControllerClip[],
  to: readonly RendererAnimationControllerClip[],
  progress: number,
): readonly RendererAnimationControllerClip[] {
  const clips = new Set([...from.map((clip) => clip.clip), ...to.map((clip) => clip.clip)]);
  return mergeWeights([...clips].map((clip) => {
    const prior = from.find((value) => value.clip === clip);
    const next = to.find((value) => value.clip === clip);
    return {
      clip,
      weight: (prior?.weight ?? 0) + ((next?.weight ?? 0) - (prior?.weight ?? 0)) * progress,
      speed: next?.speed ?? prior?.speed ?? 1,
    };
  }));
}

function hostDiagnostic(
  cause: unknown,
  sequence: number,
  handle: AnimationProjectionHandle,
  target: RenderHandle,
): AnimationProjectionDiagnostic {
  const message = errorMessage(cause);
  const code = message.includes('missing clip') ? 'clipMissing' : message.includes('handle') ? 'unknownTarget' : 'hostFailure';
  return animationDiagnostic(code, sequence, handle, target, message);
}

function animationDiagnostic(
  code: AnimationProjectionDiagnostic['code'],
  sequence: number,
  handle: AnimationProjectionHandle | null,
  target: RenderHandle | null,
  message: string,
): AnimationProjectionDiagnostic {
  return { code, sequence, handle, target, message };
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}
