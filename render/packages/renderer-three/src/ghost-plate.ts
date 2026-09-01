import * as THREE from 'three';

export type GhostPlateAnchorPolicy = 'bounds-center' | 'bounds-normalized';
export type GhostPlateMapping = 'plate-locked' | 'projective-surface';
export type GhostPlateShellMode = 'whole-mesh' | 'strict-source' | 'repaired-source';
export type GhostPlateSectorCount = 1 | 4 | 8 | 16;

export interface GhostPlateConfig {
  readonly depthRetention: number;
  readonly anchorPolicy: GhostPlateAnchorPolicy;
  readonly anchorValue: number;
  readonly plateMapping: GhostPlateMapping;
  readonly shellMode: GhostPlateShellMode;
  /** Requested source-view depth tolerance in capture-view units. */
  readonly shellDepthEpsilon: number;
  readonly sectorCount: GhostPlateSectorCount;
  readonly sectorHysteresisDegrees: number;
}

export interface GhostPlateCaptureSnapshot {
  readonly appearanceRoot: THREE.Object3D;
  readonly ownedGeometries?: readonly THREE.BufferGeometry[];
  readonly colorTexture: THREE.Texture;
  readonly coverageTexture: THREE.Texture;
  readonly depthTexture: THREE.Texture;
  readonly textureWidth: number;
  readonly textureHeight: number;
  readonly captureNear: number;
  readonly captureFar: number;
  readonly projectionKind: 'perspective' | 'orthographic';
  readonly ghostCameraWorld: THREE.Matrix4;
  readonly ghostProjection: THREE.Matrix4;
  readonly bounds: THREE.Box3;
  readonly transform: {
    readonly position: readonly [number, number, number];
    readonly width: number;
    readonly height: number;
  };
  readonly config: GhostPlateConfig;
}

export interface GhostPlateReadout {
  readonly schemaVersion: 1;
  readonly enabled: boolean;
  readonly fallbackActive: boolean;
  readonly fallbackReason: null | 'prepared-source-unsupported' | 'sector-selection-failed';
  readonly matchedPose: boolean;
  readonly projection: 'perspective' | 'orthographic';
  readonly captureBasis: {
    readonly position: readonly [number, number, number];
    readonly right: readonly [number, number, number];
    readonly up: readonly [number, number, number];
    readonly forward: readonly [number, number, number];
  };
  /** Current display-space realization of the exact source camera. */
  readonly sourceViewBasis: {
    readonly position: readonly [number, number, number];
    readonly right: readonly [number, number, number];
    readonly up: readonly [number, number, number];
    readonly forward: readonly [number, number, number];
  };
  readonly depthRetention: number;
  readonly anchorPolicy: GhostPlateAnchorPolicy;
  readonly anchorValue: number;
  readonly anchorDepth: number;
  readonly plateMapping: GhostPlateMapping;
  readonly shellMode: GhostPlateShellMode;
  readonly shellDepthEpsilon: number;
  readonly shellDepthQuantizationStep: number;
  readonly shellEffectiveDepthEpsilon: number;
  readonly rejectedFragmentRatio: { readonly status: 'unavailable'; readonly value: null };
  readonly repairedBoundaryRatio: { readonly status: 'unavailable'; readonly value: null };
  readonly angularOffsetDegrees: number | null;
  readonly sectorCount: GhostPlateSectorCount;
  readonly selectedSector: number;
  readonly pendingSector: number | null;
  readonly previousSector: number | null;
  readonly localAzimuthDegrees: number | null;
  readonly sectorHysteresisDegrees: number;
  readonly residentSectorCount: number;
  readonly currentResourceResident: boolean;
  readonly previousResourceResident: boolean;
  readonly preparationCpuMilliseconds: number | null;
  readonly invalidationReason: string | null;
  readonly expectedDrawCalls: number;
  readonly meshCount: number;
  readonly materialResourceCount: number;
  readonly borrowedTextureCount: number;
  readonly disposed: boolean;
  readonly limitations: readonly string[];
}

export interface GhostPlateWarpResult {
  readonly position: readonly [number, number, number];
  readonly sourceNdc: readonly [number, number];
}

export interface GhostPlateShellSample {
  readonly depth: number;
  readonly coverage: number;
}

