import {
  renderHandle,
  type CameraBasis,
  type RenderFrameDiff,
  type RenderHandle,
  type Transform,
} from '@rusty-engine/render-contracts';

export type StudioTransformTool = 'translate' | 'rotate' | 'scale';
export type StudioTransformOrientation = 'world' | 'local';
export type StudioTransformAxis = 0 | 1 | 2;
export type StudioTransformPlane = 'xy' | 'xz' | 'yz';

export type StudioTransformHandle =
  | { readonly kind: 'axis'; readonly tool: StudioTransformTool; readonly axis: StudioTransformAxis }
  | { readonly kind: 'plane'; readonly tool: 'translate'; readonly plane: StudioTransformPlane }
  | { readonly kind: 'uniform'; readonly tool: 'scale' };

export interface StudioTransformManipulatorCamera {
  readonly position: readonly [number, number, number];
  readonly basis: CameraBasis;
  readonly fovYDegrees: number;
  readonly viewport: { readonly width: number; readonly height: number };
}

export interface StudioTransformSnapping {
  readonly enabled: boolean;
  readonly rotationDegrees: number;
  readonly scale: readonly [number, number, number];
  readonly translation: readonly [number, number, number];
}

export interface StudioTransformManipulatorAppearance {
  readonly active: StudioTransformHandle | null;
  readonly hovered: StudioTransformHandle | null;
  readonly orientation: StudioTransformOrientation;
  readonly tool: StudioTransformTool;
  /** World transform used only for disposable viewport presentation. */
  readonly transform: Transform;
  readonly visible: boolean;
}

export interface StudioTransformManipulatorDragInput {
  readonly camera: StudioTransformManipulatorCamera;
  readonly handle: StudioTransformHandle;
  readonly orientation: StudioTransformOrientation;
  readonly pointer: readonly [number, number];
  readonly revision: string;
  readonly snapping: StudioTransformSnapping;
  /** Fixed world-transform baseline captured at pointer-down. */
  readonly source: Transform;
}

export interface StudioTransformManipulatorDrag {
  readonly kind: 'rusty_studio_transform_manipulator_drag.v1';
  readonly axis: Vec3 | null;
  readonly handle: StudioTransformHandle;
  readonly orientation: StudioTransformOrientation;
  readonly planeNormal: Vec3 | null;
  readonly revision: string;
  readonly snapping: StudioTransformSnapping;
  readonly source: Transform;
  readonly startAxisParameter: number | null;
  readonly startPlanePoint: Vec3 | null;
  readonly startRotationVector: Vec3 | null;
  readonly startPointer: readonly [number, number];
}

export interface StudioTransformManipulatorCandidate {
  readonly kind: 'rusty_studio_transform_manipulator_candidate.v1';
  readonly diagnostics: readonly string[];
  readonly previewOnly: true;
  readonly revision: string;
  /** Candidate world transform. Rust remains the only commit authority. */
  readonly transform: Transform;
}

const AXIS_VECTORS = [
  [1, 0, 0],
  [0, 1, 0],
  [0, 0, 1],
] as const;

const AXIS_NAMES = ['x', 'y', 'z'] as const;

const AXIS_COLORS = [
  [0.95, 0.2, 0.18, 1],
  [0.2, 0.9, 0.3, 1],
  [0.2, 0.45, 1, 1],
] as const;

const HANDLE_BASE = Number.MAX_SAFE_INTEGER - 128;
const EPSILON = 1e-8;

const HANDLE_SLOTS: readonly StudioTransformHandle[] = [
  { kind: 'axis', tool: 'translate', axis: 0 },
  { kind: 'axis', tool: 'translate', axis: 1 },
  { kind: 'axis', tool: 'translate', axis: 2 },
  { kind: 'plane', tool: 'translate', plane: 'xy' },
  { kind: 'plane', tool: 'translate', plane: 'xz' },
  { kind: 'plane', tool: 'translate', plane: 'yz' },
  { kind: 'axis', tool: 'rotate', axis: 0 },
  { kind: 'axis', tool: 'rotate', axis: 1 },
  { kind: 'axis', tool: 'rotate', axis: 2 },
  { kind: 'axis', tool: 'scale', axis: 0 },
  { kind: 'axis', tool: 'scale', axis: 1 },
  { kind: 'axis', tool: 'scale', axis: 2 },
  { kind: 'uniform', tool: 'scale' },
] as const;

