import type {
  ParticleAnchor,
  ParticleCollisionDescriptor,
  ParticleCollisionVolume,
  ParticleColorKey,
  ParticleEmitterDescriptor,
  ParticleEmitterHandle,
  ParticleEmitterPatch,
  ParticleProjectionOp,
  ParticleScalarKey,
  ParticleSpriteRef,
  ParticleVisual,
  PresentationFrameDiff,
  PresentationOp,
} from '@rusty-engine/render-contracts';
import { rendererResourceContentHash } from './resource-content-hash.js';
import type {
  ParticleProjectionDiagnostic,
  ParticleProjectionReadout,
} from './host-types.js';

type Vec3 = readonly [number, number, number];
type ParticlePresentationOp = Extract<PresentationOp, { readonly domain: 'particle' }>;

export interface RendererParticleResource {
  readonly bytes: ArrayBuffer;
  readonly url: string;
}

export type RendererParticleResourceResolver = (
  sprite: ParticleSpriteRef,
) => Promise<RendererParticleResource | null>;

export type RendererParticleEntityPositionResolver = (entity: number) => Vec3 | null;

export type RendererParticlePreparedVisual =
  | {
      readonly kind: 'billboard';
      readonly frameCount: number;
      readonly spriteUrl: string;
    }
  | { readonly kind: 'cube' };

export interface RendererParticleInstance {
  readonly id: number;
  readonly position: Vec3;
  readonly size: number;
  readonly color: readonly [number, number, number, number];
  readonly frameIndex: number;
  readonly visual: RendererParticlePreparedVisual;
}

export interface RendererParticleSinkReadout {
  readonly activeParticles: number;
  readonly activeBatches: number;
  readonly billboardBatches: number;
  readonly cubeBatches: number;
  readonly allocatedSlots: number;
  readonly highWaterMark: number;
}

export interface RendererParticleSink {
  create(particle: RendererParticleInstance): void;
  update(particle: RendererParticleInstance): void;
  destroy(id: number): void;
  readout?(): RendererParticleSinkReadout;
  dispose?(): void;
}

export interface RendererParticleSceneSink extends RendererParticleSink {
  readout(): RendererParticleSinkReadout;
  dispose(): void;
}

/** @deprecated Use RendererParticleInstance. */
export type RendererParticleBillboard = RendererParticleInstance;
/** @deprecated Use RendererParticleSink. */
export type RendererParticleBillboardSink = RendererParticleSink;

export interface RendererParticleHostOptions {
  readonly maxActiveEmitters?: number;
  readonly maxParticles?: number;
  readonly resolveEntityPosition: RendererParticleEntityPositionResolver;
  readonly resolveResource: RendererParticleResourceResolver;
  readonly sink: RendererParticleSink;
}

export interface RendererParticleFrameReceipt {
  readonly applied: number;
  readonly diagnostics: readonly ParticleProjectionDiagnostic[];
  readonly readout: ParticleProjectionReadout;
}

interface ActiveEmitter {
  descriptor: ParticleEmitterDescriptor;
  preparedVisual: RendererParticlePreparedVisual;
  readonly key: string;
  readonly handle: ParticleEmitterHandle | null;
  randomState: number;
  emissionCarry: number;
  readonly particleIds: Set<number>;
}

interface ActiveParticle {
  readonly id: number;
  readonly emitterKey: string;
  readonly descriptor: ParticleEmitterDescriptor;
  readonly visual: RendererParticlePreparedVisual;
  ageSeconds: number;
  readonly lifetimeSeconds: number;
  position: [number, number, number];
  velocity: [number, number, number];
  readonly collisionOrigin: Vec3;
  impactCount: number;
  sleeping: boolean;
}

