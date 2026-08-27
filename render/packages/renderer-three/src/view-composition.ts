import * as THREE from 'three';
import {
  validateRendererViewComposition,
  type RendererCompositionCamera,
  type RendererCompositionTarget,
  type RendererCompositionView,
  type RendererViewComposition,
} from '@rusty-engine/render-contracts';

import type { RendererVisibilityReadout, ThreeRenderer } from './three-renderer.js';
import { applyRendererThreeCameraBasis, applyRendererThreeCameraPose } from './camera-pose.js';

export type RendererViewCompositionDiagnosticCode =
  | 'invalid_view_composition'
  | 'stale_target_revision'
  | 'surface_disposed'
  | 'target_allocation_failed';

export interface RendererViewCompositionDiagnostic {
  readonly code: RendererViewCompositionDiagnosticCode;
  readonly message: string;
}

export interface RendererViewCompositionReceipt {
  readonly applied: boolean;
  readonly diagnostics: readonly RendererViewCompositionDiagnostic[];
  readonly revision: number;
}

export interface RendererViewCompositionReadout {
  readonly schemaVersion: 1;
  readonly revision: number;
  readonly cameras: RendererViewComposition['cameras'];
  readonly targets: readonly (RendererCompositionTarget & {
    readonly lastRefreshedSubmission: number | null;
    readonly status: 'current' | 'never_rendered' | 'stale';
  })[];
  readonly views: RendererViewComposition['views'];
  readonly presentations: RendererViewComposition['presentations'];
  readonly resources: {
    readonly presentationCount: number;
    readonly targetCount: number;
  };
}

export interface RendererViewCompositionVisibilityReadout {
  readonly schemaVersion: 1;
  readonly views: readonly {
    readonly viewId: string;
    readonly cameraId: string;
    readonly target: RendererCompositionView['target']['kind'];
    readonly visibility: RendererVisibilityReadout;
  }[];
}

export class RendererViewCompositionPolicyError extends Error {
  readonly code = 'invalid_view_composition' as const;

  constructor(message: string) {
    super(message);
    this.name = 'RendererViewCompositionPolicyError';
  }
}

interface TargetResource {
  readonly descriptor: RendererCompositionTarget;
  readonly target: THREE.WebGLRenderTarget;
  lastRefreshedSubmission: number | null;
  stale: boolean;
}

interface PresentationResource {
  readonly material: THREE.ShaderMaterial;
  readonly scene: THREE.Scene;
}

interface PreparedComposition {
  readonly cameras: ReadonlyMap<string, THREE.Camera>;
  readonly composition: RendererViewComposition;
  readonly createdTargets: readonly TargetResource[];
  readonly presentations: ReadonlyMap<string, PresentationResource>;
  readonly targets: ReadonlyMap<string, TargetResource>;
}

interface OrderedPrimaryStep {
  readonly id: string;
  readonly kind: 'presentation' | 'view';
  readonly order: number;
}

const EMPTY_COMPOSITION: RendererViewComposition = Object.freeze({
  schemaVersion: 1,
  cameras: Object.freeze([]),
  targets: Object.freeze([]),
  views: Object.freeze([]),
  presentations: Object.freeze([]),
});

/** Backend-private realization of the renderer-neutral multi-view contract. */
export class RendererViewCompositionBackend {
  readonly #highestTargetRevision = new Map<string, number>();
  readonly #projection: ThreeRenderer;
  readonly #viewmodelCamera: THREE.PerspectiveCamera;
  readonly #webgl: THREE.WebGLRenderer;
  #cameras: ReadonlyMap<string, THREE.Camera> = new Map();
  #composition = EMPTY_COMPOSITION;
  #disposed = false;
  #presentations: ReadonlyMap<string, PresentationResource> = new Map();
  #revision = 0;
  #targets: ReadonlyMap<string, TargetResource> = new Map();

  constructor(
    webgl: THREE.WebGLRenderer,
    projection: ThreeRenderer,
    viewmodelCamera = new THREE.PerspectiveCamera(),
  ) {
    this.#webgl = webgl;
    this.#projection = projection;
    this.#viewmodelCamera = viewmodelCamera;
  }

