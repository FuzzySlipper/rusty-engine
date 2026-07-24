import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';

import { renderHandle, type RenderFrameDiff } from '@rusty-engine/render-contracts';
import {
  RUSTY_RENDERER_HOST_COMPATIBILITY_VERSION,
  RUSTY_RENDERER_INSPECTION_SURFACE_COMPATIBILITY_VERSION,
  RendererHostError,
  createRendererAnimatedMeshProjection,
  createRendererDefaultSurfaceFrame,
  createRendererSurfaceProjection,
  resolveRendererStoredEditorCamera,
  type RendererAnimatedMeshResourceManifest,
} from './index.js';

const ANIMATED_ASSET = 'mesh-animation/kenney-retro-character-medium';
const ANIMATED_HASH = 'sha256:c71255a41c0373f0d2ef52593369d5fd9d2f6220ae548aff8cd6bf5edb403674';
const ANIMATED_FIXTURE = resolve(
  import.meta.dirname,
  '../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb',
);

const ANIMATED_MANIFEST: RendererAnimatedMeshResourceManifest = {
  kind: 'rusty_renderer_animated_mesh_resources.v1',
  resources: [{
    asset: ANIMATED_ASSET,
    contentHash: ANIMATED_HASH,
    clipIds: ['idle', 'run', 'jump'],
  }],
};

function animationIntentFrame(clip = 'run'): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineAnimatedMesh',
        asset: {
          asset: ANIMATED_ASSET,
          runtimeFormat: 'glb',
          contentHash: ANIMATED_HASH,
          clips: [
            { id: 'idle', name: 'Idle', durationSeconds: 1.04166662693024 },
            { id: 'run', name: 'Run', durationSeconds: 0.666666686534882 },
            { id: 'jump', name: 'Jump', durationSeconds: 0.5 },
          ],
          defaultClip: 'idle',
          materialSlots: [],
          bounds: { min: [-0.02, -0.01, 0], max: [0.02, 0.01, 0.04] },
        },
      },
      {
        op: 'createAnimatedMeshInstance',
        handle: renderHandle(4100),
        parent: null,
        instance: {
          asset: ANIMATED_ASSET,
          transform: {
            translation: [0, 0, -2.5],
            rotation: [0, 0, 0, 1],
            scale: [40, 40, 40],
          },
          materialOverrides: [],
          playback: null,
          visible: true,
          metadata: {
            sourceEntity: null,
            sourceSceneNode: null,
            tags: [],
            label: 'animated enemy visual',
          },
        },
      },
      {
        op: 'setAnimatedMeshPlayback',
        handle: renderHandle(4100),
        playback: {
          kind: 'play',
          clip,
          loop: 'repeat',
          speed: 1,
          weight: 1,
          restart: false,
          fadeSeconds: 0.1,
        },
      },
    ],
  };
}

