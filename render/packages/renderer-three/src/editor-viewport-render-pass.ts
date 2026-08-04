import type * as THREE from 'three';

type SceneDepthChannel = 'runtime' | 'authored';
type EditorViewportRenderChannel = SceneDepthChannel | 'overlay';

interface EditorViewportRenderChannelProjection {
  readonly scene: THREE.Scene;
  advanceAnimation(deltaSeconds: number): void;
  prepareSpritesForCamera(camera: THREE.Camera, scene: THREE.Scene): void;
  prepareStaticInstanceBatches(camera: THREE.Camera): void;
}

export interface EditorViewportRenderChannels {
  renderer(channel: EditorViewportRenderChannel): EditorViewportRenderChannelProjection;
}

export interface EditorViewportRenderDriver {
  clear(color: boolean, depth: boolean, stencil: boolean): void;
  clearDepth(): void;
  render(scene: THREE.Scene, camera: THREE.Camera): void;
}

const SCENE_DEPTH_CHANNELS: readonly SceneDepthChannel[] = ['runtime', 'authored'];

/** Internal realization order; public consumers remain backend-neutral. */
export function renderEditorViewportFrame(
  driver: EditorViewportRenderDriver,
  camera: THREE.Camera,
  gridScene: THREE.Scene,
  channels: EditorViewportRenderChannels,
  deltaSeconds: number,
): void {
  driver.clear(true, true, true);
  for (const channel of SCENE_DEPTH_CHANNELS) {
    const renderer = channels.renderer(channel);
    renderer.advanceAnimation(deltaSeconds);
    renderer.prepareSpritesForCamera(camera, renderer.scene);
    renderer.prepareStaticInstanceBatches(camera);
    driver.render(renderer.scene, camera);
  }

  driver.render(gridScene, camera);

  const overlay = channels.renderer('overlay');
  overlay.advanceAnimation(deltaSeconds);
  overlay.prepareSpritesForCamera(camera, overlay.scene);
  overlay.prepareStaticInstanceBatches(camera);
  driver.clearDepth();
  driver.render(overlay.scene, camera);
}
