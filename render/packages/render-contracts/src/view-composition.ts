import type { PerspectiveProjection } from './render.js';

export const RUSTY_RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION = 1;
export const MAX_RENDERER_COMPOSITION_CAMERAS = 4;
export const MAX_RENDERER_COMPOSITION_TARGETS = 4;
export const MAX_RENDERER_COMPOSITION_VIEWS = 8;
export const MAX_RENDERER_COMPOSITION_PRESENTATIONS = 4;
export const MAX_RENDERER_TARGET_DIMENSION = 2_048;
export const MAX_RENDERER_TARGET_PIXELS = 8_388_608;

export type RendererCompositionIdentifier = string;

export interface RendererCompositionCameraPose {
  readonly position: readonly [number, number, number];
  readonly pitchDegrees: number;
  readonly yawDegrees: number;
}

export type RendererCompositionProjection =
  | ({ readonly kind: 'perspective' } & PerspectiveProjection)
  | {
      readonly kind: 'orthographic';
      readonly verticalSize: number;
      readonly near: number;
      readonly far: number;
    };

export interface RendererCompositionCamera {
  readonly id: RendererCompositionIdentifier;
  readonly pose: RendererCompositionCameraPose;
  readonly projection: RendererCompositionProjection;
}

export interface RendererCompositionTarget {
  readonly id: RendererCompositionIdentifier;
  /** Caller-owned monotonic identity for replacement and stale-reference rejection. */
  readonly revision: number;
  readonly width: number;
  readonly height: number;
  readonly color: 'rgba8_srgb';
  readonly depth: 'depth24' | 'none';
  readonly sampling: 'linear' | 'nearest';
}

export interface RendererCompositionViewport {
  /** Normalized lower-left origin in the selected destination. */
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

export type RendererCompositionViewTarget =
  | { readonly kind: 'primary' }
  | {
      readonly kind: 'offscreen';
      readonly targetId: RendererCompositionIdentifier;
      readonly targetRevision: number;
    };

export interface RendererCompositionView {
  readonly id: RendererCompositionIdentifier;
  readonly cameraId: RendererCompositionIdentifier;
  readonly target: RendererCompositionViewTarget;
  readonly viewport: RendererCompositionViewport;
  readonly order: number;
}

export interface RendererCompositionPresentation {
  readonly id: RendererCompositionIdentifier;
  readonly sourceTargetId: RendererCompositionIdentifier;
  readonly sourceTargetRevision: number;
  readonly destination: {
    readonly kind: 'primary';
    readonly viewport: RendererCompositionViewport;
  };
  readonly order: number;
}

export interface RendererViewComposition {
  readonly schemaVersion: 1;
  readonly cameras: readonly RendererCompositionCamera[];
  readonly targets: readonly RendererCompositionTarget[];
  readonly views: readonly RendererCompositionView[];
  readonly presentations: readonly RendererCompositionPresentation[];
}

export class RendererViewCompositionValidationError extends Error {
  readonly code = 'invalid_view_composition' as const;