export function studioTransformHandleId(handle: StudioTransformHandle): RenderHandle {
  const slot = HANDLE_SLOTS.findIndex((candidate) => sameHandle(candidate, handle));
  if (slot < 0) throw new Error('unsupported Studio transform handle');
  return renderHandle(HANDLE_BASE + slot);
}

export function studioTransformHandleFromId(handle: RenderHandle): StudioTransformHandle | null {
  const slot = (handle as number) - HANDLE_BASE;
  return HANDLE_SLOTS[slot] ?? null;
}

export function projectStudioTransformManipulator(
  appearance: StudioTransformManipulatorAppearance,
): RenderFrameDiff {
  if (!appearance.visible) return { schemaVersion: 1, ops: [] };
  const handles = HANDLE_SLOTS.filter((handle) => handle.tool === appearance.tool);
  return {
    schemaVersion: 1,
    ops: handles.map((handle) => {
      const axis = handle.kind === 'axis'
        ? orientedAxis(handle.axis, appearance.orientation, appearance.transform)
        : null;
      const placement = handlePlacement(
        handle,
        appearance.transform,
        axis,
        appearance.orientation,
      );
      const label = transformHandleLabel(handle);
      return {
        op: 'create' as const,
        handle: studioTransformHandleId(handle),
        parent: null,
        node: {
          geometry: placement.geometry,
          material: {
            color: handleColor(handle, appearance.active, appearance.hovered),
            wireframe: placement.wireframe,
          },
          transform: placement.transform,
          visible: true,
          layer: 'debug' as const,
          metadata: {
            sourceEntity: null,
            sourceSceneNode: null,
            tags: ['studio-transform-manipulator', label],
            label,
          },
        },
      };
    }),
  };
}

export function beginStudioTransformManipulatorDrag(
  input: StudioTransformManipulatorDragInput,
): StudioTransformManipulatorDrag {
  validateTransform(input.source);
  validateCamera(input.camera);
  const ray = pointerRay(input.camera, input.pointer);
  const axis = input.handle.kind === 'axis'
    ? orientedAxis(input.handle.axis, input.orientation, input.source)
    : input.handle.kind === 'uniform'
      ? normalize(add(input.camera.basis.right, input.camera.basis.up))
      : null;
  const planeNormal = dragPlaneNormal(
    input.handle,
    axis,
    input.camera,
    input.orientation,
    input.source,
  );
  const startPlanePoint = planeNormal === null
    ? null
    : rayPlaneIntersection(ray.origin, ray.direction, input.source.translation, planeNormal);
  const startAxisParameter = axis === null
    ? null
    : closestAxisParameter(ray.origin, ray.direction, input.source.translation, axis);
  const startRotationVector = input.handle.tool === 'rotate' && startPlanePoint !== null
    ? normalize(subtract(startPlanePoint, input.source.translation))
    : null;
  return {
    kind: 'rusty_studio_transform_manipulator_drag.v1',
    axis,
    handle: input.handle,
    orientation: input.orientation,
    planeNormal,
    revision: requireIdentity(input.revision, 'revision'),
    snapping: validateSnapping(input.snapping),
    source: cloneTransform(input.source),
    startAxisParameter,
    startPlanePoint,
    startRotationVector,
    startPointer: input.pointer,
  };
}