  configure(input: RendererViewComposition): RendererViewCompositionReceipt {
    if (this.#disposed) {
      return this.#rejected('surface_disposed', 'renderer view composition is disposed');
    }

    let prepared: PreparedComposition | null = null;
    try {
      const composition = cloneComposition(input);
      validateRendererViewComposition(composition);
      this.#validateTargetRevisions(composition);
      prepared = this.#prepare(composition);
      this.#publish(prepared);
      return Object.freeze({ applied: true, diagnostics: Object.freeze([]), revision: this.#revision });
    } catch (cause) {
      if (prepared !== null) disposePrepared(prepared, this.#targets);
      const diagnostic = diagnosticFrom(cause);
      return this.#rejected(diagnostic.code, diagnostic.message);
    }
  }

  readout(): RendererViewCompositionReadout {
    const targets = this.#composition.targets.map((descriptor) => {
      const resource = this.#targets.get(descriptor.id);
      const lastRefreshedSubmission = resource?.lastRefreshedSubmission ?? null;
      return Object.freeze({
        ...descriptor,
        lastRefreshedSubmission,
        status: lastRefreshedSubmission === null
          ? 'never_rendered'
          : resource?.stale === true ? 'stale' : 'current',
      } as const);
    });
    return Object.freeze({
      schemaVersion: 1,
      revision: this.#revision,
      cameras: this.#composition.cameras,
      targets: Object.freeze(targets),
      views: this.#composition.views,
      presentations: this.#composition.presentations,
      resources: Object.freeze({
        presentationCount: this.#presentations.size,
        targetCount: this.#targets.size,
      }),
    });
  }

  visibilityReadout(): RendererViewCompositionVisibilityReadout {
    const views = this.#composition.views
      .map((view) => {
        const camera = this.#cameras.get(view.cameraId);
        if (camera === undefined) return null;
        return Object.freeze({
          viewId: view.id,
          cameraId: view.cameraId,
          target: view.target.kind,
          visibility: this.#projection.visibilityReadout(camera, this.#projection.scene),
        });
      })
      .filter((view): view is NonNullable<typeof view> => view !== null)
      .sort((left, right) => left.viewId.localeCompare(right.viewId));
    return Object.freeze({ schemaVersion: 1, views: Object.freeze(views) });
  }

  render(submission: number, primaryWidth: number, primaryHeight: number): void {
    if (this.#disposed || this.#composition.views.length === 0) return;

    const offscreenViews = this.#composition.views
      .filter((view) => view.target.kind === 'offscreen')
      .sort(compareOrdered);
    for (const view of offscreenViews) {
      this.#renderOffscreen(view, submission);
    }

    const steps: OrderedPrimaryStep[] = [
      ...this.#composition.views
        .filter((view) => view.target.kind === 'primary')
        .map((view) => ({ id: view.id, kind: 'view' as const, order: view.order })),
      ...this.#composition.presentations
        .map((presentation) => ({
          id: presentation.id,
          kind: 'presentation' as const,
          order: presentation.order,
        })),
    ].sort(compareOrdered);

    this.#webgl.setRenderTarget(null);
    this.#webgl.setScissorTest(true);
    try {
      for (const step of steps) {
        if (step.kind === 'view') {
          const view = this.#composition.views.find((candidate) => candidate.id === step.id);
          if (view !== undefined) this.#renderPrimaryView(view, primaryWidth, primaryHeight);
        } else {
          const presentation = this.#composition.presentations.find(
            (candidate) => candidate.id === step.id,
          );
          const resource = this.#presentations.get(step.id);
          if (presentation !== undefined && resource !== undefined) {
            const area = pixelViewport(
              presentation.destination.viewport,
              primaryWidth,
              primaryHeight,
            );
            setPhysicalViewport(this.#webgl, area);
            this.#webgl.clear(false, true, false);
            this.#webgl.render(resource.scene, PRESENTATION_CAMERA);
          }
        }
      }
    } finally {
      this.#webgl.setRenderTarget(null);
      this.#webgl.setScissorTest(false);
      setPhysicalViewport(this.#webgl, {
        x: 0,
        y: 0,
        width: primaryWidth,
        height: primaryHeight,
      });
    }
  }

  /** Mark reusable targets stale after retained scene facts change. */
  invalidate(): void {
    if (this.#disposed) return;
    for (const resource of this.#targets.values()) resource.stale = true;
  }

  dispose(): void {
    if (this.#disposed) return;
    disposePresentations(this.#presentations);
    for (const resource of this.#targets.values()) resource.target.dispose();
    this.#cameras = new Map();
    this.#composition = EMPTY_COMPOSITION;
    this.#presentations = new Map();
    this.#targets = new Map();
    this.#disposed = true;
  }

  #prepare(composition: RendererViewComposition): PreparedComposition {
    const cameras = new Map(
      composition.cameras.map((descriptor) => [descriptor.id, createCamera(descriptor)]),
    );
    const targets = new Map<string, TargetResource>();
    const createdTargets: TargetResource[] = [];
    const presentations = new Map<string, PresentationResource>();
    try {
      for (const descriptor of composition.targets) {
        const current = this.#targets.get(descriptor.id);
        if (current !== undefined
          && current.descriptor.revision === descriptor.revision
          && sameTargetDescriptor(current.descriptor, descriptor)) {
          targets.set(descriptor.id, current);
          continue;
        }
        const target = createTarget(descriptor);
        const resource = { descriptor, target, lastRefreshedSubmission: null, stale: true };
        createdTargets.push(resource);
        this.#webgl.initRenderTarget(target);
        targets.set(descriptor.id, resource);
      }
      for (const descriptor of composition.presentations) {
        const source = targets.get(descriptor.sourceTargetId);
        if (source === undefined) throw new Error('validated presentation source is missing');
        presentations.set(descriptor.id, createPresentation(source.target.texture));
      }
      return { cameras, composition, createdTargets, presentations, targets };
    } catch (cause) {
      disposePresentations(presentations);
      for (const resource of createdTargets) resource.target.dispose();
      throw new TargetAllocationError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  #publish(prepared: PreparedComposition): void {
    const priorTargets = this.#targets;
    const priorPresentations = this.#presentations;
    this.#cameras = prepared.cameras;
    this.#composition = prepared.composition;
    this.#presentations = prepared.presentations;
    this.#targets = prepared.targets;
    this.invalidate();
    this.#revision += 1;
    for (const descriptor of prepared.composition.targets) {
      this.#highestTargetRevision.set(descriptor.id, descriptor.revision);
    }
    disposePresentations(priorPresentations);
    for (const [id, resource] of priorTargets) {
      if (prepared.targets.get(id) !== resource) resource.target.dispose();
    }
  }

  #rejected(
    code: RendererViewCompositionDiagnosticCode,
    message: string,
  ): RendererViewCompositionReceipt {
    return Object.freeze({
      applied: false,
      diagnostics: Object.freeze([Object.freeze({ code, message })]),
      revision: this.#revision,
    });
  }

  #renderOffscreen(view: RendererCompositionView, submission: number): void {
    if (view.target.kind !== 'offscreen') return;
    const target = this.#targets.get(view.target.targetId);
    const camera = this.#cameras.get(view.cameraId);
    if (target === undefined || camera === undefined) return;
    const area = pixelViewport(view.viewport, target.descriptor.width, target.descriptor.height);
    updateCameraAspect(camera, area.width / area.height);
    camera.updateMatrixWorld(true);
    this.#projection.scene.updateMatrixWorld(true);
    this.#webgl.setRenderTarget(target.target);
    this.#webgl.setScissorTest(false);
    setPhysicalViewport(this.#webgl, area);
    this.#webgl.setScissorTest(true);
    this.#webgl.clear(true, true, true);
    this.#projection.prepareSpritesForCamera(camera, this.#projection.scene);
    this.#projection.prepareStaticInstanceBatches(camera);
    this.#webgl.render(this.#projection.scene, camera);
    target.lastRefreshedSubmission = submission;
    target.stale = false;
  }

  #renderPrimaryView(
    view: RendererCompositionView,
    primaryWidth: number,
    primaryHeight: number,
  ): void {
    const camera = this.#cameras.get(view.cameraId);
    if (camera === undefined) return;
    const area = pixelViewport(view.viewport, primaryWidth, primaryHeight);
    updateCameraAspect(camera, area.width / area.height);
    this.#syncViewmodelCamera(camera, area.width / area.height);
    setPhysicalViewport(this.#webgl, area);
    this.#webgl.clear(true, true, true);
    this.#projection.prepareSpritesForCamera(camera, this.#projection.scene);
    this.#projection.prepareStaticInstanceBatches(camera);
    this.#webgl.render(this.#projection.scene, camera);
    // Camera-relative retained content belongs to the Engine-owned viewmodel
    // pass. Reapply its depth break after each configured primary view so the
    // primary clear cannot erase it.
    this.#webgl.clearDepth();
    this.#projection.prepareSpritesForCamera(
      this.#viewmodelCamera,
      this.#projection.viewmodelScene,
    );
    this.#webgl.render(this.#projection.viewmodelScene, this.#viewmodelCamera);
  }

  #syncViewmodelCamera(camera: THREE.Camera, aspect: number): void {
    this.#viewmodelCamera.position.copy(camera.position);
    this.#viewmodelCamera.quaternion.copy(camera.quaternion);
    this.#viewmodelCamera.up.copy(camera.up);
    if (camera instanceof THREE.PerspectiveCamera) {
      this.#viewmodelCamera.fov = camera.fov;
      this.#viewmodelCamera.near = camera.near;
      this.#viewmodelCamera.far = camera.far;
    }
    this.#viewmodelCamera.aspect = aspect;
    this.#viewmodelCamera.updateProjectionMatrix();
    this.#viewmodelCamera.updateMatrixWorld(true);
  }

  #validateTargetRevisions(composition: RendererViewComposition): void {
    for (const descriptor of composition.targets) {
      const current = this.#targets.get(descriptor.id)?.descriptor;
      const highest = this.#highestTargetRevision.get(descriptor.id);
      if (current !== undefined && descriptor.revision === current.revision) {
        if (!sameTargetDescriptor(current, descriptor)) {
          throw new StaleTargetRevisionError(
            `${descriptor.id} revision ${String(descriptor.revision)} cannot change target facts`,
          );
        }
        continue;
      }
      if (highest !== undefined && descriptor.revision <= highest) {
        throw new StaleTargetRevisionError(
          `${descriptor.id} revision must be greater than ${String(highest)}`,
        );
      }
    }
  }
}

class StaleTargetRevisionError extends Error {}
class TargetAllocationError extends Error {}

const PRESENTATION_CAMERA = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 10);
PRESENTATION_CAMERA.position.z = 1;
PRESENTATION_CAMERA.updateMatrixWorld(true);

function cloneComposition(input: RendererViewComposition): RendererViewComposition {
  return freezeValue(structuredClone(input) as RendererViewComposition);
}

function freezeValue<T>(value: T): T {
  if (value === null || typeof value !== 'object' || Object.isFrozen(value)) return value;
  for (const nested of Object.values(value)) freezeValue(nested);
  return Object.freeze(value);
}

function createCamera(descriptor: RendererCompositionCamera): THREE.Camera {
  const camera = descriptor.projection.kind === 'perspective'
    ? new THREE.PerspectiveCamera(
        descriptor.projection.fovYDegrees,
        1,
        descriptor.projection.near,
        descriptor.projection.far,
      )
    : new THREE.OrthographicCamera(
        -descriptor.projection.verticalSize / 2,
        descriptor.projection.verticalSize / 2,
        descriptor.projection.verticalSize / 2,
        -descriptor.projection.verticalSize / 2,
        descriptor.projection.near,
        descriptor.projection.far,
      );
  camera.name = descriptor.id;
  applyRendererThreeCameraPose(camera, descriptor.pose);
  if (descriptor.basis !== undefined) applyRendererThreeCameraBasis(camera, descriptor.basis);
  camera.updateMatrixWorld(true);
  return camera;
}

function updateCameraAspect(camera: THREE.Camera, aspect: number): void {
  if (camera instanceof THREE.PerspectiveCamera) {
    camera.aspect = aspect;
    camera.updateProjectionMatrix();
    return;
  }
  if (camera instanceof THREE.OrthographicCamera) {
    const verticalSize = camera.top - camera.bottom;
    camera.left = -(verticalSize * aspect) / 2;
    camera.right = (verticalSize * aspect) / 2;
    camera.updateProjectionMatrix();
  }
}

function createTarget(descriptor: RendererCompositionTarget): THREE.WebGLRenderTarget {
  const filter = descriptor.sampling === 'nearest' ? THREE.NearestFilter : THREE.LinearFilter;
  const target = new THREE.WebGLRenderTarget(descriptor.width, descriptor.height, {
    depthBuffer: descriptor.depth === 'depth24',
    generateMipmaps: false,
    magFilter: filter,
    minFilter: filter,
    stencilBuffer: false,
  });
  target.texture.colorSpace = THREE.SRGBColorSpace;
  target.texture.name = descriptor.id;
  return target;
}

function createPresentation(texture: THREE.Texture): PresentationResource {
  const material = new THREE.ShaderMaterial({
    depthTest: false,
    depthWrite: false,
    fragmentShader: `
      uniform sampler2D sourceTarget;
      varying vec2 sourceUv;
      void main() {
        gl_FragColor = texture2D(sourceTarget, sourceUv);
      }
    `,
    toneMapped: false,
    uniforms: { sourceTarget: { value: texture } },
    vertexShader: `
      varying vec2 sourceUv;
      void main() {
        sourceUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
  });
  const geometry = new THREE.PlaneGeometry(2, 2);
  const mesh = new THREE.Mesh(geometry, material);
  const scene = new THREE.Scene();
  scene.add(mesh);
  return { material, scene };
}

function disposePresentations(resources: ReadonlyMap<string, PresentationResource>): void {
  for (const resource of resources.values()) {
    for (const child of resource.scene.children) {
      if (child instanceof THREE.Mesh) child.geometry.dispose();
    }
    resource.material.dispose();
  }
}

function disposePrepared(
  prepared: PreparedComposition,
  currentTargets: ReadonlyMap<string, TargetResource>,
): void {
  disposePresentations(prepared.presentations);
  for (const resource of prepared.createdTargets) {
    if (currentTargets.get(resource.descriptor.id) !== resource) resource.target.dispose();
  }
}

function sameTargetDescriptor(
  left: RendererCompositionTarget,
  right: RendererCompositionTarget,
): boolean {
  return left.id === right.id
    && left.revision === right.revision
    && left.width === right.width
    && left.height === right.height
    && left.color === right.color
    && left.depth === right.depth
    && left.sampling === right.sampling;
}

function compareOrdered(
  left: { readonly id: string; readonly order: number },
  right: { readonly id: string; readonly order: number },
): number {
  return left.order - right.order || left.id.localeCompare(right.id);
}

interface PixelViewport {
  readonly height: number;
  readonly width: number;
  readonly x: number;
  readonly y: number;
}

function pixelViewport(
  viewport: { readonly x: number; readonly y: number; readonly width: number; readonly height: number },
  destinationWidth: number,
  destinationHeight: number,
): PixelViewport {
  const x = Math.round(viewport.x * destinationWidth);
  const y = Math.round(viewport.y * destinationHeight);
  return {
    x,
    y,
    width: Math.max(1, Math.min(destinationWidth - x, Math.round(viewport.width * destinationWidth))),
    height: Math.max(1, Math.min(destinationHeight - y, Math.round(viewport.height * destinationHeight))),
  };
}

function setPhysicalViewport(webgl: THREE.WebGLRenderer, area: PixelViewport): void {
  const scale = 1 / webgl.getPixelRatio();
  webgl.setViewport(area.x * scale, area.y * scale, area.width * scale, area.height * scale);
  webgl.setScissor(area.x * scale, area.y * scale, area.width * scale, area.height * scale);
}

function diagnosticFrom(cause: unknown): RendererViewCompositionDiagnostic {
  const message = cause instanceof Error ? cause.message : String(cause);
  if (cause instanceof StaleTargetRevisionError) {
    return { code: 'stale_target_revision', message };
  }
  if (cause instanceof TargetAllocationError) {
    return { code: 'target_allocation_failed', message };
  }
  return { code: 'invalid_view_composition', message };
}
