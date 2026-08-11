import type * as THREE from 'three';

export interface RendererThreeCameraPose {
  readonly position: readonly [number, number, number];
  readonly pitchDegrees: number;
  readonly yawDegrees: number;
}

const DEGREES_TO_RADIANS = Math.PI / 180;

/**
 * Applies Engine's canonical yaw/pitch convention to a Three camera.
 *
 * Engine yaw zero faces -Z and positive yaw turns toward +X. Three's positive
 * Y rotation turns its camera toward -X, so this backend-local conversion is
 * intentionally sign-inverted. Downstream callers remain renderer-neutral.
 */
export function applyRendererThreeCameraPose(
  camera: THREE.Camera,
  pose: RendererThreeCameraPose,
): void {
  camera.position.set(...pose.position);
  camera.up.set(0, 1, 0);
  camera.rotation.order = 'YXZ';
  camera.rotation.x = pose.pitchDegrees * DEGREES_TO_RADIANS;
  camera.rotation.y = -pose.yawDegrees * DEGREES_TO_RADIANS;
  camera.rotation.z = 0;
}