export function updateStudioTransformManipulatorDrag(
  drag: StudioTransformManipulatorDrag,
  camera: StudioTransformManipulatorCamera,
  pointer: readonly [number, number],
  options: { readonly fine?: boolean; readonly snapping?: boolean } = {},
): StudioTransformManipulatorCandidate {
  validateCamera(camera);
  const fine = options.fine === true ? 0.1 : 1;
  const snapping = options.snapping ?? drag.snapping.enabled;
  const ray = pointerRay(camera, pointer);
  const diagnostics: string[] = [];
  let transform = cloneTransform(drag.source);

  if (drag.handle.tool === 'translate') {
    const delta = translationDelta(drag, ray, camera, pointer, fine, diagnostics);
    transform = {
      ...transform,
      translation: translatedCandidate(drag, delta, snapping, fine),
    };
  } else if (drag.handle.tool === 'rotate' && drag.axis !== null) {
    const radians = rotationDelta(drag, ray, camera, pointer, fine, diagnostics);
    const snapped = snapScalar(
      radians,
      degreesToRadians(drag.snapping.rotationDegrees * fine),
      snapping,
    );
    const deltaRotation = axisAngleQuaternion(drag.axis, snapped);
    transform = {
      ...transform,
      rotation: normalizeQuaternion(
        drag.orientation === 'local'
          ? multiplyQuaternion(drag.source.rotation, deltaRotation)
          : multiplyQuaternion(deltaRotation, drag.source.rotation),
      ),
    };
  } else if (drag.handle.tool === 'scale') {
    const factor = scaleFactor(drag, ray, camera, pointer, fine, diagnostics);
    const next = drag.handle.kind === 'uniform'
      ? drag.source.scale.map((value) => value * factor) as [number, number, number]
      : applyAxisScale(drag.source.scale, drag.handle.axis, factor);
    transform = {
      ...transform,
      scale: clampScale(scaledCandidate(drag, next, snapping, fine)),
    };
  }

  return {
    kind: 'rusty_studio_transform_manipulator_candidate.v1',
    diagnostics,
    previewOnly: true,
    revision: drag.revision,
    transform,
  };
}

function translatedCandidate(
  drag: StudioTransformManipulatorDrag,
  delta: Vec3,
  snapping: boolean,
  fine: number,
): [number, number, number] {
  if (!snapping || drag.handle.kind === 'uniform') {
    return [...add(drag.source.translation, delta)];
  }
  const axes = drag.handle.kind === 'axis'
    ? [drag.handle.axis] as const
    : planeAxes(drag.handle.plane);
  let candidate: Vec3 = drag.source.translation;
  for (const axisIndex of axes) {
    const direction = orientedAxis(axisIndex, drag.orientation, drag.source);
    const sourceAmount = dot(drag.source.translation, direction);
    const targetAmount = snapScalar(
      sourceAmount + dot(delta, direction),
      drag.snapping.translation[axisIndex] * fine,
      true,
    );
    candidate = add(candidate, scale(direction, targetAmount - sourceAmount));
  }
  return [...candidate];
}

function scaledCandidate(
  drag: StudioTransformManipulatorDrag,
  candidate: Vec3,
  snapping: boolean,
  fine: number,
): [number, number, number] {
  if (!snapping) return [...candidate];
  if (drag.handle.kind === 'axis') {
    const result: [number, number, number] = [...candidate];
    result[drag.handle.axis] = snapScalar(
      result[drag.handle.axis],
      drag.snapping.scale[drag.handle.axis] * fine,
      true,
    );
    return result;
  }
  return snapVector(candidate, drag.snapping.scale.map(
    (increment) => increment * fine,
  ) as [number, number, number], true);
}

export function cancelStudioTransformManipulatorDrag(
  drag: StudioTransformManipulatorDrag,
): StudioTransformManipulatorCandidate {
  return {
    kind: 'rusty_studio_transform_manipulator_candidate.v1',
    diagnostics: ['Studio transform drag cancelled; pointer-down baseline restored'],
    previewOnly: true,
    revision: drag.revision,
    transform: cloneTransform(drag.source),
  };
}