export class RendererParticleHost {
  readonly #maxActiveEmitters: number;
  readonly #maxParticles: number;
  readonly #resolveEntityPosition: RendererParticleEntityPositionResolver;
  readonly #resolveResource: RendererParticleResourceResolver;
  readonly #sink: RendererParticleSink;
  readonly #emitters = new Map<number, ActiveEmitter>();
  readonly #burstEmitters = new Map<string, ActiveEmitter>();
  readonly #particles = new Map<number, ActiveParticle>();
  readonly #seenSignals = new Set<string>();
  readonly #spriteUrls = new Map<string, Promise<string>>();
  readonly #diagnostics: ParticleProjectionDiagnostic[] = [];
  #nextParticleId = 1;
  #emittedBursts = 0;
  #droppedParticles = 0;
  #collisionTests = 0;
  #collisionImpacts = 0;
  #highWaterMark = 0;

  constructor(options: RendererParticleHostOptions) {
    this.#maxActiveEmitters = options.maxActiveEmitters ?? 64;
    this.#maxParticles = options.maxParticles ?? 4_096;
    this.#resolveEntityPosition = options.resolveEntityPosition;
    this.#resolveResource = options.resolveResource;
    this.#sink = options.sink;
  }

  async applyPresentation(frame: PresentationFrameDiff): Promise<RendererParticleFrameReceipt> {
    const diagnostics: ParticleProjectionDiagnostic[] = [];
    let applied = 0;
    for (const operation of frame.ops) {
      if (operation.domain !== 'particle') {
        continue;
      }
      const diagnostic = await this.#applyOperation(operation);
      if (diagnostic === null) {
        applied += 1;
      } else {
        diagnostics.push(diagnostic);
        retainParticleDiagnostic(this.#diagnostics, diagnostic);
      }
    }
    return { applied, diagnostics, readout: this.readout() };
  }

  advance(deltaSeconds: number): RendererParticleFrameReceipt {
    if (!Number.isFinite(deltaSeconds) || deltaSeconds < 0 || deltaSeconds > 1) {
      const diagnostic = hostDiagnostic(
        'invalidDescriptor',
        'particle frame delta must be finite and between zero and one second',
      );
      retainParticleDiagnostic(this.#diagnostics, diagnostic);
      return { applied: 0, diagnostics: [diagnostic], readout: this.readout() };
    }
    const diagnostics: ParticleProjectionDiagnostic[] = [];
    for (const emitter of this.#emitters.values()) {
      if (!emitter.descriptor.visible) {
        continue;
      }
      emitter.emissionCarry += emitter.descriptor.ratePerSecond * deltaSeconds;
      const count = Math.floor(emitter.emissionCarry);
      emitter.emissionCarry -= count;
      const diagnostic = this.#spawn(emitter, count, 0);
      if (diagnostic !== null) {
        diagnostics.push(diagnostic);
      }
    }
    for (const particle of [...this.#particles.values()]) {
      particle.ageSeconds += deltaSeconds;
      if (particle.ageSeconds >= particle.lifetimeSeconds) {
        this.#destroyParticle(particle);
        continue;
      }
      if (!particle.sleeping && this.#advanceParticle(particle, deltaSeconds)) {
        this.#destroyParticle(particle);
        continue;
      }
      this.#sink.update(projectParticle(particle));
    }
    this.#cleanupFinishedBursts();
    for (const diagnostic of diagnostics) retainParticleDiagnostic(this.#diagnostics, diagnostic);
    return { applied: this.#particles.size, diagnostics, readout: this.readout() };
  }

  requiresAnimationFrame(): boolean {
    return this.#particles.size > 0
      || this.#burstEmitters.size > 0
      || [...this.#emitters.values()].some((emitter) => (
        emitter.descriptor.visible && emitter.descriptor.ratePerSecond > 0
      ));
  }

  readout(): ParticleProjectionReadout {
    const sink = this.#sink.readout?.();
    return {
      activeEmitters: this.#emitters.size,
      activeParticles: this.#particles.size,
      loadedSprites: this.#spriteUrls.size,
      emittedBursts: this.#emittedBursts,
      droppedParticles: this.#droppedParticles,
      collisionTests: this.#collisionTests,
      collisionImpacts: this.#collisionImpacts,
      highWaterMark: this.#highWaterMark,
      activeBatches: sink?.activeBatches ?? 0,
      allocatedSlots: sink?.allocatedSlots ?? 0,
      diagnostics: [...this.#diagnostics],
    };
  }

  cleanup(): void {
    for (const particle of [...this.#particles.values()]) {
      this.#destroyParticle(particle);
    }
    this.#emitters.clear();
    this.#burstEmitters.clear();
    this.#seenSignals.clear();
  }

  dispose(): void {
    this.cleanup();
    this.#spriteUrls.clear();
    this.#diagnostics.length = 0;
  }

  #advanceParticle(particle: ActiveParticle, deltaSeconds: number): boolean {
    const acceleration = particle.descriptor.acceleration;
    particle.velocity[0] += acceleration[0] * deltaSeconds;
    particle.velocity[1] += acceleration[1] * deltaSeconds;
    particle.velocity[2] += acceleration[2] * deltaSeconds;
    const collision = particle.descriptor.collision;
    if (collision === undefined) {
      addScaled(particle.position, particle.velocity, deltaSeconds);
      return false;
    }
    const remaining = advanceWithCollision(
      particle,
      collision,
      deltaSeconds,
      () => { this.#collisionTests += 1; },
      () => { this.#collisionImpacts += 1; },
    );
    return remaining === 'kill';
  }

  async #applyOperation(
    operation: ParticlePresentationOp,
  ): Promise<ParticleProjectionDiagnostic | null> {
    try {
      switch (operation.op.op) {
        case 'emit':
          return await this.#emit(operation.meta, operation.op);
        case 'create':
          return await this.#create(operation.meta, operation.op);
        case 'update':
          return await this.#update(operation.meta, operation.op);
        case 'destroy':
          return this.#destroy(operation.meta, operation.op);
      }
    } catch (error) {
      return operationDiagnostic(
        error instanceof RendererParticleResourceError ? error.code : 'hostFailure',
        operation.meta,
        operationHandle(operation.op),
        error instanceof Error ? error.message : String(error),
      );
    }
  }

  async #emit(
    meta: ParticlePresentationOp['meta'],
    op: Extract<ParticleProjectionOp, { readonly op: 'emit' }>,
  ): Promise<ParticleProjectionDiagnostic | null> {
    if (this.#seenSignals.has(op.signalId)) {
      return null;
    }
    const preparedVisual = await this.#prepareVisual(op.descriptor);
    const emitter = createEmitter(
      `signal:${op.signalId}`,
      null,
      op.descriptor,
      preparedVisual,
    );
    const diagnostic = this.#spawn(
      emitter,
      op.descriptor.burstCount,
      meta.sequence,
    );
    if (diagnostic?.code === 'anchorMissing') {
      return diagnostic;
    }
    this.#seenSignals.add(op.signalId);
    this.#burstEmitters.set(emitter.key, emitter);
    this.#emittedBursts += 1;
    return diagnostic;
  }

  async #create(
    meta: ParticlePresentationOp['meta'],
    op: Extract<ParticleProjectionOp, { readonly op: 'create' }>,
  ): Promise<ParticleProjectionDiagnostic | null> {
    const rawHandle = op.handle as number;
    if (this.#emitters.has(rawHandle)) {
      return operationDiagnostic(
        'duplicateHandle', meta, op.handle, 'particle emitter handle is already active',
      );
    }
    if (this.#emitters.size >= this.#maxActiveEmitters) {
      return operationDiagnostic(
        'budgetExceeded', meta, op.handle, 'particle emitter budget is exhausted',
      );
    }
    const preparedVisual = await this.#prepareVisual(op.descriptor);
    const emitter = createEmitter(
      `handle:${rawHandle}`,
      op.handle,
      op.descriptor,
      preparedVisual,
    );
    this.#emitters.set(rawHandle, emitter);
    try {
      return this.#spawn(emitter, op.descriptor.burstCount, meta.sequence);
    } catch (error) {
      this.#emitters.delete(rawHandle);
      throw error;
    }
  }

  async #update(
    meta: ParticlePresentationOp['meta'],
    op: Extract<ParticleProjectionOp, { readonly op: 'update' }>,
  ): Promise<ParticleProjectionDiagnostic | null> {
    const emitter = this.#emitters.get(op.handle as number);
    if (emitter === undefined) {
      return operationDiagnostic(
        'unknownHandle', meta, op.handle, 'particle emitter handle is not active',
      );
    }
    const descriptor = applyParticlePatch(emitter.descriptor, op.patch);
    emitter.preparedVisual = await this.#prepareVisual(descriptor);
    emitter.descriptor = descriptor;
    return null;
  }

  #destroy(
    meta: ParticlePresentationOp['meta'],
    op: Extract<ParticleProjectionOp, { readonly op: 'destroy' }>,
  ): ParticleProjectionDiagnostic | null {
    const emitter = this.#emitters.get(op.handle as number);
    if (emitter === undefined) {
      return operationDiagnostic(
        'unknownHandle', meta, op.handle, 'particle emitter handle is not active',
      );
    }
    this.#emitters.delete(op.handle as number);
    for (const id of [...emitter.particleIds]) {
      const particle = this.#particles.get(id);
      if (particle !== undefined) {
        this.#destroyParticle(particle);
      }
    }
    return null;
  }

  #spawn(
    emitter: ActiveEmitter,
    requested: number,
    sequence: number,
  ): ParticleProjectionDiagnostic | null {
    if (requested <= 0 || !emitter.descriptor.visible) {
      return null;
    }
    const anchor = resolveAnchor(emitter.descriptor.anchor, this.#resolveEntityPosition);
    if (anchor === null) {
      return operationDiagnostic(
        'anchorMissing',
        { sequence },
        emitter.handle,
        'particle entity anchor is unavailable',
      );
    }
    const emitterRemaining = Math.max(0, emitter.descriptor.maxParticles - emitter.particleIds.size);
    const hostRemaining = Math.max(0, this.#maxParticles - this.#particles.size);
    const count = Math.min(requested, emitterRemaining, hostRemaining);
    const dropped = requested - count;
    const created: ActiveParticle[] = [];
    try {
      for (let index = 0; index < count; index += 1) {
        const particle = this.#newParticle(emitter, anchor);
        emitter.particleIds.add(particle.id);
        this.#particles.set(particle.id, particle);
        created.push(particle);
        this.#sink.create(projectParticle(particle));
      }
      this.#highWaterMark = Math.max(this.#highWaterMark, this.#particles.size);
      this.#droppedParticles += dropped;
    } catch (error) {
      for (const particle of created.reverse()) {
        this.#particles.delete(particle.id);
        emitter.particleIds.delete(particle.id);
        try {
          this.#sink.destroy(particle.id);
        } catch {
          // Preserve the original sink failure while completing host rollback.
        }
      }
      throw error;
    }
    return count < requested
      ? operationDiagnostic(
          'budgetExceeded',
          { sequence },
          emitter.handle,
          `particle budget dropped ${dropped} particles`,
        )
      : null;
  }

  #newParticle(emitter: ActiveEmitter, anchor: Vec3): ActiveParticle {
    const descriptor = emitter.descriptor;
    const lifetime = randomRange(emitter, descriptor.lifetimeSeconds[0], descriptor.lifetimeSeconds[1]);
    const velocity: [number, number, number] = [
      randomRange(emitter, descriptor.velocityMin[0], descriptor.velocityMax[0]),
      randomRange(emitter, descriptor.velocityMin[1], descriptor.velocityMax[1]),
      randomRange(emitter, descriptor.velocityMin[2], descriptor.velocityMax[2]),
    ];
    return {
      id: this.#nextParticleId++,
      emitterKey: emitter.key,
      descriptor,
      visual: emitter.preparedVisual,
      ageSeconds: 0,
      lifetimeSeconds: lifetime,
      position: [...anchor],
      velocity,
      collisionOrigin: [...anchor],
      impactCount: 0,
      sleeping: false,
    };
  }

  #destroyParticle(particle: ActiveParticle): void {
    this.#particles.delete(particle.id);
    this.#sink.destroy(particle.id);
    this.#emitters.get(Number(particle.emitterKey.slice(7)))?.particleIds.delete(particle.id);
    this.#burstEmitters.get(particle.emitterKey)?.particleIds.delete(particle.id);
  }