  constructor(
    readonly path: string,
    message: string,
  ) {
    super(`${path} ${message}`);
    this.name = 'RendererViewCompositionValidationError';
  }
}

export function validateRendererViewComposition(
  input: RendererViewComposition,
): RendererViewComposition {
  if (input.schemaVersion !== RUSTY_RENDERER_VIEW_COMPOSITION_SCHEMA_VERSION) {
    fail('composition.schemaVersion', 'must equal 1');
  }
  boundedList(input.cameras, 'composition.cameras', MAX_RENDERER_COMPOSITION_CAMERAS);
  boundedList(input.targets, 'composition.targets', MAX_RENDERER_COMPOSITION_TARGETS);
  boundedList(input.views, 'composition.views', MAX_RENDERER_COMPOSITION_VIEWS);
  boundedList(
    input.presentations,
    'composition.presentations',
    MAX_RENDERER_COMPOSITION_PRESENTATIONS,
  );

  const cameras = new Map<string, RendererCompositionCamera>();
  for (const [index, camera] of input.cameras.entries()) {
    const path = `composition.cameras[${String(index)}]`;
    uniqueIdentifier(camera.id, `${path}.id`, cameras);
    finiteVec3(camera.pose.position, `${path}.pose.position`);
    finite(camera.pose.pitchDegrees, `${path}.pose.pitchDegrees`);
    finite(camera.pose.yawDegrees, `${path}.pose.yawDegrees`);
    projection(camera.projection, `${path}.projection`);
    cameras.set(camera.id, camera);
  }

  const targets = new Map<string, RendererCompositionTarget>();
  let targetPixels = 0;
  for (const [index, target] of input.targets.entries()) {
    const path = `composition.targets[${String(index)}]`;
    uniqueIdentifier(target.id, `${path}.id`, targets);
    integer(target.revision, `${path}.revision`, 1, Number.MAX_SAFE_INTEGER);
    integer(target.width, `${path}.width`, 1, MAX_RENDERER_TARGET_DIMENSION);
    integer(target.height, `${path}.height`, 1, MAX_RENDERER_TARGET_DIMENSION);
    if (target.color !== 'rgba8_srgb') fail(`${path}.color`, 'must equal rgba8_srgb');
    if (target.depth !== 'depth24' && target.depth !== 'none') {
      fail(`${path}.depth`, 'must equal depth24 or none');
    }
    if (target.sampling !== 'linear' && target.sampling !== 'nearest') {
      fail(`${path}.sampling`, 'must equal linear or nearest');
    }
    targetPixels = checkedAdd(
      targetPixels,
      target.width * target.height,
      'composition.targets',
    );
    if (targetPixels > MAX_RENDERER_TARGET_PIXELS) {
      fail(
        'composition.targets',
        `aggregate pixels must not exceed ${String(MAX_RENDERER_TARGET_PIXELS)}`,
      );
    }
    targets.set(target.id, target);
  }

  const viewIds = new Set<string>();
  const producedTargets = new Set<string>();
  for (const [index, view] of input.views.entries()) {
    const path = `composition.views[${String(index)}]`;
    uniqueIdentifier(view.id, `${path}.id`, viewIds);
    identifier(view.cameraId, `${path}.cameraId`);
    if (!cameras.has(view.cameraId)) {
      fail(`${path}.cameraId`, `does not name an admitted camera ${JSON.stringify(view.cameraId)}`);
    }
    viewport(view.viewport, `${path}.viewport`);
    integer(view.order, `${path}.order`, 0, 65_535);
    if (view.target.kind === 'primary') continue;
    if (view.target.kind !== 'offscreen') {
      fail(`${path}.target.kind`, 'must equal primary or offscreen');
    }
    const target = targets.get(view.target.targetId);
    if (target === undefined) {
      fail(`${path}.target.targetId`, 'does not name an admitted target');
    }
    if (target.revision !== view.target.targetRevision) {
      fail(`${path}.target.targetRevision`, 'must equal the admitted target revision');
    }
    if (producedTargets.has(target.id)) {
      fail(`${path}.target.targetId`, 'already has a producing view');
    }
    producedTargets.add(target.id);
  }

  const presentationIds = new Set<string>();
  for (const [index, presentation] of input.presentations.entries()) {
    const path = `composition.presentations[${String(index)}]`;
    uniqueIdentifier(presentation.id, `${path}.id`, presentationIds);
    const source = targets.get(presentation.sourceTargetId);
    if (source === undefined) {
      fail(`${path}.sourceTargetId`, 'does not name an admitted target');
    }
    if (source.revision !== presentation.sourceTargetRevision) {
      fail(`${path}.sourceTargetRevision`, 'must equal the admitted target revision');
    }
    if (!producedTargets.has(source.id)) {
      fail(`${path}.sourceTargetId`, 'must have one producing view in the same composition');
    }
    if (presentation.destination.kind !== 'primary') {
      fail(`${path}.destination.kind`, 'must equal primary; render-target feedback is unsupported');
    }
    viewport(presentation.destination.viewport, `${path}.destination.viewport`);
    integer(presentation.order, `${path}.order`, 0, 65_535);
  }
  return input;
}

function projection(value: RendererCompositionProjection, path: string): void {
  finite(value.near, `${path}.near`);
  finite(value.far, `${path}.far`);
  if (value.near <= 0 || value.far <= value.near) {
    fail(path, 'must have 0 < near < far');
  }
  if (value.kind === 'perspective') {
    finite(value.fovYDegrees, `${path}.fovYDegrees`);
    if (value.fovYDegrees <= 0 || value.fovYDegrees >= 180) {
      fail(`${path}.fovYDegrees`, 'must be greater than 0 and less than 180');
    }
    return;
  }
  if (value.kind === 'orthographic') {
    finite(value.verticalSize, `${path}.verticalSize`);
    if (value.verticalSize <= 0) fail(`${path}.verticalSize`, 'must be greater than 0');
    return;
  }
  fail(`${path}.kind`, 'must equal perspective or orthographic');
}

function viewport(value: RendererCompositionViewport, path: string): void {
  for (const [name, component] of Object.entries(value)) {
    finite(component, `${path}.${name}`);
  }
  if (value.x < 0 || value.y < 0 || value.width <= 0 || value.height <= 0) {
    fail(path, 'must have non-negative origin and positive extent');
  }
  if (value.x + value.width > 1 || value.y + value.height > 1) {
    fail(path, 'must fit inside normalized destination bounds');
  }
}

function uniqueIdentifier<T>(
  value: string,
  path: string,
  values: Map<string, T> | Set<string>,
): void {
  identifier(value, path);
  if (values.has(value)) fail(path, `duplicates ${JSON.stringify(value)}`);
}

function identifier(value: string, path: string): void {
  if (!/^[a-z][a-z0-9._-]{0,63}$/u.test(value)) {
    fail(path, 'must be a lowercase stable identifier of at most 64 characters');
  }
}

function boundedList(value: readonly unknown[], path: string, maximum: number): void {
  if (!Array.isArray(value)) fail(path, 'must be an array');
  if (value.length > maximum) fail(path, `must contain at most ${String(maximum)} entries`);
}

function finiteVec3(value: readonly number[], path: string): void {
  if (!Array.isArray(value) || value.length !== 3) fail(path, 'must contain exactly 3 values');
  value.forEach((component, index) => finite(component, `${path}[${String(index)}]`));
}

function finite(value: number, path: string): void {
  if (!Number.isFinite(value)) fail(path, 'must be finite');
}

function integer(value: number, path: string, minimum: number, maximum: number): void {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    fail(path, `must be a safe integer in ${String(minimum)}..=${String(maximum)}`);
  }
}

function checkedAdd(left: number, right: number, path: string): number {
  const result = left + right;
  if (!Number.isSafeInteger(result)) fail(path, 'aggregate size overflowed');
  return result;
}

function fail(path: string, message: string): never {
  throw new RendererViewCompositionValidationError(path, message);
}