function translationDelta(
  drag: StudioTransformManipulatorDrag,
  ray: Ray,
  camera: StudioTransformManipulatorCamera,
  pointer: readonly [number, number],
  fine: number,
  diagnostics: string[],
): Vec3 {
  if (drag.handle.kind === 'plane' && drag.planeNormal !== null && drag.startPlanePoint !== null) {
    const point = rayPlaneIntersection(
      ray.origin,
      ray.direction,
      drag.source.translation,
      drag.planeNormal,
    );
    if (point !== null) return scale(subtract(point, drag.startPlanePoint), fine);
  }
  if (drag.axis !== null && drag.startAxisParameter !== null) {
    const parameter = closestAxisParameter(
      ray.origin,
      ray.direction,
      drag.source.translation,
      drag.axis,
    );
    if (parameter !== null) return scale(drag.axis, (parameter - drag.startAxisParameter) * fine);
  }
  diagnostics.push('used camera-space translation fallback for a near-parallel drag');
  const amount = pointerFallback(
    camera,
    drag.startPointer,
    pointer,
    drag.source.translation,
  ) * fine;
  return scale(drag.axis ?? camera.basis.right, amount);
}

function rotationDelta(
  drag: StudioTransformManipulatorDrag,
  ray: Ray,
  camera: StudioTransformManipulatorCamera,
  pointer: readonly [number, number],
  fine: number,
  diagnostics: string[],
): number {
  if (drag.planeNormal !== null && drag.startRotationVector !== null) {
    const point = rayPlaneIntersection(
      ray.origin,
      ray.direction,
      drag.source.translation,
      drag.planeNormal,
    );
    if (point !== null) {
      const current = normalize(subtract(point, drag.source.translation));
      const crossed = cross(drag.startRotationVector, current);
      return Math.atan2(
        dot(crossed, drag.axis ?? drag.planeNormal),
        dot(drag.startRotationVector, current),
      ) * fine;
    }
  }
  diagnostics.push('used screen-space rotation fallback for a near-parallel drag');
  return ((pointer[0] - drag.startPointer[0]) / Math.max(1, camera.viewport.width))
    * Math.PI * 2 * fine;
}

function scaleFactor(
  drag: StudioTransformManipulatorDrag,
  ray: Ray,
  camera: StudioTransformManipulatorCamera,
  pointer: readonly [number, number],
  fine: number,
  diagnostics: string[],
): number {
  if (drag.axis !== null && drag.startAxisParameter !== null) {
    const parameter = closestAxisParameter(
      ray.origin,
      ray.direction,
      drag.source.translation,
      drag.axis,
    );
    if (parameter !== null) return Math.max(0.001, 1 + (parameter - drag.startAxisParameter) * fine);
  }
  diagnostics.push('used screen-space scale fallback for a near-parallel drag');
  const pixels = (pointer[0] - drag.startPointer[0]) - (pointer[1] - drag.startPointer[1]);
  return Math.max(0.001, 1 + pixels / Math.max(1, camera.viewport.height) * 2 * fine);
}