  #cleanupFinishedBursts(): void {
    for (const [key, emitter] of this.#burstEmitters) {
      if (emitter.particleIds.size === 0) {
        this.#burstEmitters.delete(key);
      }
    }
  }

  async #prepareSprite(sprite: ParticleSpriteRef): Promise<string> {
    const key = spriteKey(sprite);
    const existing = this.#spriteUrls.get(key);
    if (existing !== undefined) {
      return existing;
    }
    const prepared = this.#resolveResource(sprite).then(async (resource) => {
      if (resource === null) {
        throw new RendererParticleResourceError(
          'spriteLoadFailed', `particle sprite ${sprite.asset} is unavailable`,
        );
      }
      await validateResourceHash(resource.bytes, sprite.contentHash);
      return resource.url;
    });
    this.#spriteUrls.set(key, prepared);
    try {
      return await prepared;
    } catch (error) {
      this.#spriteUrls.delete(key);
      throw error;
    }
  }

  async #prepareVisual(
    descriptor: ParticleEmitterDescriptor,
  ): Promise<RendererParticlePreparedVisual> {
    const visual = descriptorVisual(descriptor);
    if (visual.kind === 'cube') return visual;
    return {
      kind: 'billboard',
      frameCount: visual.sprite.frameCount,
      spriteUrl: await this.#prepareSprite(visual.sprite),
    };
  }

}

