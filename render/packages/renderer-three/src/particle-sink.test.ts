import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from 'three';

import {
  RendererThreeParticleSink,
  type RendererThreeParticleInstance,
} from './particle-sink.js';

function cube(id: number, x = id): RendererThreeParticleInstance {
  return {
    id,
    position: [x, 2, 3],
    size: 0.25,
    color: [1, 0.5, 0.25, 1],
    frameIndex: 0,
    visual: { kind: 'cube' },
  };
}

function billboard(id: number): RendererThreeParticleInstance {
  return {
    id,
    position: [0, 1, -2],
    size: 0.5,
    color: [1, 1, 1, 1],
    frameIndex: 2,
    visual: { kind: 'billboard', frameCount: 4, spriteUrl: '/sparks.png' },
  };
}

void test('Three particle sink pools billboards and instanced cubes with bounded batches', () => {
  const scene = new THREE.Scene();
  const texture = new THREE.DataTexture(new Uint8Array([255, 255, 255, 255]), 1, 1);
  const sink = new RendererThreeParticleSink({
    scene,
    batchCapacity: 2,
    textureFactory: () => texture,
  });

  sink.create(cube(1));
  sink.create(cube(2));
  sink.create(cube(3));
  sink.create(billboard(4));
  assert.deepEqual(sink.readout(), {
    activeParticles: 4,
    activeBatches: 3,
    billboardBatches: 1,
    cubeBatches: 2,
    allocatedSlots: 6,
    highWaterMark: 4,
  });
  const particleGroup = scene.getObjectByName('rusty-particles');
  assert.ok(particleGroup !== undefined);
  assert.equal(particleGroup.children.filter((child) => child instanceof THREE.InstancedMesh).length, 2);
  assert.equal(particleGroup.children.filter((child) => child instanceof THREE.Points).length, 1);

  sink.update(cube(3, 9));
  sink.destroy(1);
  sink.destroy(2);
  assert.equal(sink.readout().activeParticles, 2);
  assert.equal(sink.readout().activeBatches, 2);
  assert.equal(sink.readout().highWaterMark, 4);

  sink.dispose();
  assert.equal(scene.getObjectByName('rusty-particles'), undefined);
  assert.deepEqual(sink.readout(), {
    activeParticles: 0,
    activeBatches: 0,
    billboardBatches: 0,
    cubeBatches: 0,
    allocatedSlots: 0,
    highWaterMark: 4,
  });
});

void test('Three particle sink rejects duplicate ids and use after disposal', () => {
  const sink = new RendererThreeParticleSink({ scene: new THREE.Scene(), batchCapacity: 1 });
  sink.create(cube(7));
  assert.throws(() => sink.create(cube(7)), /already exists/u);
  sink.dispose();
  assert.throws(() => sink.create(cube(8)), /disposed/u);
});