function handlePlacement(
  handle: StudioTransformHandle,
  source: Transform,
  axis: Vec3 | null,
  orientation: StudioTransformOrientation,
): {
  readonly geometry: { readonly kind: 'cube' | 'sphere' };
  readonly transform: Transform;
  readonly wireframe: boolean;
} {
  if (handle.kind === 'uniform') {
    return {
      geometry: { kind: 'sphere' },
      transform: {
        translation: source.translation,
        rotation: [0, 0, 0, 1],
        scale: [0.18, 0.18, 0.18],
      },
      wireframe: false,
    };
  }
  if (handle.kind === 'plane') {
    const [firstAxis, secondAxis] = planeAxes(handle.plane);
    const first = orientedAxis(firstAxis, orientation, source);
    const second = orientedAxis(secondAxis, orientation, source);
    const scaleValue: Vec3 = handle.plane === 'xy'
      ? [0.22, 0.22, 0.035]
      : handle.plane === 'xz'
        ? [0.22, 0.035, 0.22]
        : [0.035, 0.22, 0.22];
    return {
      geometry: { kind: 'cube' },
      transform: {
        translation: add(source.translation, scale(add(first, second), 0.28)),
        rotation: orientation === 'local' ? source.rotation : [0, 0, 0, 1],
        scale: scaleValue,
      },
      wireframe: true,
    };
  }
  const resolvedAxis = axis ?? AXIS_VECTORS[handle.axis];
  const distance = handle.tool === 'rotate' ? 0.78 : 0.62;
  const thickness = handle.tool === 'rotate' ? 0.07 : 0.09;
  return {
    geometry: handle.tool === 'rotate' ? { kind: 'sphere' } : { kind: 'cube' },
    transform: {
      translation: add(source.translation, scale(resolvedAxis, distance)),
      rotation: quaternionFromUnitX(resolvedAxis),
      scale: handle.tool === 'rotate'
        ? [0.16, 0.16, 0.16]
        : [0.55, thickness, thickness],
    },
    wireframe: handle.tool === 'rotate',
  };
}

function handleColor(
  handle: StudioTransformHandle,
  active: StudioTransformHandle | null,
  hovered: StudioTransformHandle | null,
): readonly [number, number, number, number] {
  if (active !== null && sameHandle(active, handle)) return [1, 0.85, 0.1, 1];
  if (hovered !== null && sameHandle(hovered, handle)) return [1, 1, 0.55, 1];
  if (handle.kind === 'axis') return AXIS_COLORS[handle.axis];
  if (handle.kind === 'plane') return [0.85, 0.85, 0.85, 0.45];
  return [0.95, 0.95, 0.95, 1];
}

function transformHandleLabel(handle: StudioTransformHandle): string {
  const target = handle.kind === 'axis'
    ? AXIS_NAMES[handle.axis]
    : handle.kind === 'plane'
      ? handle.plane
      : 'uniform';
  return `studio-transform-manipulator:${handle.tool}:${target}`;
}

function dragPlaneNormal(
  handle: StudioTransformHandle,
  axis: Vec3 | null,
  camera: StudioTransformManipulatorCamera,
  orientation: StudioTransformOrientation,
  source: Transform,
): Vec3 | null {
  if (handle.kind === 'plane') {
    const [first, second] = planeAxes(handle.plane);
    return normalize(cross(
      orientedAxis(first, orientation, source),
      orientedAxis(second, orientation, source),
    ));
  }
  if (handle.tool === 'rotate') return axis;
  if (axis === null) return normalize(camera.basis.forward);
  const perpendicular = cross(axis, camera.basis.forward);
  if (length(perpendicular) <= EPSILON) return normalize(camera.basis.up);
  return normalize(cross(perpendicular, axis));
}

function orientedAxis(
  axis: StudioTransformAxis,
  orientation: StudioTransformOrientation,
  transform: Transform,
): Vec3 {
  const world = AXIS_VECTORS[axis];
  return orientation === 'local' ? normalize(rotateVector(transform.rotation, world)) : world;
}

function planeAxes(
  plane: StudioTransformPlane,
): readonly [StudioTransformAxis, StudioTransformAxis] {
  if (plane === 'xy') return [0, 1];
  if (plane === 'xz') return [0, 2];
  return [1, 2];
}

function sameHandle(left: StudioTransformHandle, right: StudioTransformHandle): boolean {
  if (left.kind !== right.kind || left.tool !== right.tool) return false;
  if (left.kind === 'axis' && right.kind === 'axis') return left.axis === right.axis;
  if (left.kind === 'plane' && right.kind === 'plane') return left.plane === right.plane;
  return left.kind === 'uniform' && right.kind === 'uniform';
}

interface Ray { readonly origin: Vec3; readonly direction: Vec3 }
type Vec3 = readonly [number, number, number];
type Quaternion = readonly [number, number, number, number];