const MAX_RETAINED_PARTICLE_DIAGNOSTICS = 256;

function retainParticleDiagnostic(
  diagnostics: ParticleProjectionDiagnostic[],
  diagnostic: ParticleProjectionDiagnostic,
): void {
  const duplicate = diagnostics.findIndex((candidate) => (
    candidate.code === diagnostic.code
    && candidate.handle === diagnostic.handle
    && candidate.message === diagnostic.message
  ));
  if (duplicate >= 0) {
    diagnostics[duplicate] = diagnostic;
    return;
  }
  diagnostics.push(diagnostic);
  if (diagnostics.length > MAX_RETAINED_PARTICLE_DIAGNOSTICS) diagnostics.shift();
}

function createEmitter(
  key: string,
  handle: ParticleEmitterHandle | null,
  descriptor: ParticleEmitterDescriptor,
  preparedVisual: RendererParticlePreparedVisual,
): ActiveEmitter {
  return {
    descriptor,
    preparedVisual,
    key,
    handle,
    randomState: normalizeSeed(descriptor.seed),
    emissionCarry: 0,
    particleIds: new Set(),
  };
}

function normalizeSeed(seed: number): number {
  const normalized = Math.trunc(seed) >>> 0;
  return normalized === 0 ? 0x9e3779b9 : normalized;
}

