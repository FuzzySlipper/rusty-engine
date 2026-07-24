/// <reference types="vite/client" />

import {
  renderHandle,
  type MeshPayloadDescriptor,
  type RenderFrameDiff,
} from '@rusty-engine/render-contracts';
import {
  MapAnimatedMeshAssetSource,
  loadAnimatedMeshGlbResource,
  mountRendererBrowserSurface,
} from '@rusty-engine/renderer-three';

import characterUrl from '../../fixtures/render/assets/kenney-retro-character/character-medium.glb?url';

interface BrowserProof {
  readonly animationClip: string | null;
  readonly context: string;
  readonly lightCount: number;
  readonly pickHandle: number | null;
  readonly projectionInsideViewport: boolean;
  readonly ready: true;
  readonly snapshot: string;
}

declare global {
  interface Window {
    __rustyRenderDispose?: () => void;
    __rustyRenderFailure?: string;
    __rustyRenderProof?: BrowserProof;
  }
}

const ASSET = 'mesh-animation/kenney-retro-character-medium';
const CONTENT_HASH = 'c71255a41c0373f0d2ef52593369d5fd9d2f6220ae548aff8cd6bf5edb403674';

async function main(): Promise<void> {
  const response = await fetch(characterUrl);
  if (!response.ok) throw new Error(`animated fixture fetch failed: ${String(response.status)}`);
  const resource = await loadAnimatedMeshGlbResource(
    ASSET,
    await response.arrayBuffer(),
    CONTENT_HASH,
  );
  const source = new MapAnimatedMeshAssetSource([resource]);
  const canvas = document.querySelector<HTMLCanvasElement>('#renderer');
  if (canvas === null) throw new Error('browser proof canvas is missing');

  const surface = mountRendererBrowserSurface(canvas, {
    animatedMeshSource: source,
    autoStart: false,
    frame: browserFrame(),
    pixelRatio: 1,
  });
  surface.renderOnce(16);
  const context = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
  if (context === null || context.isContextLost()) throw new Error('real WebGL context is unavailable');
  const pick = surface.pick({ ray: { kind: 'viewport', point: [0, 0] }, maxDistance: 20 });
  const projected = surface.projectWorldPoint([0, 0, -5]);

  window.__rustyRenderProof = {
    animationClip: surface.animatedMeshPlayback(renderHandle(105))?.currentClip ?? null,
    context: context instanceof WebGL2RenderingContext ? 'webgl2' : 'webgl',
    lightCount: surface.renderer.lightReadout().length,
    pickHandle: pick.hit?.handle ?? null,
    projectionInsideViewport: projected.insideViewport,
    ready: true,
    snapshot: surface.snapshot(),
  };
  window.__rustyRenderDispose = () => surface.dispose();
}

