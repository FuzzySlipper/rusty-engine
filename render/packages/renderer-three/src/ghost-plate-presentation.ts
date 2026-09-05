import * as THREE from 'three';
import * as SkeletonUtils from 'three/examples/jsm/utils/SkeletonUtils.js';
import type { GhostPlateCaptureSettings, GhostPlateConfig, GhostPlateDescriptor, GhostPlatePatch, RenderFrameDiff, RenderHandle } from '@rusty-engine/render-contracts';
import { GhostPlateRuntimeCapture } from './ghost-plate-capture.js';
import { GhostPlateDirectionalPresentation, GhostPlatePresentation, type GhostPlateConfig as ThreeGhostPlateConfig } from './ghost-plate.js';
import type { ThreeRendererIsolatedCaptureScene } from './three-renderer.js';

export interface RendererThreeGhostPlateBackend {
  readonly scene: THREE.Scene;
  objectFor(handle: RenderHandle): THREE.Object3D | undefined;
  createIsolatedCaptureScene?(frame: RenderFrameDiff): ThreeRendererIsolatedCaptureScene;
}
export interface RendererThreeGhostPlateReceipt { readonly applied: boolean; readonly diagnostics: readonly { readonly code: string; readonly message: string }[]; }
export interface RendererThreeGhostPlateReadout {
  readonly source: number; readonly sourceMatch: boolean; readonly currentSector: number; readonly localAzimuthDegrees: number | null;
  readonly capture: GhostPlateCaptureSettings; readonly config: GhostPlateConfig; readonly fallbackActive: boolean; readonly fallbackReason: string | null;
  readonly limitationMask: number;
  readonly preparationCpuMilliseconds: number | null; readonly captureCpuSubmissionMilliseconds: number | null;
  readonly retainedResourceCounts: { readonly sectors: number; readonly meshes: number; readonly materials: number; readonly borrowedTextures: number; };
  readonly disposed: boolean;
}
interface ActiveGhostPlate { readonly descriptor: GhostPlateDescriptor; readonly presentation: GhostPlateDirectionalPresentation; readonly captures: readonly GhostPlateRuntimeCapture[]; readonly captureCpuSubmissionMilliseconds: number | null; }
const GHOST_CAPTURE_MAX_RETAINED_BYTES = 256 * 1024 * 1024;
const GHOST_CAPTURE_BYTES_PER_PIXEL_PER_SECTOR = 20;
const ghostCaptureBytesByRenderer = new WeakMap<THREE.WebGLRenderer, number>();

/** Dedicated retained realization: frozen source capture, directional bank, and ghost shell only. */
export class RendererThreeGhostPlatePresentation {
  readonly #webgl: THREE.WebGLRenderer;
  readonly #backend: RendererThreeGhostPlateBackend;
  readonly #invalidate: () => void;
  readonly #onDispose: () => void;
  #active: ActiveGhostPlate | null = null;
  #disposed = false;