function randomRange(emitter: ActiveEmitter, min: number, max: number): number {
  let value = emitter.randomState;
  value ^= value << 13;
  value ^= value >>> 17;
  value ^= value << 5;
  emitter.randomState = value >>> 0;
  return min + (max - min) * (emitter.randomState / 0x1_0000_0000);
}

function resolveAnchor(
  anchor: ParticleAnchor,
  resolveEntityPosition: RendererParticleEntityPositionResolver,
): Vec3 | null {
  if (anchor.kind === 'world') {
    return anchor.position;
  }
  const base = resolveEntityPosition(anchor.entity);
  return base === null
    ? null
    : [
        base[0] + anchor.offset[0],
        base[1] + anchor.offset[1],
        base[2] + anchor.offset[2],
      ];
}

function projectParticle(particle: ActiveParticle): RendererParticleInstance {
  const age = Math.min(1, particle.ageSeconds / particle.lifetimeSeconds);
  const frameCount = particle.visual.kind === 'billboard' ? particle.visual.frameCount : 1;
  return {
    id: particle.id,
    position: [...particle.position],
    size: interpolateScalar(particle.descriptor.sizeCurve, age),
    color: interpolateColor(particle.descriptor.colorCurve, age),
    frameIndex: frameCount === 1
      ? 0
      : Math.floor(particle.ageSeconds * particle.descriptor.flipbookFramesPerSecond)
        % frameCount,
    visual: particle.visual,
  };
}

