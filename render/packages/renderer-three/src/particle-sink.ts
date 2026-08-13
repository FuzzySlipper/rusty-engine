import * as THREE from 'three';

type Vec3 = readonly [number, number, number];
type Vec4 = readonly [number, number, number, number];

export type RendererThreeParticleVisual =
  | { readonly kind: 'billboard'; readonly frameCount: number; readonly spriteUrl: string }
  | { readonly kind: 'cube' };

/** Structural counterpart of renderer-host's particle instance. */
export interface RendererThreeParticleInstance {
  readonly id: number;
  readonly position: Vec3;
  readonly size: number;
  readonly color: Vec4;
  readonly frameIndex: number;
  readonly visual: RendererThreeParticleVisual;
}

export interface RendererThreeParticleSinkOptions {
  readonly scene: THREE.Scene;
  readonly batchCapacity?: number;
  readonly pixelsPerWorldUnit?: number;
  readonly textureFactory?: (url: string) => THREE.Texture;
}

export interface RendererThreeParticleSinkReadout {
  readonly activeParticles: number;
  readonly activeBatches: number;
  readonly billboardBatches: number;
  readonly cubeBatches: number;
  readonly allocatedSlots: number;
  readonly highWaterMark: number;
}

interface ParticleBatch {
  readonly kind: RendererThreeParticleVisual['kind'];
  readonly capacity: number;
  readonly activeCount: number;
  readonly object: THREE.Object3D;
  hasCapacity(): boolean;
  create(particle: RendererThreeParticleInstance): void;
  update(particle: RendererThreeParticleInstance): void;
  destroy(id: number): boolean;
  dispose(): void;
}

const BILLBOARD_VERTEX_SHADER = `
attribute float particleSize;
attribute float particleFrame;
attribute vec4 particleColor;
uniform float pixelsPerWorldUnit;
varying float vParticleFrame;
varying vec4 vParticleColor;
void main() {
  vParticleFrame = particleFrame;
  vParticleColor = particleColor;
  vec4 viewPosition = modelViewMatrix * vec4(position, 1.0);
  gl_Position = projectionMatrix * viewPosition;
  gl_PointSize = max(1.0, particleSize * pixelsPerWorldUnit);
}
`;

const BILLBOARD_FRAGMENT_SHADER = `
uniform sampler2D particleMap;
uniform float frameCount;
varying float vParticleFrame;
varying vec4 vParticleColor;
void main() {
  float frame = clamp(floor(vParticleFrame + 0.5), 0.0, frameCount - 1.0);
  vec2 frameUv = vec2((frame + gl_PointCoord.x) / frameCount, 1.0 - gl_PointCoord.y);
  vec4 sampled = texture2D(particleMap, frameUv);
  vec4 color = sampled * vParticleColor;
  if (color.a <= 0.001) discard;
  gl_FragColor = color;
}
`;

const CUBE_VERTEX_SHADER = `
attribute vec4 particleColor;
varying vec4 vParticleColor;
void main() {
  vParticleColor = particleColor;
  gl_Position = projectionMatrix * modelViewMatrix * instanceMatrix * vec4(position, 1.0);
}
`;

const CUBE_FRAGMENT_SHADER = `
varying vec4 vParticleColor;
void main() {
  if (vParticleColor.a <= 0.001) discard;
  gl_FragColor = vParticleColor;
}
`;

/**
 * Pooled Three-scene realization for disposable billboard and cube particles.
 * It accepts presentation instances only and exposes no scene mutation policy.
 */