/** CPU reference for the bounded source-shell admission used by deterministic tests. */
export function evaluateGhostPlateShell(
  sourceDepth: number,
  center: GhostPlateShellSample,
  neighbors: readonly GhostPlateShellSample[],
  captureNear: number,
  captureFar: number,
  mode: GhostPlateShellMode,
  epsilon: number,
): { readonly accepted: boolean; readonly repaired: boolean } {
  positive(sourceDepth, 'ghost source depth');
  if (!Number.isFinite(captureNear) || !Number.isFinite(captureFar)
    || captureNear < 0 || captureFar <= captureNear) {
    throw new RangeError('ghost capture depth range must be finite and increasing');
  }
  bounded(epsilon, 0, 2, 'ghost shell depth epsilon');
  if (mode === 'whole-mesh') return Object.freeze({ accepted: true, repaired: false });
  if (mode !== 'strict-source' && mode !== 'repaired-source') {
    throw new TypeError(`unsupported ghost shell mode ${String(mode)}`);
  }
  const tolerance = epsilon + (captureFar - captureNear) / 510;
  const agrees = (sample: GhostPlateShellSample): boolean => {
    bounded(sample.depth, 0, 1, 'ghost shell sample depth');
    bounded(sample.coverage, 0, 1, 'ghost shell sample coverage');
    const sampledDepth = captureNear + sample.depth * (captureFar - captureNear);
    return sample.coverage >= 0.5 && Math.abs(sourceDepth - sampledDepth) <= tolerance;
  };
  if (agrees(center)) return Object.freeze({ accepted: true, repaired: false });
  if (mode === 'repaired-source' && neighbors.some(agrees)) {
    return Object.freeze({ accepted: true, repaired: true });
  }
  return Object.freeze({ accepted: false, repaired: false });
}

const LIMITATIONS = Object.freeze([
  'retained-source-only',
  'single-capture-view',
  'frozen-appearance-pose',
  'whole-hierarchy-relief',
  'rgba8-shell-depth',
  'fragment-ratios-unavailable-without-readback',
  'gpu-time-not-measured',
] as const);

const MIN_GHOST_DEPTH = 1e-4;

/** Pure camera-space reference used by CPU projection invariance tests. */
export function warpGhostCameraPoint(
  point: readonly [number, number, number],
  projection: THREE.Matrix4,
  projectionKind: 'perspective' | 'orthographic',
  anchorDepth: number,
  depthRetention: number,
): GhostPlateWarpResult {
  finiteTuple(point, 'ghost camera point');
  finiteMatrix(projection, 'ghost projection');
  positive(anchorDepth, 'ghost anchor depth');
  bounded(depthRetention, 0.02, 1, 'ghost depth retention');
  const depth = -point[2];
  if (!Number.isFinite(depth) || depth <= MIN_GHOST_DEPTH) {
    throw new RangeError('ghost camera point must be in front of the capture camera');
  }
  const warpedDepth = Math.max(
    MIN_GHOST_DEPTH,
    anchorDepth + depthRetention * (depth - anchorDepth),
  );
  const scale = projectionKind === 'perspective' ? warpedDepth / depth : 1;
  const position = Object.freeze([
    point[0] * scale,
    point[1] * scale,
    -warpedDepth,
  ]) as readonly [number, number, number];
  const source = new THREE.Vector4(...point, 1).applyMatrix4(projection);
  if (!Number.isFinite(source.w) || Math.abs(source.w) <= Number.EPSILON) {
    throw new RangeError('ghost source projection produced an invalid clip w');
  }
  return Object.freeze({
    position,
    sourceNdc: Object.freeze([source.x / source.w, source.y / source.w]) as readonly [number, number],
  });
}

interface GhostUniforms {
  readonly ghostViewWorld: THREE.IUniform<THREE.Matrix4>;
  readonly ghostViewWorldInverse: THREE.IUniform<THREE.Matrix4>;
  readonly ghostProjection: THREE.IUniform<THREE.Matrix4>;
  readonly anchorDepth: THREE.IUniform<number>;
  readonly depthRetention: THREE.IUniform<number>;
  readonly projectionKind: THREE.IUniform<number>;
  readonly plateMapping: THREE.IUniform<number>;
  readonly coverageTexture: THREE.IUniform<THREE.Texture>;
  readonly depthTexture: THREE.IUniform<THREE.Texture>;
  readonly textureTexelSize: THREE.IUniform<THREE.Vector2>;
  readonly captureNear: THREE.IUniform<number>;
  readonly captureDepthRange: THREE.IUniform<number>;
  readonly shellMode: THREE.IUniform<number>;
  readonly shellDepthEpsilon: THREE.IUniform<number>;
  readonly shellDepthQuantizationHalfStep: THREE.IUniform<number>;
}

/** Owns one frozen cloned appearance hierarchy and its ghost-only materials. */
export class GhostPlatePresentation {
  readonly object = new THREE.Group();
  readonly #appearanceRoot: THREE.Object3D;
  readonly #ghostCameraLocal = new THREE.Matrix4();
  readonly #ghostProjection: THREE.Matrix4;
  readonly #materials: THREE.MeshBasicMaterial[] = [];
  readonly #ownedGeometries: readonly THREE.BufferGeometry[];
  readonly #skeletons = new Set<THREE.Skeleton>();
  readonly #uniforms: GhostUniforms;
  readonly #projectionKind: 'perspective' | 'orthographic';
  readonly #captureBasis: GhostPlateReadout['captureBasis'];
  readonly #depthMinimum: number;
  readonly #depthMaximum: number;
  readonly #shellDepthQuantizationStep: number;
  #sourceViewBasis: GhostPlateReadout['sourceViewBasis'];
  #angularOffsetDegrees: number | null = null;
  #config: GhostPlateConfig;
  #disposed = false;
  #meshCount = 0;