function descriptorVisual(descriptor: ParticleEmitterDescriptor): ParticleVisual {
  if ('visual' in descriptor && descriptor.visual !== undefined) return descriptor.visual;
  return { kind: 'billboard', sprite: descriptor.sprite };
}

function addScaled(
  position: [number, number, number],
  velocity: Vec3,
  seconds: number,
): void {
  position[0] += velocity[0] * seconds;
  position[1] += velocity[1] * seconds;
  position[2] += velocity[2] * seconds;
}

type CollisionAdvanceResult = 'continue' | 'kill';

interface ParticleCollisionHit {
  readonly time: number;
  readonly normal: Vec3;
}

function advanceWithCollision(
  particle: ActiveParticle,
  collision: ParticleCollisionDescriptor,
  seconds: number,
  tested: () => void,
  impacted: () => void,
): CollisionAdvanceResult {
  let remaining = seconds;
  let iterations = 0;
  while (remaining > 1e-6 && iterations < 4) {
    iterations += 1;
    const localStart = subtract(particle.position, particle.collisionOrigin);
    const localEnd: Vec3 = [
      localStart[0] + particle.velocity[0] * remaining,
      localStart[1] + particle.velocity[1] * remaining,
      localStart[2] + particle.velocity[2] * remaining,
    ];
    let earliest: ParticleCollisionHit | null = null;
    for (const volume of collision.volumes) {
      tested();
      const hit = sweepCollisionVolume(localStart, localEnd, collision.radius, volume);
      if (hit !== null && (earliest === null || hit.time < earliest.time)) earliest = hit;
    }
    if (earliest === null) {
      addScaled(particle.position, particle.velocity, remaining);
      return 'continue';
    }
    addScaled(particle.position, particle.velocity, Math.max(0, earliest.time * remaining));
    particle.position[0] += earliest.normal[0] * 1e-4;
    particle.position[1] += earliest.normal[1] * 1e-4;
    particle.position[2] += earliest.normal[2] * 1e-4;
    remaining *= Math.max(0, 1 - earliest.time);
    reflectVelocity(particle.velocity, earliest.normal, collision);
    particle.impactCount += 1;
    impacted();
    if (particle.impactCount >= collision.maximumImpacts) {
      if (collision.limitBehavior === 'kill') return 'kill';
      particle.velocity = [0, 0, 0];
      particle.sleeping = true;
      return 'continue';
    }
    if (vectorLength(particle.velocity) <= collision.sleepSpeed) {
      particle.velocity = [0, 0, 0];
      particle.sleeping = true;
      return 'continue';
    }
  }
  addScaled(particle.position, particle.velocity, remaining);
  return 'continue';
}