  constructor(options: { readonly webgl: THREE.WebGLRenderer; readonly backend: RendererThreeGhostPlateBackend; readonly invalidate: () => void; readonly onDispose: () => void; }) {
    this.#webgl = options.webgl; this.#backend = options.backend; this.#invalidate = options.invalidate; this.#onDispose = options.onDispose;
  }
  create(descriptor: GhostPlateDescriptor): RendererThreeGhostPlateReceipt {
    if (this.#disposed) return rejected('disposed', 'ghost plate presentation is disposed');
    if (this.#active !== null) return rejected('duplicate', 'ghost plate presentation is already active');
    return this.#replace(descriptor);
  }
  update(patch: GhostPlatePatch): RendererThreeGhostPlateReceipt {
    const current = this.#active?.descriptor;
    if (current === undefined) return rejected('inactive', 'ghost plate presentation is not active');
    return this.#replace(Object.freeze({ ...current, ...(patch.placement === undefined ? {} : { placement: patch.placement }), ...(patch.config === undefined ? {} : { config: patch.config }) }));
  }
  recapture(capture: GhostPlateCaptureSettings | null, capturedScene?: RenderFrameDiff): RendererThreeGhostPlateReceipt {
    const current = this.#active?.descriptor;
    if (current === undefined) return rejected('inactive', 'ghost plate presentation is not active');
    return this.#replace(Object.freeze({
      ...current,
      ...(capture === null ? {} : { capture }),
      ...(capturedScene === undefined ? {} : { capturedScene }),
    }));
  }
  /** Called by the browser surface's ordinary render lifecycle. */
  prepare(camera: THREE.Camera, view: object = camera): void { this.#active?.presentation.prepare(camera, undefined, view); }
  readout(): RendererThreeGhostPlateReadout {
    const active = this.#active; const ghost = active?.presentation.readout();
    return Object.freeze({
      source: active?.descriptor.source ?? 0, sourceMatch: active !== null && this.#backend.objectFor(active.descriptor.source) !== undefined,
      currentSector: ghost?.selectedSector ?? 0, localAzimuthDegrees: ghost?.localAzimuthDegrees ?? null,
      capture: active?.descriptor.capture ?? emptyCapture(), config: active?.descriptor.config ?? emptyConfig(),
      fallbackActive: ghost?.fallbackActive ?? false, fallbackReason: ghost?.fallbackReason ?? null,
      limitationMask: ghostPlateLimitationMask(ghost?.limitations ?? []),
      preparationCpuMilliseconds: ghost?.preparationCpuMilliseconds ?? null, captureCpuSubmissionMilliseconds: active?.captureCpuSubmissionMilliseconds ?? null,
      retainedResourceCounts: Object.freeze({ sectors: ghost?.residentSectorCount ?? 0, meshes: ghost?.meshCount ?? 0, materials: ghost?.materialResourceCount ?? 0, borrowedTextures: ghost?.borrowedTextureCount ?? 0 }),
      disposed: this.#disposed,
    });
  }
  destroy(): RendererThreeGhostPlateReceipt {
    if (this.#disposed) return rejected('disposed', 'ghost plate presentation is disposed');
    if (this.#active === null) return rejected('inactive', 'ghost plate presentation is not active');
    this.#disposeActive(this.#active); this.#active = null; this.#invalidate(); return applied();
  }
  dispose(): void { if (this.#disposed) return; if (this.#active !== null) this.#disposeActive(this.#active); this.#active = null; this.#disposed = true; this.#invalidate(); this.#onDispose(); }

  #replace(descriptor: GhostPlateDescriptor): RendererThreeGhostPlateReceipt {
    const candidateBytes = captureBytes(descriptor); const currentBytes = this.#active === null ? 0 : captureBytes(this.#active.descriptor);
    const retainedBytes = ghostCaptureBytesByRenderer.get(this.#webgl) ?? 0;
    // Current + candidate is the actual replacement peak; do not dispose the
    // valid plate merely to make an oversized replacement fit.
    if (retainedBytes + candidateBytes > GHOST_CAPTURE_MAX_RETAINED_BYTES) return rejected('captureFailed', 'ghost capture aggregate budget exceeded');
    let candidate: ActiveGhostPlate | null = null;
    try { candidate = this.#build(descriptor); this.#backend.scene.add(candidate.presentation.object); }
    catch (cause) { if (candidate !== null) this.#disposeActive(candidate, false); return rejected('captureFailed', cause instanceof Error ? cause.message : String(cause)); }
    const previous = this.#active; if (previous !== null) this.#disposeActive(previous); this.#active = candidate; ghostCaptureBytesByRenderer.set(this.#webgl, retainedBytes + candidateBytes - currentBytes); this.#invalidate(); return applied();
  }
  #build(descriptor: GhostPlateDescriptor): ActiveGhostPlate {
    const captureSource = this.#captureSource(descriptor);
    let appearanceRoot: THREE.Object3D | null = null; let frozenGeometries: readonly THREE.BufferGeometry[] = [];
    const captures: GhostPlateRuntimeCapture[] = []; const plates: GhostPlatePresentation[] = []; let directional: GhostPlateDirectionalPresentation | null = null;
    try {
      const retained = captureSource.object;
      retained.updateWorldMatrix(true, true);
      appearanceRoot = SkeletonUtils.clone(retained);
      removeClonedLights(appearanceRoot); appearanceRoot.matrix.copy(retained.matrixWorld); appearanceRoot.matrixAutoUpdate = false; appearanceRoot.visible = true;
      appearanceRoot.traverse((object) => { if (object instanceof THREE.Mesh) object.layers.enable(0); }); appearanceRoot.updateWorldMatrix(true, true);
      const frozen = freezeSkinnedMeshes(appearanceRoot); appearanceRoot = frozen.root; frozenGeometries = frozen.ownedGeometries; appearanceRoot.updateWorldMatrix(true, true);
      const bounds = new THREE.Box3().setFromObject(appearanceRoot, true); if (bounds.isEmpty()) throw new Error('retained source bounds are empty');
      const center = bounds.getCenter(new THREE.Vector3()); const size = bounds.getSize(new THREE.Vector3()); const config = threeConfig(descriptor.config); const started = nowMilliseconds();
      let totalCaptureMilliseconds = 0; let captureTimesPresent = true;
      for (let sector = 0; sector < config.sectorCount; sector += 1) {
        const sectorAppearance = cloneFrozenAppearance(appearanceRoot); const scene = new THREE.Scene(); scene.add(sectorAppearance.root);
        const settings = Object.freeze({ ...descriptor.capture, azimuthDegrees: normalizedAzimuth(descriptor.capture.azimuthDegrees + sector * 360 / config.sectorCount) });
        const camera = captureCamera(settings, center, size); const releaseLighting = settings.lighting.mode === 'scene' ? cloneSceneLights(captureSource.scene, scene) : addStudioRig(scene, camera, center, size, settings.lighting);
        const capture = new GhostPlateRuntimeCapture(this.#webgl); captures.push(capture);
        const receipt = capture.capture({ scene, camera, width: settings.resolution, height: settings.resolution, bounds }); releaseLighting(); scene.remove(sectorAppearance.root);
        if (!receipt.applied || receipt.frame === null) { for (const geometry of sectorAppearance.ownedGeometries) geometry.dispose(); throw new Error(receipt.diagnostics[0]?.message ?? 'runtime capture failed'); }
        if (receipt.readout.cpuSubmissionMilliseconds === null) captureTimesPresent = false; else totalCaptureMilliseconds += receipt.readout.cpuSubmissionMilliseconds;
        plates.push(new GhostPlatePresentation({ appearanceRoot: sectorAppearance.root, ownedGeometries: sectorAppearance.ownedGeometries, colorTexture: receipt.frame.descriptor.textures.color, coverageTexture: receipt.frame.descriptor.textures.coverage, depthTexture: receipt.frame.descriptor.textures.depth, textureWidth: receipt.frame.descriptor.width, textureHeight: receipt.frame.descriptor.height, captureNear: receipt.frame.descriptor.depth.near, captureFar: receipt.frame.descriptor.depth.far, projectionKind: 'perspective', ghostCameraWorld: camera.matrixWorld.clone(), ghostProjection: camera.projectionMatrix.clone(), bounds, transform: { position: [0, 0, 0], width: descriptor.placement.width, height: descriptor.placement.height }, config }));
      }
      for (const geometry of frozenGeometries) geometry.dispose(); frozenGeometries = []; disposeClonedSkeletons(appearanceRoot);
      directional = new GhostPlateDirectionalPresentation({ plates, config, baseAzimuthDegrees: descriptor.capture.azimuthDegrees, preparationCpuMilliseconds: nowMilliseconds() - started });
      applyTransform(directional.object, descriptor.placement.transform);
      return Object.freeze({ descriptor, presentation: directional, captures: Object.freeze(captures), captureCpuSubmissionMilliseconds: captureTimesPresent ? totalCaptureMilliseconds : null });
    } catch (cause) {
      directional?.dispose(); if (directional === null) { for (const plate of plates) plate.dispose(); if (appearanceRoot !== null) disposeClonedSkeletons(appearanceRoot); for (const geometry of frozenGeometries) geometry.dispose(); }
      for (const capture of captures) capture.dispose(); throw cause;
    } finally {
      // The retained ghost bank owns only its capture textures and cloned
      // ghost materials. The temporary captured-scene renderer can therefore
      // release its source geometry, materials, and resource borrows here.
      captureSource.dispose();
    }
  }

  #captureSource(descriptor: GhostPlateDescriptor): { readonly scene: THREE.Scene; readonly object: THREE.Object3D; dispose(): void } {
    if (descriptor.capturedScene === undefined) {
      const object = this.#backend.objectFor(descriptor.source);
      if (object === undefined) throw new Error(`retained handle ${String(descriptor.source)} is unavailable`);
      return Object.freeze({ scene: this.#backend.scene, object, dispose: () => undefined });
    }
    const isolated = this.#backend.createIsolatedCaptureScene?.(descriptor.capturedScene);
    if (isolated === undefined) {
      throw new Error('backend cannot reconstruct an Engine-captured ghost scene');
    }
    const object = isolated.objectFor(descriptor.source);
    const scene = isolated.sceneFor(descriptor.source);
    if (object === undefined || scene === undefined) {
      isolated.dispose();
      throw new Error(`captured ghost scene does not retain handle ${String(descriptor.source)}`);
    }
    return Object.freeze({ scene, object, dispose: () => isolated.dispose() });
  }
  #disposeActive(active: ActiveGhostPlate, releaseAccounting = true): void { active.presentation.object.removeFromParent(); active.presentation.dispose(); for (const capture of active.captures) capture.dispose(); if (releaseAccounting) { const current = ghostCaptureBytesByRenderer.get(this.#webgl) ?? 0; ghostCaptureBytesByRenderer.set(this.#webgl, Math.max(0, current - captureBytes(active.descriptor))); } }
}

function ghostPlateLimitationMask(limitations: readonly string[]): number {
  let mask = 0;
  for (const limitation of limitations) {
    switch (limitation) {
      case 'retained-source-only': mask |= 1; break;
      case 'single-capture-view': mask |= 2; break;
      case 'frozen-appearance-pose': mask |= 4; break;
      case 'whole-hierarchy-relief': mask |= 8; break;
      case 'rgba8-shell-depth': mask |= 16; break;
      case 'fragment-ratios-unavailable-without-readback': mask |= 32; break;
      case 'gpu-time-not-measured': mask |= 64; break;
      default: break;
    }
  }
  return mask;
}

function applied(): RendererThreeGhostPlateReceipt { return Object.freeze({ applied: true, diagnostics: Object.freeze([]) }); }
function rejected(code: string, message: string): RendererThreeGhostPlateReceipt { return Object.freeze({ applied: false, diagnostics: Object.freeze([{ code, message }]) }); }
function threeConfig(config: GhostPlateConfig): ThreeGhostPlateConfig { return Object.freeze({ ...config }); }
function applyTransform(object: THREE.Object3D, transform: GhostPlateDescriptor['placement']['transform']): void { object.position.set(...transform.translation); object.quaternion.set(...transform.rotation).normalize(); object.scale.set(...transform.scale); object.updateWorldMatrix(true, true); }
function normalizedAzimuth(value: number): number { const normalized = ((value + 180) % 360 + 360) % 360 - 180; return normalized === -180 ? 180 : normalized; }
function nowMilliseconds(): number { return globalThis.performance?.now() ?? Date.now(); }

// Mesh.copy also copies geometry: install the baked geometry after copying
// transforms and flags, so capture never falls back to the bind-pose vertices.
function freezeSkinnedMeshes(root: THREE.Object3D): { readonly root: THREE.Object3D; readonly ownedGeometries: readonly THREE.BufferGeometry[] } {
  const skinned: THREE.SkinnedMesh[] = []; const skeletons = new Set<THREE.Skeleton>(); const geometries: THREE.BufferGeometry[] = []; let frozenRoot = root;
  root.traverse((object) => { if (object instanceof THREE.SkinnedMesh) skinned.push(object); });
  try { for (const source of skinned) { const parent = source.parent; skeletons.add(source.skeleton); source.skeleton.update(); const geometry = source.geometry.clone(); geometries.push(geometry); const positions = source.geometry.getAttribute('position'); const frozenPositions = positions.clone(); const vertex = new THREE.Vector3(); for (let index = 0; index < positions.count; index += 1) { source.getVertexPosition(index, vertex); frozenPositions.setXYZ(index, vertex.x, vertex.y, vertex.z); } frozenPositions.needsUpdate = true; geometry.setAttribute('position', frozenPositions); geometry.deleteAttribute('skinIndex'); geometry.deleteAttribute('skinWeight'); geometry.morphAttributes = {}; geometry.morphTargetsRelative = false; geometry.computeBoundingBox(); geometry.computeBoundingSphere(); const frozen = new THREE.Mesh(geometry, source.material); frozen.copy(source, false); frozen.geometry = geometry; for (const child of [...source.children]) frozen.add(child); if (parent === null) frozenRoot = frozen; else { const index = parent.children.indexOf(source); parent.remove(source); parent.add(frozen); parent.children.splice(parent.children.indexOf(frozen), 1); parent.children.splice(index, 0, frozen); } } }
  catch (cause) { for (const geometry of geometries) geometry.dispose(); throw cause; } finally { for (const skeleton of skeletons) skeleton.dispose(); }
  return { root: frozenRoot, ownedGeometries: geometries };
}
function cloneFrozenAppearance(root: THREE.Object3D): { readonly root: THREE.Object3D; readonly ownedGeometries: readonly THREE.BufferGeometry[] } { const clone = root.clone(true); const geometries: THREE.BufferGeometry[] = []; clone.traverse((object) => { if (object instanceof THREE.Mesh) { object.geometry = object.geometry.clone(); geometries.push(object.geometry); } }); clone.updateWorldMatrix(true, true); return { root: clone, ownedGeometries: geometries }; }
function disposeClonedSkeletons(root: THREE.Object3D): void { const skeletons = new Set<THREE.Skeleton>(); root.traverse((object) => { if (object instanceof THREE.SkinnedMesh) skeletons.add(object.skeleton); }); for (const skeleton of skeletons) skeleton.dispose(); }
function removeClonedLights(root: THREE.Object3D): void { const lights: THREE.Light[] = []; root.traverse((object) => { if (object instanceof THREE.Light) lights.push(object); }); for (const light of lights) light.removeFromParent(); }
function captureCamera(settings: GhostPlateCaptureSettings, center: THREE.Vector3, size: THREE.Vector3): THREE.PerspectiveCamera { const camera = new THREE.PerspectiveCamera(settings.fieldOfViewDegrees, 1, settings.near, settings.far); const azimuth = THREE.MathUtils.degToRad(settings.azimuthDegrees); const elevation = THREE.MathUtils.degToRad(settings.elevationDegrees); const radius = Math.max(size.length() * 1.7, 1); camera.position.set(center.x + Math.sin(azimuth) * Math.cos(elevation) * radius, center.y + Math.sin(elevation) * radius, center.z + Math.cos(azimuth) * Math.cos(elevation) * radius); camera.lookAt(center); camera.updateMatrixWorld(true); return camera; }
function cloneSceneLights(source: THREE.Scene, target: THREE.Scene): () => void { const clones: THREE.Object3D[] = []; source.updateWorldMatrix(true, true); source.traverse((object) => { if (!(object instanceof THREE.Light) || !object.visible) return; const clone = object.clone(false) as THREE.Light; object.matrixWorld.decompose(clone.position, clone.quaternion, clone.scale); clone.matrixAutoUpdate = true; target.add(clone); clones.push(clone); if ((object instanceof THREE.DirectionalLight || object instanceof THREE.SpotLight) && (clone instanceof THREE.DirectionalLight || clone instanceof THREE.SpotLight)) { object.target.updateWorldMatrix(true, false); const targetClone = new THREE.Object3D(); object.target.matrixWorld.decompose(targetClone.position, targetClone.quaternion, targetClone.scale); clone.target = targetClone; target.add(targetClone); clones.push(targetClone); } }); return () => target.remove(...clones); }
function addStudioRig(scene: THREE.Scene, camera: THREE.Camera, center: THREE.Vector3, size: THREE.Vector3, lighting: Exclude<GhostPlateCaptureSettings['lighting'], { readonly mode: 'scene' }>): () => void { camera.updateMatrixWorld(true); const distance = Math.max(2, size.length() * 2); const directional = (direction: readonly [number, number, number], color: readonly [number, number, number], intensity: number) => { const towardLight = new THREE.Vector3(...direction).applyQuaternion(camera.quaternion).normalize(); const light = new THREE.DirectionalLight(new THREE.Color().setRGB(...color), intensity); const target = new THREE.Object3D(); target.position.copy(center); light.position.copy(center).addScaledVector(towardLight, distance); light.target = target; return { light, target }; }; const ambient = new THREE.AmbientLight(new THREE.Color().setRGB(...lighting.ambientColor), lighting.ambientIntensity); const key = directional(lighting.keyDirection, lighting.keyColor, lighting.keyIntensity); const fill = directional(lighting.fillDirection, lighting.fillColor, lighting.fillIntensity); scene.add(ambient, key.light, key.target, fill.light, fill.target); return () => scene.remove(ambient, key.light, key.target, fill.light, fill.target); }
function emptyCapture(): GhostPlateCaptureSettings { return Object.freeze({ resolution: 8, azimuthDegrees: 0, elevationDegrees: 0, near: 0.001, far: 1, fieldOfViewDegrees: 35, lighting: { mode: 'isolated' as const, ambientColor: [1, 1, 1] as const, ambientIntensity: 0, keyDirection: [1, 0, 0] as const, keyColor: [1, 1, 1] as const, keyIntensity: 0, fillDirection: [0, 1, 0] as const, fillColor: [1, 1, 1] as const, fillIntensity: 0 } }); }
function emptyConfig(): GhostPlateConfig { return Object.freeze({ depthRetention: 0.02, anchorPolicy: 'bounds-center', anchorValue: 0, plateMapping: 'plate-locked', shellMode: 'whole-mesh', shellDepthEpsilon: 0, sectorCount: 1, sectorHysteresisDegrees: 0 }); }
function captureBytes(descriptor: GhostPlateDescriptor): number { return descriptor.capture.resolution * descriptor.capture.resolution * descriptor.config.sectorCount * GHOST_CAPTURE_BYTES_PER_PIXEL_PER_SECTOR; }
