import type * as THREE from 'three';

import { synchronizeCameraRelativeViewmodelCamera } from './viewmodel-camera.js';

export interface BrowserSurfaceRenderProjection {
  readonly scene: THREE.Scene;
  readonly viewmodelScene: THREE.Scene;
  advanceAnimation(deltaSeconds: number): void;
  prepareSpritesForCamera(camera: THREE.Camera, scene: THREE.Scene): void;
  prepareStaticInstanceBatches(camera: THREE.Camera): void;
}

export interface BrowserSurfaceRenderDriver {
  clear(color: boolean, depth: boolean, stencil: boolean): void;
  clearDepth(): void;
  render(scene: THREE.Scene, camera: THREE.Camera): void;
}

/**
 * Compose one browser-surface frame with an explicit after-world depth break.
 *
 * World and camera-relative presentation retain one renderer lifecycle and one
 * animation advance. The viewmodel camera is host-owned and never enters the
 * renderer-neutral contract.
 */
export function renderBrowserSurfaceFrame(
  driver: BrowserSurfaceRenderDriver,
  worldCamera: THREE.Camera,
  viewmodelCamera: THREE.PerspectiveCamera,
  projection: BrowserSurfaceRenderProjection,
  deltaSeconds: number,
): void {
  const aspect = 'aspect' in worldCamera && typeof worldCamera.aspect === 'number'
    ? worldCamera.aspect
    : viewmodelCamera.aspect;
  synchronizeCameraRelativeViewmodelCamera(worldCamera, viewmodelCamera, aspect);
  driver.clear(true, true, true);
  projection.advanceAnimation(deltaSeconds);
  projection.prepareSpritesForCamera(worldCamera, projection.scene);
  projection.prepareStaticInstanceBatches(worldCamera);
  driver.render(projection.scene, worldCamera);
  driver.clearDepth();
  projection.prepareSpritesForCamera(viewmodelCamera, projection.viewmodelScene);
  driver.render(projection.viewmodelScene, viewmodelCamera);
}
