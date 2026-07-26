import type { Transform } from '@rusty-engine/render-contracts';
import type {
  StudioTransformAxis,
  StudioTransformOrientation,
  StudioTransformTool,
} from '@rusty-engine/studio-viewport';

export interface TransformToolSettings {
  readonly snappingEnabled: boolean;
  readonly translationSnapAxes: readonly [number, number, number];
  readonly rotationSnapDegrees: number;
  readonly scaleSnapAxes: readonly [number, number, number];
  readonly fineMultiplier: number;
}

export interface TransformToolDelta {
  readonly local: Transform;
  readonly world: Transform;
  readonly parentWorld: Transform | null;
  readonly tool: StudioTransformTool;
  readonly orientation: StudioTransformOrientation;
  readonly axis: StudioTransformAxis;
  readonly delta: number;
  readonly fine: boolean;
  readonly toggleSnap: boolean;
  readonly settings: TransformToolSettings;
}

export function composeTransform(parent: Transform, local: Transform): Transform {
  const scaledLocal: readonly [number, number, number] = [
    local.translation[0] * parent.scale[0],
    local.translation[1] * parent.scale[1],
    local.translation[2] * parent.scale[2],
  ];
  return {
    translation: add(parent.translation, rotateVector(parent.rotation, scaledLocal)),
    rotation: multiplyQuaternion(parent.rotation, local.rotation),
    scale: [
      parent.scale[0] * local.scale[0],
      parent.scale[1] * local.scale[1],
      parent.scale[2] * local.scale[2],
    ],
  };
}

/**
 * Applies one disposable gizmo delta and returns the local transform that the
 * named Rust owner can later validate and commit. Hierarchy conversion remains
 * explicit; the helper never mutates accepted project state.
 */
export function applyTransformToolDelta(input: TransformToolDelta): Transform {
  const fineMultiplier = input.fine ? input.settings.fineMultiplier : 1;
  const delta = input.delta * fineMultiplier;
  const snapping = input.toggleSnap
    ? !input.settings.snappingEnabled
    : input.settings.snappingEnabled;

  switch (input.tool) {
    case 'translate': {
      const step = input.settings.translationSnapAxes[input.axis] * fineMultiplier;
      const direction = input.orientation === 'world'
        ? unitAxis(input.axis)
        : rotateVector(input.world.rotation, unitAxis(input.axis));
      const currentAlongAxis = dot(input.world.translation, direction);
      const targetAlongAxis = snapping
        ? quantize(currentAlongAxis + delta, step)
        : currentAlongAxis + delta;
      const worldTranslation = add(
        input.world.translation,
        multiply(direction, targetAlongAxis - currentAlongAxis),
      );
      return {
        ...input.local,
        translation: input.parentWorld === null
          ? worldTranslation
          : inverseTransformPoint(input.parentWorld, worldTranslation),
      };
    }
    case 'rotate': {
      const step = input.settings.rotationSnapDegrees * fineMultiplier;
      const degrees = snapping ? quantize(delta, step) : delta;
      const deltaRotation = axisAngle(unitAxis(input.axis), degrees * Math.PI / 180);
      const worldRotation = input.orientation === 'world'
        ? multiplyQuaternion(deltaRotation, input.world.rotation)
        : multiplyQuaternion(input.world.rotation, deltaRotation);
      return {
        ...input.local,
        rotation: input.parentWorld === null
          ? worldRotation
          : multiplyQuaternion(inverseQuaternion(input.parentWorld.rotation), worldRotation),
      };
    }
    case 'scale': {
      const step = input.settings.scaleSnapAxes[input.axis] * fineMultiplier;
      if (input.orientation === 'local') {
        const scale = [...input.local.scale] as [number, number, number];
        scale[input.axis] = Math.max(Number.EPSILON, snapping
          ? quantize(scale[input.axis] + delta, step)
          : scale[input.axis] + delta);
        return { ...input.local, scale };
      }
      // The persisted hierarchy is TRS-only. World-axis scale therefore uses
      // the hierarchy's component-wise world scale representation; arbitrary
      // shear is deliberately not invented by the editor.
      const worldScale = [...input.world.scale] as [number, number, number];
      worldScale[input.axis] = Math.max(Number.EPSILON, snapping
        ? quantize(worldScale[input.axis] + delta, step)
        : worldScale[input.axis] + delta);
      return {
        ...input.local,
        scale: input.parentWorld === null
          ? worldScale
          : [
              worldScale[0] / input.parentWorld.scale[0],
              worldScale[1] / input.parentWorld.scale[1],
              worldScale[2] / input.parentWorld.scale[2],
            ],
      };
    }
  }
}