function pointerRay(
  camera: StudioTransformManipulatorCamera,
  pointer: readonly [number, number],
): Ray {
  const x = pointer[0] / camera.viewport.width * 2 - 1;
  const y = 1 - pointer[1] / camera.viewport.height * 2;
  const tangent = Math.tan(degreesToRadians(camera.fovYDegrees) / 2);
  const aspect = camera.viewport.width / camera.viewport.height;
  return {
    origin: camera.position,
    direction: normalize(add(camera.basis.forward, add(
      scale(camera.basis.right, x * tangent * aspect),
      scale(camera.basis.up, y * tangent),
    ))),
  };
}

function closestAxisParameter(
  rayOrigin: Vec3,
  rayDirection: Vec3,
  axisOrigin: Vec3,
  axis: Vec3,
): number | null {
  const offset = subtract(rayOrigin, axisOrigin);
  const a = dot(rayDirection, rayDirection);
  const b = dot(rayDirection, axis);
  const c = dot(axis, axis);
  const d = dot(rayDirection, offset);
  const e = dot(axis, offset);
  const denominator = a * c - b * b;
  if (Math.abs(denominator) <= EPSILON) return null;
  return (a * e - b * d) / denominator;
}

function rayPlaneIntersection(
  rayOrigin: Vec3,
  rayDirection: Vec3,
  planePoint: Vec3,
  planeNormal: Vec3,
): Vec3 | null {
  const denominator = dot(rayDirection, planeNormal);
  if (Math.abs(denominator) <= EPSILON) return null;
  const distance = dot(subtract(planePoint, rayOrigin), planeNormal) / denominator;
  return add(rayOrigin, scale(rayDirection, distance));
}

function pointerFallback(
  camera: StudioTransformManipulatorCamera,
  start: readonly [number, number],
  current: readonly [number, number],
  origin: Vec3,
): number {
  const pixelDistance = current[0] - start[0] - (current[1] - start[1]);
  const worldPerViewport = 2 * Math.tan(degreesToRadians(camera.fovYDegrees) / 2);
  return pixelDistance / Math.max(1, camera.viewport.height)
    * worldPerViewport * distance(camera.position, origin);
}

function applyAxisScale(
  source: Vec3,
  axis: StudioTransformAxis,
  factor: number,
): [number, number, number] {
  const next: [number, number, number] = [...source];
  next[axis] *= factor;
  return next;
}

function snapVector(
  value: Vec3,
  increment: Vec3,
  enabled: boolean,
): [number, number, number] {
  return value.map((component, axis) => snapScalar(
    component,
    increment[axis as StudioTransformAxis],
    enabled,
  )) as [number, number, number];
}

function snapScalar(value: number, increment: number, enabled: boolean): number {
  if (!enabled || increment <= EPSILON) return value;
  return Math.round(value / increment) * increment;
}

function clampScale(value: Vec3): [number, number, number] {
  return value.map((component) => Math.max(0.001, component)) as [number, number, number];
}

function validateTransform(transform: Transform): void {
  const values = [...transform.translation, ...transform.rotation, ...transform.scale];
  if (values.some((value) => !Number.isFinite(value))) {
    throw new Error('Studio transform manipulator source must be finite');
  }
  if (transform.scale.some((value) => value <= 0)) {
    throw new Error('Studio transform manipulator source scale must be positive');
  }
  if (Math.hypot(...transform.rotation) <= EPSILON) {
    throw new Error('Studio transform manipulator source rotation must be nonzero');
  }
}

function validateCamera(camera: StudioTransformManipulatorCamera): void {
  const values = [
    ...camera.position,
    ...camera.basis.forward,
    ...camera.basis.right,
    ...camera.basis.up,
    camera.fovYDegrees,
    camera.viewport.width,
    camera.viewport.height,
  ];
  if (values.some((value) => !Number.isFinite(value))) {
    throw new Error('Studio transform manipulator camera must be finite');
  }
  if (camera.viewport.width <= 0 || camera.viewport.height <= 0) {
    throw new Error('Studio transform manipulator viewport must be positive');
  }
  if (camera.fovYDegrees <= 0 || camera.fovYDegrees >= 180) {
    throw new Error('Studio transform manipulator fov must be in 0..180');
  }
}

