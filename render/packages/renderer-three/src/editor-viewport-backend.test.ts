import { test } from 'node:test';
import assert from 'node:assert/strict';

import { renderHandle, type AnimatedMeshAsset, type RenderFrameDiff, type RenderNode } from '@rusty-engine/render-contracts';
import { RendererEditorProjectionChannels } from './editor-viewport-backend.js';

function primitiveNode(label: string): RenderNode {
  return {
    geometry: { kind: 'cube' },
    material: { color: [0.2, 0.4, 0.6, 1], wireframe: false },
    transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    visible: true,
    layer: 'scene',
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label },
  };
}

function primitiveFrame(handle: number, label: string): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [{
      op: 'create',
      handle: renderHandle(handle),
      parent: null,
      node: primitiveNode(label),
    }],
  };
}

function missingAnimatedResourceFrame(): RenderFrameDiff {
  const asset: AnimatedMeshAsset = {
    asset: 'mesh-animation/missing-editor-preview',
    runtimeFormat: 'glb',
    contentHash: 'sha256:missing',
    clips: [],
    defaultClip: null,
    materialSlots: [],
    bounds: { min: [-0.5, 0, -0.5], max: [0.5, 1.8, 0.5] },
  };
  return { schemaVersion: 1, ops: [{ op: 'defineAnimatedMesh', asset }] };
}

void test('real projection channels isolate equal handles in fixed runtime authored and overlay cells', () => {
  const channels = new RendererEditorProjectionChannels();
  channels.replace('runtime', primitiveFrame(7, 'runtime'));
  channels.replace('authored', primitiveFrame(7, 'authored'));
  channels.replace('overlay', primitiveFrame(7, 'overlay'));

  assert.equal(channels.renderer('runtime').objectFor(renderHandle(7))?.name, 'runtime');
  assert.equal(channels.renderer('authored').objectFor(renderHandle(7))?.name, 'authored');
  assert.equal(channels.renderer('overlay').objectFor(renderHandle(7))?.name, 'overlay');
  assert.match(channels.snapshot(), /^\[runtime\]/);
  assert.match(channels.snapshot(), /\[authored\]/);
  assert.match(channels.snapshot(), /\[overlay\]/);

  channels.dispose();
});

void test('real projection channel replacement is atomic and does not disturb sibling channels', () => {
  const channels = new RendererEditorProjectionChannels();
  channels.replace('runtime', primitiveFrame(1, 'runtime-stable'));
  channels.replace('authored', primitiveFrame(2, 'authored-stable'));
  const runtimeBefore = channels.renderer('runtime');
  const authoredBefore = channels.renderer('authored');

  assert.throws(
    () => channels.replace('authored', {
      schemaVersion: 1,
      ops: [{
        op: 'update',
        handle: renderHandle(999),
        transform: null,
        material: null,
        visible: false,
        metadata: null,
      }],
    }),
    /unknown handle 999/,
  );

  assert.equal(channels.renderer('runtime'), runtimeBefore);
  assert.equal(channels.renderer('authored'), authoredBefore);
  assert.equal(channels.renderer('runtime').objectFor(renderHandle(1))?.name, 'runtime-stable');
  assert.equal(channels.renderer('authored').objectFor(renderHandle(2))?.name, 'authored-stable');

  channels.dispose();
  assert.equal(runtimeBefore.handleCount, 0);
  assert.equal(authoredBefore.handleCount, 0);
});

void test('one missing editor resource fails closed without degrading healthy runtime projection', () => {
  const channels = new RendererEditorProjectionChannels();
  channels.replace('runtime', primitiveFrame(3, 'runtime-healthy'));
  const runtimeBefore = channels.renderer('runtime');
  const authoredBefore = channels.renderer('authored');

  assert.throws(
    () => channels.replace('authored', missingAnimatedResourceFrame()),
    /missing animated mesh resource mesh-animation\/missing-editor-preview/,
  );

  assert.equal(channels.renderer('runtime'), runtimeBefore);
  assert.equal(channels.renderer('authored'), authoredBefore);
  assert.equal(channels.renderer('runtime').objectFor(renderHandle(3))?.name, 'runtime-healthy');
  assert.equal(channels.renderer('authored').handleCount, 0);
  channels.dispose();
});