function sweepCollisionVolume(
  start: Vec3,
  end: Vec3,
  radius: number,
  volume: ParticleCollisionVolume,
): ParticleCollisionHit | null {
  if (volume.kind === 'plane') {
    const startDistance = dot(volume.normal, start) - volume.offset - radius;
    const endDistance = dot(volume.normal, end) - volume.offset - radius;
    if (startDistance < 0) return { time: 0, normal: volume.normal };
    if (endDistance >= 0 || startDistance === endDistance) return null;
    return {
      time: startDistance / (startDistance - endDistance),
      normal: volume.normal,
    };
  }
  const minimum: Vec3 = [
    volume.minimum[0] - radius,
    volume.minimum[1] - radius,
    volume.minimum[2] - radius,
  ];
  const maximum: Vec3 = [
    volume.maximum[0] + radius,
    volume.maximum[1] + radius,
    volume.maximum[2] + radius,
  ];
  return sweepAabb(start, end, minimum, maximum);
}

function sweepAabb(start: Vec3, end: Vec3, minimum: Vec3, maximum: Vec3): ParticleCollisionHit | null {
  if (insideAabb(start, minimum, maximum)) return nearestAabbExit(start, minimum, maximum);
  let enter = 0;
  let exit = 1;
  let normal: Vec3 = [0, 0, 0];
  for (let axis = 0; axis < 3; axis += 1) {
    const delta = end[axis]! - start[axis]!;
    if (Math.abs(delta) < 1e-9) {
      if (start[axis]! < minimum[axis]! || start[axis]! > maximum[axis]!) return null;
      continue;
    }
    const inverse = 1 / delta;
    let first = (minimum[axis]! - start[axis]!) * inverse;
    let second = (maximum[axis]! - start[axis]!) * inverse;
    let axisNormal = -Math.sign(delta);
    if (first > second) {
      [first, second] = [second, first];
    }
    if (first > enter) {
      enter = first;
      normal = axisVector(axis, axisNormal);
    }
    exit = Math.min(exit, second);
    if (enter > exit) return null;
  }
  return enter >= 0 && enter <= 1 ? { time: enter, normal } : null;
}

function nearestAabbExit(start: Vec3, minimum: Vec3, maximum: Vec3): ParticleCollisionHit {
  let distance = Number.POSITIVE_INFINITY;
  let normal: Vec3 = [0, 1, 0];
  for (let axis = 0; axis < 3; axis += 1) {
    const lowDistance = start[axis]! - minimum[axis]!;
    if (lowDistance < distance) {
      distance = lowDistance;
      normal = axisVector(axis, -1);
    }
    const highDistance = maximum[axis]! - start[axis]!;
    if (highDistance < distance) {
      distance = highDistance;
      normal = axisVector(axis, 1);
    }
  }
  return { time: 0, normal };
}

function reflectVelocity(
  velocity: [number, number, number],
  normal: Vec3,
  collision: ParticleCollisionDescriptor,
): void {
  const normalSpeed = dot(velocity, normal);
  if (normalSpeed >= 0) return;
  const normalComponent: Vec3 = [
    normal[0] * normalSpeed,
    normal[1] * normalSpeed,
    normal[2] * normalSpeed,
  ];
  const tangentialScale = 1 - collision.friction;
  for (let axis = 0; axis < 3; axis += 1) {
    const tangent = velocity[axis]! - normalComponent[axis]!;
    velocity[axis] = tangent * tangentialScale
      - normalComponent[axis]! * collision.restitution;
  }
}

function subtract(left: Vec3, right: Vec3): Vec3 {
  return [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
}

function dot(left: Vec3, right: Vec3): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2];
}

function vectorLength(value: Vec3): number {
  return Math.hypot(value[0], value[1], value[2]);
}

function insideAabb(value: Vec3, minimum: Vec3, maximum: Vec3): boolean {
  return value.every((component, axis) => component >= minimum[axis]! && component <= maximum[axis]!);
}

function axisVector(axis: number, value: number): Vec3 {
  return [axis === 0 ? value : 0, axis === 1 ? value : 0, axis === 2 ? value : 0];
}

function interpolateScalar(keys: readonly ParticleScalarKey[], age: number): number {
  const [left, right] = curvePair(keys, age);
  const blend = curveBlend(left.age, right.age, age);
  return left.value + (right.value - left.value) * blend;
}