function validateSnapping(snapping: StudioTransformSnapping): StudioTransformSnapping {
  const increments = [
    snapping.rotationDegrees,
    ...snapping.scale,
    ...snapping.translation,
  ];
  if (increments.some((value) => !Number.isFinite(value) || value <= 0)) {
    throw new Error('Studio transform manipulator snap increments must be positive and finite');
  }
  return {
    ...snapping,
    scale: [...snapping.scale],
    translation: [...snapping.translation],
  };
}

function requireIdentity(value: string, label: string): string {
  const trimmed = value.trim();
  if (trimmed.length === 0) throw new Error(`${label} must not be empty`);
  return trimmed;
}

function cloneTransform(transform: Transform): Transform {
  return {
    translation: [...transform.translation],
    rotation: [...transform.rotation],
    scale: [...transform.scale],
  };
}

function add(left: Vec3, right: Vec3): Vec3 {
  return [left[0] + right[0], left[1] + right[1], left[2] + right[2]];
}

function subtract(left: Vec3, right: Vec3): Vec3 {
  return [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
}

function scale(value: Vec3, scalar: number): Vec3 {
  return [value[0] * scalar, value[1] * scalar, value[2] * scalar];
}

function dot(left: Vec3, right: Vec3): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2];
}

function cross(left: Vec3, right: Vec3): Vec3 {
  return [
    left[1] * right[2] - left[2] * right[1],
    left[2] * right[0] - left[0] * right[2],
    left[0] * right[1] - left[1] * right[0],
  ];
}

function length(value: Vec3): number {
  return Math.hypot(...value);
}

function normalize(value: Vec3): Vec3 {
  const magnitude = length(value);
  if (magnitude <= EPSILON) return [0, 0, 0];
  return scale(value, 1 / magnitude);
}

function distance(left: Vec3, right: Vec3): number {
  return length(subtract(left, right));
}

function rotateVector(rotation: Quaternion, value: Vec3): Vec3 {
  const vector: Quaternion = [value[0], value[1], value[2], 0];
  const inverse: Quaternion = [-rotation[0], -rotation[1], -rotation[2], rotation[3]];
  const result = multiplyQuaternion(multiplyQuaternion(rotation, vector), inverse);
  return [result[0], result[1], result[2]];
}

function axisAngleQuaternion(axis: Vec3, radians: number): Quaternion {
  const half = radians / 2;
  const sine = Math.sin(half);
  return [axis[0] * sine, axis[1] * sine, axis[2] * sine, Math.cos(half)];
}

function multiplyQuaternion(left: Quaternion, right: Quaternion): Quaternion {
  const [ax, ay, az, aw] = left;
  const [bx, by, bz, bw] = right;
  return [
    aw * bx + ax * bw + ay * bz - az * by,
    aw * by - ax * bz + ay * bw + az * bx,
    aw * bz + ax * by - ay * bx + az * bw,
    aw * bw - ax * bx - ay * by - az * bz,
  ];
}

function normalizeQuaternion(value: Quaternion): Quaternion {
  const magnitude = Math.hypot(...value);
  if (magnitude <= EPSILON) return [0, 0, 0, 1];
  return value.map((component) => component / magnitude) as [number, number, number, number];
}

function quaternionFromUnitX(axis: Vec3): Quaternion {
  const unit = normalize(axis);
  const cosine = dot([1, 0, 0], unit);
  if (cosine > 1 - EPSILON) return [0, 0, 0, 1];
  if (cosine < -1 + EPSILON) return [0, 1, 0, 0];
  return normalizeQuaternion([0, -unit[2], unit[1], 1 + cosine]);
}

function degreesToRadians(degrees: number): number {
  return degrees * Math.PI / 180;
}