export class RendererThreeParticleSink {
  readonly #group = new THREE.Group();
  readonly #scene: THREE.Scene;
  readonly #batchCapacity: number;
  readonly #pixelsPerWorldUnit: number;
  readonly #textureFactory: (url: string) => THREE.Texture;
  readonly #batchesByKey = new Map<string, ParticleBatch[]>();
  readonly #batchByParticle = new Map<number, ParticleBatch>();
  readonly #billboardMaterials = new Map<string, { texture: THREE.Texture; material: THREE.ShaderMaterial }>();
  readonly #cubeMaterial = new THREE.ShaderMaterial({
    transparent: true,
    depthTest: true,
    depthWrite: true,
    vertexShader: CUBE_VERTEX_SHADER,
    fragmentShader: CUBE_FRAGMENT_SHADER,
  });
  #highWaterMark = 0;
  #disposed = false;

  constructor(options: RendererThreeParticleSinkOptions) {
    const capacity = options.batchCapacity ?? 256;
    const pixelsPerWorldUnit = options.pixelsPerWorldUnit ?? 24;
    if (!Number.isSafeInteger(capacity) || capacity < 1 || capacity > 4_096) {
      throw new RangeError('particle batchCapacity must be an integer in 1..=4096');
    }
    if (!Number.isFinite(pixelsPerWorldUnit) || pixelsPerWorldUnit <= 0) {
      throw new RangeError('particle pixelsPerWorldUnit must be finite and positive');
    }
    this.#scene = options.scene;
    this.#batchCapacity = capacity;
    this.#pixelsPerWorldUnit = pixelsPerWorldUnit;
    this.#textureFactory = options.textureFactory ?? ((url) => new THREE.TextureLoader().load(url));
    this.#group.name = 'rusty-particles';
    this.#scene.add(this.#group);
  }

  create(particle: RendererThreeParticleInstance): void {
    this.#assertLive();
    if (this.#batchByParticle.has(particle.id)) {
      throw new Error(`particle ${String(particle.id)} already exists`);
    }
    const key = visualKey(particle.visual);
    const batches = this.#batchesByKey.get(key) ?? [];
    let batch = batches.find((candidate) => candidate.hasCapacity());
    if (batch === undefined) {
      batch = this.#createBatch(particle.visual);
      batches.push(batch);
      this.#batchesByKey.set(key, batches);
      this.#group.add(batch.object);
    }
    batch.create(particle);
    this.#batchByParticle.set(particle.id, batch);
    this.#highWaterMark = Math.max(this.#highWaterMark, this.#batchByParticle.size);
  }

  update(particle: RendererThreeParticleInstance): void {
    this.#assertLive();
    const batch = this.#batchByParticle.get(particle.id);
    if (batch === undefined) throw new Error(`particle ${String(particle.id)} does not exist`);
    if (batch.kind !== particle.visual.kind) {
      throw new Error(`particle ${String(particle.id)} cannot change visual kind after spawning`);
    }
    batch.update(particle);
  }

  destroy(id: number): void {
    if (this.#disposed) return;
    const batch = this.#batchByParticle.get(id);
    if (batch === undefined) return;
    batch.destroy(id);
    this.#batchByParticle.delete(id);
  }

  readout(): RendererThreeParticleSinkReadout {
    const batches = [...this.#batchesByKey.values()].flat();
    return Object.freeze({
      activeParticles: this.#batchByParticle.size,
      activeBatches: batches.filter((batch) => batch.activeCount > 0).length,
      billboardBatches: batches.filter((batch) => batch.kind === 'billboard').length,
      cubeBatches: batches.filter((batch) => batch.kind === 'cube').length,
      allocatedSlots: batches.reduce((total, batch) => total + batch.capacity, 0),
      highWaterMark: this.#highWaterMark,
    });
  }

  dispose(): void {
    if (this.#disposed) return;
    for (const batch of [...this.#batchesByKey.values()].flat()) {
      this.#group.remove(batch.object);
      batch.dispose();
    }
    this.#batchesByKey.clear();
    this.#batchByParticle.clear();
    for (const resource of this.#billboardMaterials.values()) {
      resource.material.dispose();
      resource.texture.dispose();
    }
    this.#billboardMaterials.clear();
    this.#cubeMaterial.dispose();
    this.#scene.remove(this.#group);
    this.#disposed = true;
  }

  #createBatch(visual: RendererThreeParticleVisual): ParticleBatch {
    if (visual.kind === 'cube') {
      return new CubeParticleBatch(this.#batchCapacity, this.#cubeMaterial);
    }
    const key = visualKey(visual);
    let resource = this.#billboardMaterials.get(key);
    if (resource === undefined) {
      const texture = this.#textureFactory(visual.spriteUrl);
      texture.minFilter = THREE.NearestFilter;
      texture.magFilter = THREE.NearestFilter;
      texture.generateMipmaps = false;
      const material = new THREE.ShaderMaterial({
        transparent: true,
        depthTest: true,
        depthWrite: false,
        uniforms: {
          particleMap: { value: texture },
          frameCount: { value: visual.frameCount },
          pixelsPerWorldUnit: { value: this.#pixelsPerWorldUnit },
        },
        vertexShader: BILLBOARD_VERTEX_SHADER,
        fragmentShader: BILLBOARD_FRAGMENT_SHADER,
      });
      resource = { texture, material };
      this.#billboardMaterials.set(key, resource);
    }
    return new BillboardParticleBatch(this.#batchCapacity, resource.material);
  }

  #assertLive(): void {
    if (this.#disposed) throw new Error('particle sink is disposed');
  }
}

abstract class PackedParticleBatch implements ParticleBatch {
  abstract readonly kind: RendererThreeParticleVisual['kind'];
  abstract readonly object: THREE.Object3D;
  readonly capacity: number;
  readonly #ids: number[] = [];
  readonly #particles = new Map<number, RendererThreeParticleInstance>();
  readonly #slotById = new Map<number, number>();

  constructor(capacity: number) {
    this.capacity = capacity;
  }

  get activeCount(): number {
    return this.#ids.length;
  }

  hasCapacity(): boolean {
    return this.activeCount < this.capacity;
  }

  create(particle: RendererThreeParticleInstance): void {
    if (!this.hasCapacity()) throw new Error('particle batch is full');
    const slot = this.#ids.length;
    this.#ids.push(particle.id);
    this.#slotById.set(particle.id, slot);
    this.#particles.set(particle.id, particle);
    this.write(slot, particle);
    this.commitCount(this.#ids.length);
  }

  update(particle: RendererThreeParticleInstance): void {
    const slot = this.#slotById.get(particle.id);
    if (slot === undefined) throw new Error(`particle ${String(particle.id)} is not in batch`);
    this.#particles.set(particle.id, particle);
    this.write(slot, particle);
  }

  destroy(id: number): boolean {
    const slot = this.#slotById.get(id);
    if (slot === undefined) return false;
    const lastSlot = this.#ids.length - 1;
    const lastId = this.#ids[lastSlot]!;
    if (slot !== lastSlot) {
      const lastParticle = this.#particles.get(lastId)!;
      this.#ids[slot] = lastId;
      this.#slotById.set(lastId, slot);
      this.write(slot, lastParticle);
    }
    this.#ids.pop();
    this.#slotById.delete(id);
    this.#particles.delete(id);
    this.commitCount(this.#ids.length);
    return true;
  }

  abstract write(slot: number, particle: RendererThreeParticleInstance): void;
  abstract commitCount(count: number): void;
  abstract dispose(): void;
}

class BillboardParticleBatch extends PackedParticleBatch {
  readonly kind = 'billboard' as const;
  readonly object: THREE.Points;
  readonly #geometry: THREE.BufferGeometry;
  readonly #position: THREE.BufferAttribute;
  readonly #size: THREE.BufferAttribute;
  readonly #frame: THREE.BufferAttribute;
  readonly #color: THREE.BufferAttribute;

  constructor(capacity: number, material: THREE.ShaderMaterial) {
    super(capacity);
    this.#geometry = new THREE.BufferGeometry();
    this.#position = new THREE.BufferAttribute(new Float32Array(capacity * 3), 3);
    this.#size = new THREE.BufferAttribute(new Float32Array(capacity), 1);
    this.#frame = new THREE.BufferAttribute(new Float32Array(capacity), 1);
    this.#color = new THREE.BufferAttribute(new Float32Array(capacity * 4), 4);
    this.#geometry.setAttribute('position', this.#position);
    this.#geometry.setAttribute('particleSize', this.#size);
    this.#geometry.setAttribute('particleFrame', this.#frame);
    this.#geometry.setAttribute('particleColor', this.#color);
    this.#geometry.setDrawRange(0, 0);
    this.object = new THREE.Points(this.#geometry, material);
    this.object.frustumCulled = false;
  }

  write(slot: number, particle: RendererThreeParticleInstance): void {
    this.#position.setXYZ(slot, ...particle.position);
    this.#size.setX(slot, particle.size);
    this.#frame.setX(slot, particle.frameIndex);
    this.#color.setXYZW(slot, ...particle.color);
    this.#position.needsUpdate = true;
    this.#size.needsUpdate = true;
    this.#frame.needsUpdate = true;
    this.#color.needsUpdate = true;
  }

  commitCount(count: number): void {
    this.#geometry.setDrawRange(0, count);
  }

  dispose(): void {
    this.#geometry.dispose();
  }
}

class CubeParticleBatch extends PackedParticleBatch {
  readonly kind = 'cube' as const;
  readonly object: THREE.InstancedMesh;
  readonly #geometry: THREE.BoxGeometry;
  readonly #color: THREE.InstancedBufferAttribute;
  readonly #matrix = new THREE.Matrix4();
  readonly #position = new THREE.Vector3();
  readonly #scale = new THREE.Vector3();
  readonly #rotation = new THREE.Quaternion();

  constructor(capacity: number, material: THREE.ShaderMaterial) {
    super(capacity);
    this.#geometry = new THREE.BoxGeometry(1, 1, 1);
    this.#color = new THREE.InstancedBufferAttribute(new Float32Array(capacity * 4), 4);
    this.#geometry.setAttribute('particleColor', this.#color);
    this.object = new THREE.InstancedMesh(this.#geometry, material, capacity);
    this.object.count = 0;
    this.object.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    this.object.frustumCulled = false;
  }

  write(slot: number, particle: RendererThreeParticleInstance): void {
    this.#position.set(...particle.position);
    this.#scale.setScalar(Math.max(0, particle.size));
    this.#matrix.compose(this.#position, this.#rotation, this.#scale);
    this.object.setMatrixAt(slot, this.#matrix);
    this.#color.setXYZW(slot, ...particle.color);
    this.object.instanceMatrix.needsUpdate = true;
    this.#color.needsUpdate = true;
  }

  commitCount(count: number): void {
    this.object.count = count;
  }

  dispose(): void {
    this.#geometry.dispose();
  }
}

function visualKey(visual: RendererThreeParticleVisual): string {
  return visual.kind === 'cube'
    ? 'cube'
    : `billboard:${visual.spriteUrl}:${String(visual.frameCount)}`;
}
