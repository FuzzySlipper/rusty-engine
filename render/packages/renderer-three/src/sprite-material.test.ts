import assert from 'node:assert/strict';
import { test } from 'node:test';
import * as THREE from 'three';

import type { SpriteInstanceDescriptor } from '@rusty-engine/render-contracts';
import {
  createSpriteMaterial,
  resolveSpriteMaterialDescriptor,
  spriteMaterialVariantKey,
  updateSpriteMaterialTint,
} from './sprite-material.js';

function sprite(over: Partial<SpriteInstanceDescriptor> = {}): SpriteInstanceDescriptor {
  return {
    asset: 'sprite/test',
    frame: 0,
    pivot: [0.5, 0.5],
    size: [1, 1],
    sizeMode: 'world',
    billboard: 'spherical',
    tint: [1, 1, 1, 1],
    renderOrder: 0,
    depth: 'default',
    shading: 'unlit',
    visible: true,
    transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
    attachment: { sourceEntity: null, sourceSceneNode: null, attachmentPoint: null },
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'test' },
    ...over,
  };
}

function texture(): THREE.DataTexture {
  return new THREE.DataTexture(new Uint8Array([255, 255, 255, 255]), 1, 1);
}

void test('legacy sprite shading resolves without admitting custom shader source', () => {
  assert.equal(resolveSpriteMaterialDescriptor(sprite()).lighting, 'unlit');
  assert.equal(resolveSpriteMaterialDescriptor(sprite({ shading: 'lit' })).lighting, 'synthetic');
  assert.equal(resolveSpriteMaterialDescriptor(sprite({ shading: 'shadowed' })).shadow, 'castAndReceive');
  assert.equal(resolveSpriteMaterialDescriptor(sprite({ shading: 'custom' })).lighting, 'unlit');
});

void test('bounded sprite modes select stock Three material features and alpha policy', () => {
  const color = texture();
  const normal = texture();
  const depth = texture();
  const normalResult = createSpriteMaterial(sprite({
    material: {
      lighting: 'authoredNormal', normalTexture: 'texture/normal', depthTexture: null,
      normalStrength: 1.5, normalBias: 0, alpha: { kind: 'mask', cutoff: 0.4 },
      shadow: 'castAndReceive',
    },
  }), { color, normal, depth: null });
  assert.ok(normalResult.material instanceof THREE.MeshStandardMaterial);
  assert.equal(normalResult.material.normalMap, normal);
  assert.equal(normalResult.material.alphaTest, 0.4);
  assert.equal(normalResult.material.transparent, false);
  assert.equal(normalResult.castShadow, true);
  assert.equal(normalResult.receiveShadow, true);

  const depthResult = createSpriteMaterial(sprite({
    material: {
      lighting: 'authoredDepth', normalTexture: null, depthTexture: 'texture/depth',
      normalStrength: 2, normalBias: 0.5, alpha: { kind: 'blend' }, shadow: 'none',
    },
  }), { color, normal: null, depth });
  assert.ok(depthResult.material instanceof THREE.MeshStandardMaterial);
  assert.equal(depthResult.material.bumpMap, depth);
  assert.equal(depthResult.material.transparent, true);
  assert.equal(depthResult.material.depthWrite, false);

  const derived = createSpriteMaterial(sprite({
    material: {
      lighting: 'derivedGradient', normalTexture: null, depthTexture: null,
      normalStrength: 1, normalBias: 0, alpha: { kind: 'opaque' }, shadow: 'receive',
    },
  }), { color, normal: null, depth: null });
  assert.ok(derived.material instanceof THREE.MeshStandardMaterial);
  assert.equal(derived.material.bumpMap, color);
  assert.equal(derived.receiveShadow, true);
});

void test('synthetic normals share one shader variant while tint remains per instance', () => {
  const result = createSpriteMaterial(sprite({
    material: {
      lighting: 'synthetic', normalTexture: null, depthTexture: null,
      normalStrength: 0.8, normalBias: 0.2, alpha: { kind: 'blend' }, shadow: 'none',
    },
  }), { color: texture(), normal: null, depth: null });
  assert.ok(result.material instanceof THREE.MeshStandardMaterial);
  assert.equal(result.material.customProgramCacheKey(), 'rusty-sprite-synthetic-v1');
  updateSpriteMaterialTint(result.material, sprite({
    tint: [0.2, 0.4, 0.6, 0.5],
    material: result.descriptor,
  }));
  assert.deepEqual(result.material.color.toArray(), [0.2, 0.4, 0.6]);
  assert.equal(result.material.opacity, 0.5);
  assert.equal(result.material.transparent, true);
});

void test('material variant identity excludes mutable per-instance tint and texture identity', () => {
  const material = {
    lighting: 'authoredNormal' as const,
    normalTexture: 'texture/normal-a',
    depthTexture: null,
    normalStrength: 1,
    normalBias: 0,
    alpha: { kind: 'mask' as const, cutoff: 0.4 },
    shadow: 'castAndReceive' as const,
  };
  const first = sprite({ material });
  const second = sprite({
    tint: [0.2, 0.4, 0.6, 0.5],
    material: { ...material, normalTexture: 'texture/normal-b', normalStrength: 2 },
  });
  assert.equal(spriteMaterialVariantKey(first), spriteMaterialVariantKey(second));
  assert.notEqual(
    spriteMaterialVariantKey(first),
    spriteMaterialVariantKey(sprite({ ...first, depth: 'depthTestOff' })),
  );
});