  constructor(snapshot: GhostPlateCaptureSnapshot) {
    finiteMatrix(snapshot.ghostCameraWorld, 'ghost camera world');
    finiteMatrix(snapshot.ghostProjection, 'ghost projection');
    if (Math.abs(snapshot.ghostCameraWorld.determinant()) <= Number.EPSILON) {
      throw new RangeError('ghost camera world matrix must be invertible');
    }
    if (snapshot.bounds.isEmpty()) throw new RangeError('ghost source bounds must not be empty');
    finiteTuple(snapshot.transform.position, 'ghost transform position');
    positive(snapshot.transform.width, 'ghost transform width');
    positive(snapshot.transform.height, 'ghost transform height');
    positive(snapshot.textureWidth, 'ghost capture texture width');
    positive(snapshot.textureHeight, 'ghost capture texture height');
    if (!Number.isFinite(snapshot.captureNear) || !Number.isFinite(snapshot.captureFar)
      || snapshot.captureNear < 0 || snapshot.captureFar <= snapshot.captureNear) {
      throw new RangeError('ghost capture depth range must be finite and increasing');
    }
    this.#config = validatedConfig(snapshot.config);
    this.#appearanceRoot = snapshot.appearanceRoot;
    this.#ownedGeometries = snapshot.ownedGeometries ?? [];
    this.#projectionKind = snapshot.projectionKind;
    this.#captureBasis = captureBasis(snapshot.ghostCameraWorld);
    this.#sourceViewBasis = this.#captureBasis;
    this.#ghostProjection = snapshot.ghostProjection.clone();
    this.#shellDepthQuantizationStep = (snapshot.captureFar - snapshot.captureNear) / 255;

    const ghostView = snapshot.ghostCameraWorld.clone().invert();
    const extents = projectedBounds(snapshot.bounds, ghostView);
    this.#depthMinimum = extents.depthMinimum;
    this.#depthMaximum = extents.depthMaximum;
    const scale = Math.min(
      snapshot.transform.width / extents.width,
      snapshot.transform.height / extents.height,
    );
    const center = snapshot.bounds.getCenter(new THREE.Vector3());
    const sourceToDisplay = new THREE.Matrix4()
      .makeScale(scale, scale, scale)
      .multiply(new THREE.Matrix4().makeTranslation(-center.x, -center.y, -center.z));
    this.#appearanceRoot.updateWorldMatrix(true, true);
    this.#appearanceRoot.matrix.premultiply(sourceToDisplay);
    this.#appearanceRoot.matrixAutoUpdate = false;
    this.#appearanceRoot.visible = true;
    this.#ghostCameraLocal.multiplyMatrices(sourceToDisplay, snapshot.ghostCameraWorld);

    this.#uniforms = {
      ghostViewWorld: { value: new THREE.Matrix4() },
      ghostViewWorldInverse: { value: new THREE.Matrix4() },
      ghostProjection: { value: this.#ghostProjection },
      anchorDepth: { value: this.#anchorDepth() },
      depthRetention: { value: this.#config.depthRetention },
      projectionKind: { value: this.#projectionKind === 'perspective' ? 0 : 1 },
      plateMapping: { value: this.#config.plateMapping === 'plate-locked' ? 0 : 1 },
      coverageTexture: { value: snapshot.coverageTexture },
      depthTexture: { value: snapshot.depthTexture },
      textureTexelSize: { value: new THREE.Vector2(1 / snapshot.textureWidth, 1 / snapshot.textureHeight) },
      captureNear: { value: snapshot.captureNear },
      captureDepthRange: { value: snapshot.captureFar - snapshot.captureNear },
      shellMode: { value: shellModeUniform(this.#config.shellMode) },
      shellDepthEpsilon: { value: this.#config.shellDepthEpsilon },
      shellDepthQuantizationHalfStep: { value: this.#shellDepthQuantizationStep * 0.5 },
    };

    validateAppearanceHierarchy(this.#appearanceRoot);
    this.#replaceMaterials(snapshot.colorTexture);
    if (this.#meshCount === 0) throw new RangeError('ghost appearance hierarchy has no mesh');
    this.object.name = 'ghost-plate';
    this.object.position.set(...snapshot.transform.position);
    this.object.add(this.#appearanceRoot);
    this.prepare(new THREE.PerspectiveCamera());
    this.#angularOffsetDegrees = null;
  }

  configure(patch: Partial<GhostPlateConfig>): GhostPlateReadout {
    this.#assertLive();
    rejectUnknownKeys(patch);
    this.#config = validatedConfig({ ...this.#config, ...patch });
    this.#uniforms.anchorDepth.value = this.#anchorDepth();
    this.#uniforms.depthRetention.value = this.#config.depthRetention;
    this.#uniforms.plateMapping.value = this.#config.plateMapping === 'plate-locked' ? 0 : 1;
    this.#uniforms.shellMode.value = shellModeUniform(this.#config.shellMode);
    this.#uniforms.shellDepthEpsilon.value = this.#config.shellDepthEpsilon;
    return this.readout();
  }

  setVisible(visible: boolean): void {
    this.#assertLive();
    this.object.visible = visible;
  }

  prepare(realCamera: THREE.Camera): void {
    if (this.#disposed) return;
    this.object.updateWorldMatrix(true, true);
    realCamera.updateWorldMatrix(true, false);
    const ghostCameraWorld = this.object.matrixWorld.clone().multiply(this.#ghostCameraLocal);
    this.#sourceViewBasis = captureBasis(ghostCameraWorld);
    this.#uniforms.ghostViewWorldInverse.value.copy(ghostCameraWorld);
    this.#uniforms.ghostViewWorld.value.copy(ghostCameraWorld).invert();

    const center = this.object.getWorldPosition(new THREE.Vector3());
    const ghostPosition = new THREE.Vector3().setFromMatrixPosition(ghostCameraWorld);
    const realPosition = realCamera.getWorldPosition(new THREE.Vector3());
    const sourceDirection = ghostPosition.sub(center);
    const viewerDirection = realPosition.sub(center);
    this.#angularOffsetDegrees = sourceDirection.lengthSq() < 1e-10 || viewerDirection.lengthSq() < 1e-10
      ? null
      : THREE.MathUtils.radToDeg(sourceDirection.angleTo(viewerDirection));
  }

  readout(): GhostPlateReadout {
    return Object.freeze({
      schemaVersion: 1,
      enabled: !this.#disposed,
      fallbackActive: false,
      fallbackReason: null,
      matchedPose: true,
      projection: this.#projectionKind,
      captureBasis: this.#captureBasis,
      sourceViewBasis: this.#sourceViewBasis,
      depthRetention: this.#config.depthRetention,
      anchorPolicy: this.#config.anchorPolicy,
      anchorValue: this.#config.anchorValue,
      anchorDepth: this.#anchorDepth(),
      plateMapping: this.#config.plateMapping,
      shellMode: this.#config.shellMode,
      shellDepthEpsilon: this.#config.shellDepthEpsilon,
      shellDepthQuantizationStep: this.#shellDepthQuantizationStep,
      shellEffectiveDepthEpsilon: this.#config.shellDepthEpsilon + this.#shellDepthQuantizationStep * 0.5,
      rejectedFragmentRatio: Object.freeze({ status: 'unavailable', value: null }),
      repairedBoundaryRatio: Object.freeze({ status: 'unavailable', value: null }),
      angularOffsetDegrees: this.#angularOffsetDegrees,
      sectorCount: 1,
      selectedSector: 0,
      pendingSector: null,
      previousSector: null,
      localAzimuthDegrees: null,
      sectorHysteresisDegrees: 0,
      residentSectorCount: this.#disposed ? 0 : 1,
      currentResourceResident: !this.#disposed,
      previousResourceResident: false,
      preparationCpuMilliseconds: null,
      invalidationReason: null,
      expectedDrawCalls: this.#disposed ? 0 : this.#materials.length,
      meshCount: this.#disposed ? 0 : this.#meshCount,
      materialResourceCount: this.#disposed ? 0 : this.#materials.length,
      borrowedTextureCount: this.#disposed ? 0 : 3,
      disposed: this.#disposed,
      limitations: LIMITATIONS,
    });
  }

  dispose(): void {
    if (this.#disposed) return;
    this.object.remove(this.#appearanceRoot);
    for (const material of this.#materials) material.dispose();
    for (const geometry of this.#ownedGeometries) geometry.dispose();
    for (const skeleton of this.#skeletons) skeleton.dispose();
    this.#materials.length = 0;
    this.#skeletons.clear();
    this.#disposed = true;
    this.#angularOffsetDegrees = null;
  }

  #replaceMaterials(colorTexture: THREE.Texture): void {
    this.#appearanceRoot.traverse((object) => {
      if (object instanceof THREE.SkinnedMesh) this.#skeletons.add(object.skeleton);
      if (!(object instanceof THREE.Mesh)) return;
      const sourceMaterials = Array.isArray(object.material) ? object.material : [object.material];
      const replacements = sourceMaterials.map((source) => this.#material(colorTexture, source));
      object.material = Array.isArray(object.material) ? replacements : replacements[0]!;
      object.castShadow = false;
      object.receiveShadow = false;
      object.frustumCulled = false;
      this.#meshCount += 1;
    });
  }

  #material(colorTexture: THREE.Texture, source: THREE.Material): THREE.MeshBasicMaterial {
    const material = new THREE.MeshBasicMaterial({
      name: 'ghost-plate-material',
      color: 0xffffff,
      map: colorTexture,
      side: THREE.DoubleSide,
      transparent: false,
      depthTest: true,
      depthWrite: true,
      fog: true,
      toneMapped: source.toneMapped,
    });
    material.clippingPlanes = source.clippingPlanes?.map((plane) => plane.clone()) ?? null;
    material.clipIntersection = source.clipIntersection;
    material.clipShadows = source.clipShadows;
    material.onBeforeCompile = (shader) => patchShader(shader, this.#uniforms);
    material.customProgramCacheKey = () => 'rusty-engine-ghost-plate-v7-hard-snap';
    this.#materials.push(material);
    return material;
  }

  #anchorDepth(): number {
    if (this.#config.anchorPolicy === 'bounds-center') {
      return (this.#depthMinimum + this.#depthMaximum) * 0.5;
    }
    return THREE.MathUtils.lerp(
      this.#depthMinimum,
      this.#depthMaximum,
      this.#config.anchorValue,
    );
  }

  #assertLive(): void {
    if (this.#disposed) throw new Error('ghost plate presentation is disposed');
  }
}

/** Owns one exact-pose plate bank and exposes exactly one hard-selected sector. */
export class GhostPlateDirectionalPresentation {
  readonly object = new THREE.Group();
  readonly #plates: readonly GhostPlatePresentation[];
  readonly #baseAzimuthDegrees: number;
  readonly #preparationCpuMilliseconds: number | null;
  #config: GhostPlateConfig;
  #selectedSector = 0;
  #localAzimuthDegrees: number | null = null;
  #fallbackReason: GhostPlateReadout['fallbackReason'] = null;
  #invalidationReason: string | null = null;
  #disposed = false;

  constructor(options: {
    readonly plates: readonly GhostPlatePresentation[];
    readonly config: GhostPlateConfig;
    readonly baseAzimuthDegrees: number;
    readonly preparationCpuMilliseconds: number | null;
  }) {
    this.#config = validatedConfig(options.config);
    if (options.plates.length !== this.#config.sectorCount) {
      throw new RangeError('ghost plate bank must match the configured sector count');
    }
    if (!Number.isFinite(options.baseAzimuthDegrees)) {
      throw new TypeError('ghost base azimuth must be finite');
    }
    this.#plates = Object.freeze([...options.plates]);
    this.#baseAzimuthDegrees = normalizeDegrees(options.baseAzimuthDegrees);
    this.#preparationCpuMilliseconds = options.preparationCpuMilliseconds;
    this.object.name = 'ghost-plate-directional';
    for (const [index, plate] of this.#plates.entries()) {
      this.object.add(plate.object);
      plate.setVisible(index === 0);
    }
  }

  configure(patch: Partial<GhostPlateConfig>): GhostPlateReadout {
    this.#assertLive();
    const next = validatedConfig({ ...this.#config, ...patch });
    if (next.sectorCount !== this.#plates.length) {
      throw new RangeError('changing ghost sector count requires an atomic source replacement');
    }
    this.#config = next;
    for (const plate of this.#plates) plate.configure(next);
    return this.readout();
  }

  prepare(realCamera: THREE.Camera, now = nowMilliseconds()): void {
    if (this.#disposed) return;
    try {
      for (const plate of this.#plates) plate.prepare(realCamera);
      this.#localAzimuthDegrees = actorRelativeAzimuth(realCamera, this.#plates[this.#selectedSector]!.object);
      const nextSector = selectGhostPlateSector(
        this.#localAzimuthDegrees,
        this.#baseAzimuthDegrees,
        this.#config.sectorCount,
        this.#selectedSector,
        this.#config.sectorHysteresisDegrees,
      );
      if (nextSector !== this.#selectedSector) this.#selectSector(nextSector);
    } catch (cause) {
      this.#fallbackReason = 'sector-selection-failed';
      this.#invalidationReason = cause instanceof Error ? cause.message : String(cause);
      this.#selectSector(this.#selectedSector);
    }
  }

  readout(): GhostPlateReadout {
    const current = this.#plates[this.#selectedSector]!.readout();
    return Object.freeze({
      ...current,
      fallbackActive: this.#fallbackReason !== null,
      fallbackReason: this.#fallbackReason,
      angularOffsetDegrees: this.#localAzimuthDegrees === null
        ? null
        : Math.abs(signedAngularDifference(
            this.#localAzimuthDegrees,
            this.#baseAzimuthDegrees + this.#selectedSector * 360 / this.#config.sectorCount,
          )),
      sectorCount: this.#config.sectorCount,
      selectedSector: this.#selectedSector,
      pendingSector: null,
      previousSector: null,
      localAzimuthDegrees: this.#localAzimuthDegrees,
      sectorHysteresisDegrees: this.#config.sectorHysteresisDegrees,
      residentSectorCount: this.#disposed ? 0 : this.#plates.length,
      currentResourceResident: !this.#disposed,
      previousResourceResident: false,
      preparationCpuMilliseconds: this.#preparationCpuMilliseconds,
      invalidationReason: this.#invalidationReason,
      expectedDrawCalls: current.expectedDrawCalls,
      meshCount: this.#plates.reduce((total, plate) => total + plate.readout().meshCount, 0),
      materialResourceCount: this.#plates.reduce(
        (total, plate) => total + plate.readout().materialResourceCount,
        0,
      ),
      borrowedTextureCount: this.#plates.reduce(
        (total, plate) => total + plate.readout().borrowedTextureCount,
        0,
      ),
      disposed: this.#disposed,
      limitations: Object.freeze(current.limitations.filter((value) => value !== 'single-capture-view')),
    });
  }

  dispose(): void {
    if (this.#disposed) return;
    for (const plate of this.#plates) {
      this.object.remove(plate.object);
      plate.dispose();
    }
    this.#disposed = true;
  }

  #selectSector(sector: number): void {
    this.#selectedSector = sector;
    for (const [index, plate] of this.#plates.entries()) {
      plate.setVisible(index === sector);
    }
  }

  #assertLive(): void {
    if (this.#disposed) throw new Error('directional ghost plate presentation is disposed');
  }
}

/** Deterministic nearest-sector selection with a bounded hold zone around the live sector. */
export function selectGhostPlateSector(
  localAzimuthDegrees: number,
  baseAzimuthDegrees: number,
  sectorCount: GhostPlateSectorCount,
  currentSector: number,
  hysteresisDegrees: number,
): number {
  if (![1, 4, 8, 16].includes(sectorCount)) throw new RangeError('unsupported ghost sector count');
  if (!Number.isInteger(currentSector) || currentSector < 0 || currentSector >= sectorCount) {
    throw new RangeError('current ghost sector is out of range');
  }
  bounded(hysteresisDegrees, 0, 22.5, 'ghost sector hysteresis');
  if (!Number.isFinite(localAzimuthDegrees) || !Number.isFinite(baseAzimuthDegrees)) {
    throw new TypeError('ghost sector azimuths must be finite');
  }
  if (sectorCount === 1) return 0;
  const width = 360 / sectorCount;
  const currentCenter = baseAzimuthDegrees + currentSector * width;
  if (Math.abs(signedAngularDifference(localAzimuthDegrees, currentCenter))
    <= width * 0.5 + hysteresisDegrees) return currentSector;
  return ((Math.round(signedAngularDifference(localAzimuthDegrees, baseAzimuthDegrees) / width)
    % sectorCount) + sectorCount) % sectorCount;
}

function actorRelativeAzimuth(camera: THREE.Camera, object: THREE.Object3D): number {
  camera.updateWorldMatrix(true, false);
  object.updateWorldMatrix(true, false);
  const center = object.getWorldPosition(new THREE.Vector3());
  const direction = camera.getWorldPosition(new THREE.Vector3()).sub(center);
  if (direction.lengthSq() < 1e-10) throw new RangeError('viewer is coincident with ghost plate center');
  const orientation = object.getWorldQuaternion(new THREE.Quaternion()).invert();
  direction.applyQuaternion(orientation);
  return normalizeDegrees(THREE.MathUtils.radToDeg(Math.atan2(direction.x, direction.z)));
}

function signedAngularDifference(value: number, reference: number): number {
  return ((value - reference + 540) % 360) - 180;
}

function normalizeDegrees(value: number): number {
  return ((value % 360) + 360) % 360;
}

function nowMilliseconds(): number {
  return globalThis.performance?.now() ?? Date.now();
}

function validateAppearanceHierarchy(root: THREE.Object3D): void {
  root.traverse((object) => {
    if (object instanceof THREE.InstancedMesh || object instanceof THREE.BatchedMesh) {
      throw new TypeError('ghost appearance hierarchy requires ordinary meshes');
    }
    if (object instanceof THREE.Line || object instanceof THREE.Points || object instanceof THREE.Sprite) {
      throw new TypeError('ghost appearance hierarchy contains an unsupported renderable');
    }
  });
}

function patchShader(shader: THREE.WebGLProgramParametersWithUniforms, uniforms: GhostUniforms): void {
  Object.assign(shader.uniforms, uniforms);
  const vertexDeclarations = `
    uniform mat4 ghostViewWorld;
    uniform mat4 ghostViewWorldInverse;
    uniform mat4 ghostProjection;
    uniform float anchorDepth;
    uniform float depthRetention;
    uniform float projectionKind;
    varying vec3 ghostProjectiveUv;
    varying vec3 ghostPlateLockedUv;
    varying float ghostOriginalDepth;
  `;
  shader.vertexShader = shader.vertexShader
    .replace('void main() {', `${vertexDeclarations}\nvoid main() {`)
    .replace('#include <project_vertex>', `
      vec4 ghostOriginalWorld = modelMatrix * vec4(transformed, 1.0);
      vec4 ghostOriginalCamera = ghostViewWorld * ghostOriginalWorld;
      float ghostDepth = max(-ghostOriginalCamera.z, ${MIN_GHOST_DEPTH.toFixed(4)});
      float ghostWarpedDepth = max(
        ${MIN_GHOST_DEPTH.toFixed(4)},
        anchorDepth + depthRetention * (ghostDepth - anchorDepth)
      );
      float ghostRayScale = projectionKind < 0.5 ? ghostWarpedDepth / ghostDepth : 1.0;
      vec4 ghostWarpedCamera = vec4(
        ghostOriginalCamera.xy * ghostRayScale,
        -ghostWarpedDepth,
        1.0
      );
      vec4 ghostWarpedWorld = ghostViewWorldInverse * ghostWarpedCamera;
      vec4 mvPosition = viewMatrix * ghostWarpedWorld;
      gl_Position = projectionMatrix * mvPosition;
      vec4 ghostSourceClip = ghostProjection * ghostOriginalCamera;
      vec2 ghostSourceUv = ghostSourceClip.xy / ghostSourceClip.w * 0.5 + 0.5;
      ghostProjectiveUv = vec3(ghostSourceClip.xy * 0.5 + ghostSourceClip.w * 0.5, ghostSourceClip.w);
      ghostPlateLockedUv = vec3(ghostSourceUv * gl_Position.w, gl_Position.w);
      ghostOriginalDepth = ghostDepth;
    `);
  const fragmentDeclarations = `
    uniform sampler2D coverageTexture;
    uniform sampler2D depthTexture;
    uniform float plateMapping;
    uniform vec2 textureTexelSize;
    uniform float captureNear;
    uniform float captureDepthRange;
    uniform float shellMode;
    uniform float shellDepthEpsilon;
    uniform float shellDepthQuantizationHalfStep;
    varying vec3 ghostProjectiveUv;
    varying vec3 ghostPlateLockedUv;
    varying float ghostOriginalDepth;

    bool ghostShellSampleAgrees(vec2 uv) {
      if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) return false;
      float coverage = texture2D(coverageTexture, uv).r;
      if (coverage < 0.5) return false;
      float sampledDepth = captureNear + texture2D(depthTexture, uv).r * captureDepthRange;
      return abs(ghostOriginalDepth - sampledDepth)
        <= shellDepthEpsilon + shellDepthQuantizationHalfStep;
    }

  `;
  shader.fragmentShader = shader.fragmentShader
    .replace('void main() {', `${fragmentDeclarations}\nvoid main() {`)
    .replace('#include <map_fragment>', `
      vec2 ghostUv = plateMapping < 0.5
        ? ghostPlateLockedUv.xy / ghostPlateLockedUv.z
        : ghostProjectiveUv.xy / ghostProjectiveUv.z;
      if (ghostUv.x < 0.0 || ghostUv.x > 1.0 || ghostUv.y < 0.0 || ghostUv.y > 1.0) discard;
      float ghostCoverage = texture2D(coverageTexture, ghostUv).r;
      vec4 ghostPlateColor = texture2D(map, ghostUv);
      if (ghostCoverage < 0.5 || ghostPlateColor.a < 0.01) discard;
      if (shellMode > 0.5) {
        bool ghostShellAccepted = ghostShellSampleAgrees(ghostUv);
        if (!ghostShellAccepted && shellMode > 1.5) {
          ghostShellAccepted = ghostShellSampleAgrees(ghostUv + vec2(textureTexelSize.x, 0.0))
            || ghostShellSampleAgrees(ghostUv - vec2(textureTexelSize.x, 0.0))
            || ghostShellSampleAgrees(ghostUv + vec2(0.0, textureTexelSize.y))
            || ghostShellSampleAgrees(ghostUv - vec2(0.0, textureTexelSize.y));
        }
        if (!ghostShellAccepted) discard;
      }
      diffuseColor *= vec4(ghostPlateColor.rgb, 1.0);
    `);
}

function projectedBounds(bounds: THREE.Box3, ghostView: THREE.Matrix4): {
  readonly width: number;
  readonly height: number;
  readonly depthMinimum: number;
  readonly depthMaximum: number;
} {
  let minimumX = Number.POSITIVE_INFINITY;
  let maximumX = Number.NEGATIVE_INFINITY;
  let minimumY = Number.POSITIVE_INFINITY;
  let maximumY = Number.NEGATIVE_INFINITY;
  let depthMinimum = Number.POSITIVE_INFINITY;
  let depthMaximum = Number.NEGATIVE_INFINITY;
  for (const x of [bounds.min.x, bounds.max.x]) {
    for (const y of [bounds.min.y, bounds.max.y]) {
      for (const z of [bounds.min.z, bounds.max.z]) {
        const point = new THREE.Vector3(x, y, z).applyMatrix4(ghostView);
        const depth = -point.z;
        if (!Number.isFinite(depth) || depth <= MIN_GHOST_DEPTH) {
          throw new RangeError('ghost source bounds must be in front of the capture camera');
        }
        minimumX = Math.min(minimumX, point.x);
        maximumX = Math.max(maximumX, point.x);
        minimumY = Math.min(minimumY, point.y);
        maximumY = Math.max(maximumY, point.y);
        depthMinimum = Math.min(depthMinimum, depth);
        depthMaximum = Math.max(depthMaximum, depth);
      }
    }
  }
  const width = maximumX - minimumX;
  const height = maximumY - minimumY;
  if (width <= MIN_GHOST_DEPTH || height <= MIN_GHOST_DEPTH) {
    throw new RangeError('ghost source projected bounds must have nonzero width and height');
  }
  return { width, height, depthMinimum, depthMaximum };
}

function captureBasis(cameraWorld: THREE.Matrix4): GhostPlateReadout['captureBasis'] {
  const position = new THREE.Vector3().setFromMatrixPosition(cameraWorld);
  const right = new THREE.Vector3().setFromMatrixColumn(cameraWorld, 0).normalize();
  const up = new THREE.Vector3().setFromMatrixColumn(cameraWorld, 1).normalize();
  const forward = new THREE.Vector3().setFromMatrixColumn(cameraWorld, 2).normalize().negate();
  const tuple = (value: THREE.Vector3): readonly [number, number, number] =>
    Object.freeze(value.toArray()) as unknown as readonly [number, number, number];
  return Object.freeze({
    position: tuple(position),
    right: tuple(right),
    up: tuple(up),
    forward: tuple(forward),
  });
}

function validatedConfig(config: GhostPlateConfig): GhostPlateConfig {
  rejectUnknownKeys(config);
  bounded(config.depthRetention, 0.02, 1, 'ghost depth retention');
  if (config.anchorPolicy !== 'bounds-center' && config.anchorPolicy !== 'bounds-normalized') {
    throw new TypeError(`unsupported ghost anchor policy ${String(config.anchorPolicy)}`);
  }
  bounded(config.anchorValue, 0, 1, 'ghost anchor value');
  if (config.plateMapping !== 'plate-locked' && config.plateMapping !== 'projective-surface') {
    throw new TypeError(`unsupported ghost plate mapping ${String(config.plateMapping)}`);
  }
  if (config.shellMode !== 'whole-mesh'
    && config.shellMode !== 'strict-source'
    && config.shellMode !== 'repaired-source') {
    throw new TypeError(`unsupported ghost shell mode ${String(config.shellMode)}`);
  }
  bounded(config.shellDepthEpsilon, 0, 2, 'ghost shell depth epsilon');
  if (![1, 4, 8, 16].includes(config.sectorCount)) {
    throw new RangeError('ghost sector count must be 1, 4, 8, or 16');
  }
  bounded(config.sectorHysteresisDegrees, 0, 22.5, 'ghost sector hysteresis');
  return Object.freeze({ ...config });
}

function rejectUnknownKeys(config: Partial<GhostPlateConfig>): void {
  const keys = new Set([
    'depthRetention', 'anchorPolicy', 'anchorValue', 'plateMapping', 'shellMode', 'shellDepthEpsilon',
    'sectorCount', 'sectorHysteresisDegrees',
  ]);
  for (const key of Object.keys(config)) {
    if (!keys.has(key)) throw new TypeError(`unknown ghost plate config field ${key}`);
  }
}

function shellModeUniform(mode: GhostPlateShellMode): number {
  return mode === 'whole-mesh' ? 0 : mode === 'strict-source' ? 1 : 2;
}

function finiteMatrix(matrix: THREE.Matrix4, name: string): void {
  if (matrix.elements.some((component) => !Number.isFinite(component))) {
    throw new TypeError(`${name} must contain only finite values`);
  }
}

function finiteTuple(value: readonly number[], name: string): void {
  if (value.length !== 3 || value.some((component) => !Number.isFinite(component))) {
    throw new TypeError(`${name} must contain three finite values`);
  }
}

function positive(value: number, name: string): void {
  if (!Number.isFinite(value) || value <= 0) throw new RangeError(`${name} must be positive and finite`);
}

function bounded(value: number, minimum: number, maximum: number, name: string): void {
  if (!Number.isFinite(value) || value < minimum || value > maximum) {
    throw new RangeError(`${name} must be finite from ${String(minimum)} to ${String(maximum)}`);
  }
}
