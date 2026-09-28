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
 * Begin one browser-surface frame: clear the canvas and advance animation once.
 * Unless a view composition owns the primary output, also draw the fallback
 * world and then camera-relative presentation after an explicit depth break.
 *
 * When the composition owns the primary output it draws every primary view
 * itself; canvas area no primary view covers keeps this clear. The viewmodel
 * camera is host-owned and never enters the renderer-neutral contract.
 */
export function renderBrowserSurfaceFrame(
  driver: BrowserSurfaceRenderDriver,
  worldCamera: THREE.Camera,
  viewmodelCamera: THREE.PerspectiveCamera,
  projection: BrowserSurfaceRenderProjection,
  deltaSeconds: number,
  compositionOwnsPrimaryOutput = false,
): void {
  driver.clear(true, true, true);
  projection.advanceAnimation(deltaSeconds);
  if (compositionOwnsPrimaryOutput) return;
  const aspect = 'aspect' in worldCamera && typeof worldCamera.aspect === 'number'
    ? worldCamera.aspect
    : viewmodelCamera.aspect;
  synchronizeCameraRelativeViewmodelCamera(worldCamera, viewmodelCamera, aspect);
  projection.prepareSpritesForCamera(worldCamera, projection.scene);
  projection.prepareStaticInstanceBatches(worldCamera);
  driver.render(projection.scene, worldCamera);
  driver.clearDepth();
  projection.prepareSpritesForCamera(viewmodelCamera, projection.viewmodelScene);
  driver.render(projection.viewmodelScene, viewmodelCamera);
}