function inverseTransformPoint(
  transform: Transform,
  worldPoint: readonly [number, number, number],
): readonly [number, number, number] {
  const offset = subtract(worldPoint, transform.translation);
  const unrotated = rotateVector(inverseQuaternion(transform.rotation), offset);
  return [
    unrotated[0] / transform.scale[0],
    unrotated[1] / transform.scale[1],
    unrotated[2] / transform.scale[2],
  ];
}

function unitAxis(axis: StudioTransformAxis): readonly [number, number, number] {
  return axis === 0 ? [1, 0, 0] : axis === 1 ? [0, 1, 0] : [0, 0, 1];
}

function axisAngle(
  axis: readonly [number, number, number],
  radians: number,
): readonly [number, number, number, number] {
  const half = radians / 2;
  const sine = Math.sin(half);
  return [axis[0] * sine, axis[1] * sine, axis[2] * sine, Math.cos(half)];
}

function multiplyQuaternion(
  left: readonly [number, number, number, number],
  right: readonly [number, number, number, number],
): readonly [number, number, number, number] {
  const [ax, ay, az, aw] = left;
  const [bx, by, bz, bw] = right;
  return normalizeQuaternion([
    aw * bx + ax * bw + ay * bz - az * by,
    aw * by - ax * bz + ay * bw + az * bx,
    aw * bz + ax * by - ay * bx + az * bw,
    aw * bw - ax * bx - ay * by - az * bz,
  ]);
}

function inverseQuaternion(
  value: readonly [number, number, number, number],
): readonly [number, number, number, number] {
  const squared = value.reduce((sum, component) => sum + component * component, 0);
  return [-value[0] / squared, -value[1] / squared, -value[2] / squared, value[3] / squared];
}

function normalizeQuaternion(
  value: readonly [number, number, number, number],
): readonly [number, number, number, number] {
  const length = Math.hypot(...value);
  return value.map((component) => component / length) as unknown as readonly [number, number, number, number];
}

function rotateVector(
  rotation: readonly [number, number, number, number],
  vector: readonly [number, number, number],
): readonly [number, number, number] {
  const [x, y, z, w] = rotation;
  const tx = 2 * (y * vector[2] - z * vector[1]);
  const ty = 2 * (z * vector[0] - x * vector[2]);
  const tz = 2 * (x * vector[1] - y * vector[0]);
  return [
    vector[0] + w * tx + (y * tz - z * ty),
    vector[1] + w * ty + (z * tx - x * tz),
    vector[2] + w * tz + (x * ty - y * tx),
  ];
}

function quantize(value: number, step: number): number {
  return Math.round(value / step) * step;
}

function dot(
  left: readonly [number, number, number],
  right: readonly [number, number, number],
): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2];
}

function add(
  left: readonly [number, number, number],
  right: readonly [number, number, number],
): readonly [number, number, number] {
  return [left[0] + right[0], left[1] + right[1], left[2] + right[2]];
}

function subtract(
  left: readonly [number, number, number],
  right: readonly [number, number, number],
): readonly [number, number, number] {
  return [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
}

function multiply(
  vector: readonly [number, number, number],
  scalar: number,
): readonly [number, number, number] {
  return [vector[0] * scalar, vector[1] * scalar, vector[2] * scalar];
}