function browserFrame(): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineMaterial',
        material: {
          schemaVersion: 2,
          id: 'material/browser-proof',
          color: [0.25, 0.7, 0.9, 1],
          texture: null,
          roughness: 0.8,
          textureTint: [1, 1, 1, 1],
          emissionColor: [0, 0, 0],
          emissionIntensity: 0,
          uvStrategy: 'flat',
        },
      },
      {
        op: 'defineTexture',
        texture: {
          id: 'texture/browser-proof', width: 2, height: 2, filter: 'nearest',
          wrap: 'clamp', contentHash: null, version: 1,
        },
      },
      {
        op: 'defineSpriteAtlas',
        atlas: {
          id: 'sprite/browser-proof',
          texture: 'texture/browser-proof',
          frames: [{ frame: 0, uvMin: [0, 0], uvMax: [1, 1] }],
        },
      },
      {
        op: 'defineStaticMesh',
        asset: {
          asset: 'mesh/browser-proof',
          payload: trianglePayload(),
          materialSlots: [{ slot: 0, material: 'material/browser-proof' }],
          collision: { kind: 'visualOnly' },
        },
      },
      {
        op: 'defineAnimatedMesh',
        asset: {
          asset: ASSET,
          runtimeFormat: 'glb',
          contentHash: CONTENT_HASH,
          clips: [
            { id: 'idle', name: 'idle', durationSeconds: null },
            { id: 'run', name: 'run', durationSeconds: null },
            { id: 'jump', name: 'jump', durationSeconds: null },
          ],
          defaultClip: 'idle',
          materialSlots: [],
          bounds: { min: [-0.02, -0.01, 0], max: [0.02, 0.01, 0.04] },
        },
      },
      {
        op: 'create', handle: renderHandle(100), parent: null,
        node: {
          geometry: { kind: 'group' },
          material: { color: [1, 1, 1, 1], wireframe: false },
          transform: identity([0, 0, 0], [1, 1, 1]), visible: true, layer: 'scene',
          metadata: metadata('browser-root'),
        },
      },
      {
        op: 'create', handle: renderHandle(101), parent: renderHandle(100),
        node: {
          geometry: { kind: 'cube' },
          material: { color: [0.9, 0.4, 0.2, 1], wireframe: false },
          transform: identity([0, 1.62, -5], [1.5, 1.5, 1.5]), visible: true, layer: 'scene',
          metadata: metadata('pick-target', 101),
        },
      },
      {
        op: 'createStaticMeshInstance', handle: renderHandle(104), parent: null,
        instance: {
          asset: 'mesh/browser-proof', transform: identity([-2, 0, -4], [2, 2, 2]),
          visible: true, materialOverrides: [], metadata: metadata('static-proof'),
        },
      },
      {
        op: 'createAnimatedMeshInstance', handle: renderHandle(105), parent: null,
        instance: {
          asset: ASSET, transform: identity([2, 0, -5], [30, 30, 30]), visible: true,
          materialOverrides: [],
          playback: {
            kind: 'play', clip: 'run', loop: 'repeat', speed: 1,
            weight: 1, restart: true, fadeSeconds: null,
          },
          metadata: metadata('animated-proof'),
        },
      },
      {
        op: 'createSprite', handle: renderHandle(106), parent: null,
        sprite: {
          asset: 'sprite/browser-proof', frame: 0, pivot: [0.5, 0.5], size: [1, 1],
          sizeMode: 'world', billboard: 'none', tint: [1, 1, 0.2, 0.9], renderOrder: 3,
          depth: 'default', shading: 'unlit', visible: true,
          transform: identity([3, 2, -4], [1, 1, 1]),
          attachment: { sourceEntity: null, sourceSceneNode: null, attachmentPoint: null },
          metadata: metadata('sprite-proof'),
        },
      },
      {
        op: 'createLight', handle: renderHandle(107), parent: null,
        light: {
          kind: 'point', color: [1, 0.8, 0.6], intensity: 2, enabled: true,
          position: [0, 3, -2], range: 20, decay: 2, shadowIntent: 'disabled',
        },
      },
    ],
  };
}

function trianglePayload(): MeshPayloadDescriptor {
  return {
    layout: {
      vertexCount: 3, indexCount: 3, indexWidth: 'u32',
      attributes: [
        { name: 'position', components: 3, kind: 'f32' },
        { name: 'normal', components: 3, kind: 'f32' },
      ],
    },
    groups: [{ materialSlot: 0, start: 0, count: 3 }],
    bounds: { min: [-0.5, -0.5, 0], max: [0.5, 0.5, 0] },
    source: {
      kind: 'inline', positions: [-0.5, -0.5, 0, 0.5, -0.5, 0, 0, 0.5, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1], indices: [0, 1, 2],
    },
    provenance: 'staticAsset',
  };
}

function identity(
  translation: readonly [number, number, number],
  scale: readonly [number, number, number],
) {
  return { translation, rotation: [0, 0, 0, 1] as const, scale };
}

function metadata(label: string, sourceEntity: number | null = null) {
  return { sourceEntity, sourceSceneNode: null, tags: [] as string[], label };
}

void main().catch((error: unknown) => {
  window.__rustyRenderFailure = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
});
