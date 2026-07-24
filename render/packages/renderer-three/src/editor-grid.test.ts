import assert from 'node:assert/strict';
import test from 'node:test';
import type * as THREE from 'three';
import type { EditorGridDescriptor } from '@rusty-engine/render-contracts';
import { projectEditorGrid, ThreeEditorGridProjection } from './editor-grid.js';

const descriptor: EditorGridDescriptor = {
  visible: true,
  grid: {
    coordinateSystem: 'rightHandedYUp',
    origin: [0.25, 1.5, -0.5],
    spacing: [0.5, 2, 0.25],
  },
  plane: 'xz',
  snapAnchor: 'cellCenter',
  style: {
    minorColor: [0.1, 0.2, 0.3, 0.4],
    majorColor: [0.2, 0.3, 0.4, 0.8],
    xAxisColor: [1, 0, 0, 1],
    yAxisColor: [0, 1, 0, 1],
    zAxisColor: [0, 0, 1, 1],
    majorLineEvery: 4,
    opacity: 0.9,
    fadeStart: 8,
    fadeEnd: 40,
  },
};

const camera = {
  pose: { position: [8, 10, 12] as const, yawDegrees: 0, pitchDegrees: -35 },
  basis: {
    forward: [-0.48, -0.62, -0.62] as const,
    right: [0.79, 0, -0.61] as const,
    up: [-0.38, 0.78, -0.49] as const,
  },
  projection: { fovYDegrees: 55, near: 0.05, far: 1000 },
};

void test('procedural XZ grid keeps every line on the explicit Y-up plane and origin lattice', () => {
  const projection = projectEditorGrid(descriptor, camera, { width: 1200, height: 700 });
  assert.ok(projection.lines.length > 20);
  assert.equal(projection.readout.descriptor.grid.coordinateSystem, 'rightHandedYUp');
  assert.equal(projection.readout.bounds?.min[1], 1.5);
  assert.equal(projection.readout.bounds?.max[1], 1.5);
  for (const line of projection.lines) {
    assert.equal(line.a[1], 1.5);
    assert.equal(line.b[1], 1.5);
    const fixedX = line.a[0] === line.b[0];
    const fixedZ = line.a[2] === line.b[2];
    assert.notEqual(fixedX, fixedZ);
    if (fixedX) {
      assert.ok(Math.abs((line.a[0] - descriptor.grid.origin[0]) / descriptor.grid.spacing[0]
        - Math.round((line.a[0] - descriptor.grid.origin[0]) / descriptor.grid.spacing[0])) < 1e-8);
    }
    if (fixedZ) {
      assert.ok(Math.abs((line.a[2] - descriptor.grid.origin[2]) / descriptor.grid.spacing[2]
        - Math.round((line.a[2] - descriptor.grid.origin[2]) / descriptor.grid.spacing[2])) < 1e-8);
    }
  }
});

void test('camera movement changes projected extent without changing descriptor spacing or origin', () => {
  const first = projectEditorGrid(descriptor, camera, { width: 1200, height: 700 });
  const moved = projectEditorGrid(descriptor, {
    ...camera,
    pose: { ...camera.pose, position: [24, 18, -16] },
  }, { width: 1200, height: 700 });
  assert.notDeepEqual(moved.readout.bounds, first.readout.bounds);
  assert.deepEqual(moved.readout.descriptor.grid, descriptor.grid);
  assert.ok(moved.readout.minorLineStep >= 1);
});

void test('Three realization updates and removes the public descriptor without scene authority nodes', () => {
  const grid = new ThreeEditorGridProjection();
  grid.setCamera(camera);
  grid.resize({ width: 800, height: 600 });
  grid.setDescriptor(descriptor);
  assert.equal(grid.scene.children[0]?.name, 'rusty-editor-grid');
  const lines = grid.scene.children[0] as THREE.LineSegments;
  const material = lines.material as THREE.ShaderMaterial;
  assert.equal(material.depthTest, true);
  assert.equal(material.depthWrite, false);
  assert.ok((grid.readout()?.renderedLineCount ?? 0) > 0);
  grid.setDescriptor(null);
  assert.equal(grid.scene.children.length, 0);
  assert.equal(grid.readout(), null);
  grid.dispose();
});
