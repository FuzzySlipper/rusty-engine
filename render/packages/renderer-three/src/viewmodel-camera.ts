import * as THREE from 'three';

/** Realize camera-relative presentation in canonical camera-local coordinates. */
export function synchronizeCameraRelativeViewmodelCamera(
  worldCamera: THREE.Camera,
  viewmodelCamera: THREE.PerspectiveCamera,
  aspect: number,
): void {
  viewmodelCamera.position.set(0, 0, 0);
  viewmodelCamera.quaternion.identity();
  viewmodelCamera.up.set(0, 1, 0);
  if (worldCamera instanceof THREE.PerspectiveCamera) {
    viewmodelCamera.fov = worldCamera.fov;
    viewmodelCamera.near = worldCamera.near;
    viewmodelCamera.far = worldCamera.far;
  }
  viewmodelCamera.aspect = aspect;
  viewmodelCamera.updateProjectionMatrix();
  viewmodelCamera.updateMatrixWorld(true);
}