function interpolateColor(
  keys: readonly ParticleColorKey[],
  age: number,
): readonly [number, number, number, number] {
  const [left, right] = curvePair(keys, age);
  const blend = curveBlend(left.age, right.age, age);
  return [0, 1, 2, 3].map((index) =>
    left.color[index]! + (right.color[index]! - left.color[index]!) * blend,
  ) as unknown as readonly [number, number, number, number];
}

function curvePair<T extends { readonly age: number }>(keys: readonly T[], age: number): [T, T] {
  for (let index = 1; index < keys.length; index += 1) {
    const right = keys[index]!;
    if (age <= right.age) {
      return [keys[index - 1]!, right];
    }
  }
  return [keys[keys.length - 1]!, keys[keys.length - 1]!];
}

function curveBlend(start: number, end: number, age: number): number {
  return end === start ? 0 : (age - start) / (end - start);
}

function applyParticlePatch(
  descriptor: ParticleEmitterDescriptor,
  patch: ParticleEmitterPatch,
): ParticleEmitterDescriptor {
  const visual = patch.visual
    ?? (patch.sprite === null ? descriptorVisual(descriptor) : { kind: 'billboard', sprite: patch.sprite });
  const collision = patch.collision === undefined
    ? descriptor.collision
    : patch.collision ?? undefined;
  return {
    anchor: patch.anchor ?? descriptor.anchor,
    visual,
    ratePerSecond: patch.ratePerSecond ?? descriptor.ratePerSecond,
    burstCount: patch.burstCount ?? descriptor.burstCount,
    lifetimeSeconds: patch.lifetimeSeconds ?? descriptor.lifetimeSeconds,
    velocityMin: patch.velocityMin ?? descriptor.velocityMin,
    velocityMax: patch.velocityMax ?? descriptor.velocityMax,
    acceleration: patch.acceleration ?? descriptor.acceleration,
    sizeCurve: patch.sizeCurve ?? descriptor.sizeCurve,
    colorCurve: patch.colorCurve ?? descriptor.colorCurve,
    flipbookFramesPerSecond:
      patch.flipbookFramesPerSecond ?? descriptor.flipbookFramesPerSecond,
    seed: descriptor.seed,
    maxParticles: patch.maxParticles ?? descriptor.maxParticles,
    visible: patch.visible ?? descriptor.visible,
    ...(collision === undefined ? {} : { collision }),
  };
}

function operationHandle(op: ParticleProjectionOp): ParticleEmitterHandle | null {
  return op.op === 'emit' ? null : op.handle;
}

function operationDiagnostic(
  code: ParticleProjectionDiagnostic['code'],
  meta: ParticlePresentationOp['meta'],
  handle: ParticleEmitterHandle | null,
  message: string,
): ParticleProjectionDiagnostic {
  return { code, sequence: meta.sequence, handle, message };
}

function hostDiagnostic(
  code: ParticleProjectionDiagnostic['code'],
  message: string,
): ParticleProjectionDiagnostic {
  return { code, sequence: 0, handle: null, message };
}

function spriteKey(sprite: ParticleSpriteRef): string {
  return `${sprite.asset}:${sprite.contentHash}`;
}

async function validateResourceHash(bytes: ArrayBuffer, expected: string): Promise<void> {
  const actual = await rendererResourceContentHash(bytes, expected).catch((error: unknown) => {
    throw new RendererParticleResourceError(
      'contentHashMismatch',
      error instanceof Error ? error.message : String(error),
    );
  });
  if (actual !== expected) {
    throw new RendererParticleResourceError(
      'contentHashMismatch', `particle sprite hash ${actual} does not match ${expected}`,
    );
  }
}

class RendererParticleResourceError extends Error {
  constructor(
    readonly code: 'contentHashMismatch' | 'spriteLoadFailed',
    message: string,
  ) {
    super(message);
  }
}
