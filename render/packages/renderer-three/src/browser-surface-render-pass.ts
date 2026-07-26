import type * as THREE from 'three';

export interface BrowserSurfaceRenderProjection {
  readonly scene: THREE.Scene;
  readonly viewmodelScene: THREE.Scene;
  advanceAnimation(deltaSeconds: number): void;
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
  viewmodelCamera: THREE.Camera,
  projection: BrowserSurfaceRenderProjection,
  deltaSeconds: number,
): void {
  driver.clear(true, true, true);
  projection.advanceAnimation(deltaSeconds);
  driver.render(projection.scene, worldCamera);
  driver.clearDepth();
  driver.render(projection.viewmodelScene, viewmodelCamera);
}