function fixtureResolver(): Promise<ArrayBuffer> {
  const bytes = readFileSync(ANIMATED_FIXTURE);
  return Promise.resolve(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
}

function fnv1a64(data: ArrayBuffer): string {
  let hash = 0xcbf29ce484222325n;
  for (const byte of new Uint8Array(data)) {
    hash ^= BigInt(byte);
    hash = BigInt.asUintN(64, hash * 0x100000001b3n);
  }
  return hash.toString(16).padStart(16, '0');
}

void test('renderer-host projects render frames through the neutral projection model', () => {
  const frame: RenderFrameDiff = {
    schemaVersion: 1,
    ops: [{
      op: 'create',
      handle: renderHandle(4385001),
      parent: null,
      node: {
        layer: 'scene',
        geometry: { kind: 'cube' },
        transform: {
          translation: [0, 0, 0],
          rotation: [0, 0, 0, 1],
          scale: [1, 1, 1],
        },
        material: { color: [0.2, 0.4, 0.6, 1], wireframe: false },
        visible: true,
        metadata: {
          sourceEntity: null,
          sourceSceneNode: null,
          tags: [],
          label: 'renderer-host-neutral-cube',
        },
      },
    }],
  };

  const receipt = createRendererSurfaceProjection(frame);
  assert.equal(RUSTY_RENDERER_HOST_COMPATIBILITY_VERSION, 'renderer-host.v1');
  assert.equal(receipt.instructions.length, 1);
  assert.equal(receipt.snapshot.nodes.length, 1);
  assert.equal(receipt.snapshot.nodes[0]?.handle, 4385001);
});

void test('renderer-host creates a visible default surface frame', () => {
  const frame = createRendererDefaultSurfaceFrame();
  assert.equal(frame.schemaVersion, 1);
  assert.ok(frame.ops.some((op) => op.op === 'create'));
});

void test('renderer-host exposes backend-neutral stored editor camera resolution', () => {
  const result = resolveRendererStoredEditorCamera({
    position: [0, 0, 5],
    target: [0, 0, 0],
    up: [0, 1, 0],
    projection: { fovYDegrees: 55, near: 0.05, far: 1000 },
  });
  assert.equal(result.ok, true);
});

void test('animated mesh projection loads the committed GLB and advances selected playback', async () => {
  const restore = installGltfNodeGlobals();
  try {
    const projection = await createRendererAnimatedMeshProjection({
      manifest: ANIMATED_MANIFEST,
      resolveResource: fixtureResolver,
    });
    assert.equal(projection.applyFrame(animationIntentFrame()).applied, true);
    const selected = projection.playback(renderHandle(4100));
    assert.equal(selected.selectedClip, 'run');
    assert.equal(selected.contentHash, ANIMATED_HASH);
    assert.equal(selected.status, 'playing');
    assert.equal(selected.commandSelected, true);
    assert.deepEqual(selected.diagnostics, []);

    assert.equal(projection.advance(0.25).applied, true);
    const advanced = projection.playback(renderHandle(4100));
    assert.ok(advanced.mixerTimeSeconds > selected.mixerTimeSeconds);
    assert.ok((advanced.actionTimeSeconds ?? 0) > (selected.actionTimeSeconds ?? 0));
    assert.notDeepEqual(
      advanced.poseSample?.hierarchyRotationSum,
      selected.poseSample?.hierarchyRotationSum,
    );
  } finally {
    restore();
  }
});

void test('animated mesh projection accepts the manifest-native FNV content hash', async () => {
  const restore = installGltfNodeGlobals();
  try {
    const data = await fixtureResolver();
    const manifest: RendererAnimatedMeshResourceManifest = {
      ...ANIMATED_MANIFEST,
      resources: ANIMATED_MANIFEST.resources.map((resource) => ({
        ...resource,
        contentHash: fnv1a64(data),
      })),
    };
    assert.ok(await createRendererAnimatedMeshProjection({
      manifest,
      resolveResource: () => Promise.resolve(data),
    }));
  } finally {
    restore();
  }
});

void test('animated mesh projection permits a static GLTF resource with no required clips', async () => {
  const restore = installGltfNodeGlobals();
  try {
    const manifest: RendererAnimatedMeshResourceManifest = {
      ...ANIMATED_MANIFEST,
      resources: ANIMATED_MANIFEST.resources.map((resource) => ({ ...resource, clipIds: [] })),
    };
    assert.ok(await createRendererAnimatedMeshProjection({
      manifest,
      resolveResource: fixtureResolver,
    }));
  } finally {
    restore();
  }
});

void test('animated resources and playback fail closed with typed diagnostics', async () => {
  const restore = installGltfNodeGlobals();
  try {
    const badManifest: RendererAnimatedMeshResourceManifest = {
      ...ANIMATED_MANIFEST,
      resources: ANIMATED_MANIFEST.resources.map((resource) => ({
        ...resource,
        contentHash: `sha256:${'0'.repeat(64)}`,
      })),
    };
    await assert.rejects(
      createRendererAnimatedMeshProjection({ manifest: badManifest, resolveResource: fixtureResolver }),
      (error: unknown) => error instanceof RendererHostError
        && error.diagnostics[0]?.code === 'animated_mesh_content_hash_mismatch',
    );

    const projection = await createRendererAnimatedMeshProjection({
      manifest: ANIMATED_MANIFEST,
      resolveResource: fixtureResolver,
    });
    const unavailable = projection.playback(renderHandle(999));
    assert.equal(unavailable.status, 'unavailable');
    assert.equal(unavailable.contentHash, null);
    assert.equal(unavailable.diagnostics[0]?.code, 'animated_mesh_handle_unavailable');
    const rejected = projection.applyFrame(animationIntentFrame('missing'));
    assert.equal(rejected.applied, false);
    assert.equal(rejected.diagnostics[0]?.code, 'animated_mesh_frame_rejected');
  } finally {
    restore();
  }
});

void test('renderer-host declarations expose no concrete backend or donor runtime types', () => {
  const declarationText = declaration('./index.d.ts');
  const surfaceDeclarationText = declaration('./surface.d.ts');
  const editorDeclarationText = declaration('./editor-viewport.d.ts');
  const inspectionDeclarationText = declaration('./inspection-surface.d.ts');
  const all = [declarationText, surfaceDeclarationText, editorDeclarationText, inspectionDeclarationText].join('\n');

  assert.doesNotMatch(all, new RegExp(`@${'asha'}/`));
  assert.doesNotMatch(all, new RegExp(['runtime', 'bridge'].join('-')));
  assert.doesNotMatch(all, /ThreeRenderer|WebGLRenderer|from ['"]three['"]/);
  assert.doesNotMatch(
    all,
    new RegExp([
      ['Runtime', 'ProjectionFrame'].join(''),
      ['replay', 'Scope'].join(''),
      ['authority', 'Tick'].join(''),
    ].join('|')),
  );
  assert.match(declarationText, /mountRendererInspectionSurface/);
  assert.equal(RUSTY_RENDERER_INSPECTION_SURFACE_COMPATIBILITY_VERSION, 'inspection-surface.v1');
  assert.match(editorDeclarationText, /RendererEditorViewportChannelHandle/);
  assert.match(editorDeclarationText, /readonly replaceChunks:/);
  assert.match(inspectionDeclarationText, /projection_only_inspection/);
  assert.match(inspectionDeclarationText, /readonly applyRuntimeFrame:/);
  assert.match(inspectionDeclarationText, /readonly replaceAuthoredFrameChunks:/);
  assert.match(inspectionDeclarationText, /readonly setGrid:/);
  assert.match(surfaceDeclarationText, /RendererSurfacePickRequest/);
  assert.match(surfaceDeclarationText, /RendererSurfaceMovementResolver/);
  assert.match(surfaceDeclarationText, /readonly applyPresentation:/);
  assert.doesNotMatch(surfaceDeclarationText, /inputSession|movementAuthority/);
});

function declaration(relative: string): string {
  return readFileSync(fileURLToPath(new URL(relative, import.meta.url)), 'utf8');
}

function installGltfNodeGlobals(): () => void {
  const globals = globalThis as unknown as { self: unknown };
  const previousSelf = globals.self;
  const previousWarn = console.warn;
  const previousError = console.error;
  globals.self = globalThis;
  console.warn = () => undefined;
  console.error = () => undefined;
  return () => {
    globals.self = previousSelf;
    console.warn = previousWarn;
    console.error = previousError;
  };
}
