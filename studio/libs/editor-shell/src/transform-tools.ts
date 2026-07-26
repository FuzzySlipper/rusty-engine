import type { Transform } from '@rusty-engine/render-contracts';

/** Resolves one local TRS against its parent for renderer-facing world presentation. */
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

/** Converts a renderer-facing world candidate back to the selected Rust owner's local TRS. */
export function localTransformFromWorld(parent: Transform | null, world: Transform): Transform {
  if (parent === null) return cloneTransform(world);
  return {
    translation: inverseTransformPoint(parent, world.translation),
    rotation: multiplyQuaternion(inverseQuaternion(parent.rotation), world.rotation),
    scale: [
      world.scale[0] / parent.scale[0],
      world.scale[1] / parent.scale[1],
      world.scale[2] / parent.scale[2],
    ],
  };
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
  return value.map((component) => component / length) as [number, number, number, number];
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

function cloneTransform(transform: Transform): Transform {
  return {
    translation: [...transform.translation],
    rotation: [...transform.rotation],
    scale: [...transform.scale],
  };
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
