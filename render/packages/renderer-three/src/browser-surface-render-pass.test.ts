import assert from 'node:assert/strict';
import { test } from 'node:test';

import * as THREE from 'three';
import {
  renderBrowserSurfaceFrame,
  type BrowserSurfaceRenderDriver,
} from './browser-surface-render-pass.js';

void test('browser surface composes world then camera-relative presentation after clearing depth', () => {
  const events: string[] = [];
  const world = namedScene('world');
  const viewmodel = namedScene('viewmodel');
  const worldCamera = namedCamera('world-camera');
  const viewmodelCamera = namedCamera('viewmodel-camera');
  const driver: BrowserSurfaceRenderDriver = {
    clear: (color, depth, stencil) => events.push(`clear:${color}:${depth}:${stencil}`),
    clearDepth: () => events.push('clearDepth'),
    render: (scene, camera) => events.push(`render:${scene.name}:${camera.name}`),
  };

  renderBrowserSurfaceFrame(driver, worldCamera, viewmodelCamera, {
    scene: world,
    viewmodelScene: viewmodel,
    advanceAnimation: (deltaSeconds) => events.push(`advance:${deltaSeconds}`),
    prepareSpritesForCamera: (camera, scene) =>
      events.push(`sprites:${scene.name}:${camera.name}`),
    prepareStaticInstanceBatches: (camera) => events.push(`prepare:${camera.name}`),
  }, 0.025);

  assert.deepEqual(events, [
    'clear:true:true:true',
    'advance:0.025',
    'sprites:world:world-camera',
    'prepare:world-camera',
    'render:world:world-camera',
    'clearDepth',
    'sprites:viewmodel:viewmodel-camera',
    'render:viewmodel:viewmodel-camera',
  ]);
});

void test('browser surface keeps bounded viewmodel transforms camera-relative across world poses', () => {
  const worldCamera = namedCamera('world-camera');
  const viewmodelCamera = namedCamera('viewmodel-camera');
  const viewmodelScene = namedScene('viewmodel');
  const localPoint = new THREE.Vector3(0.25, -0.2, -1.5);
  const projected: THREE.Vector3[] = [];
  const driver: BrowserSurfaceRenderDriver = {
    clear: () => undefined,
    clearDepth: () => undefined,
    render: (scene, camera) => {
      if (scene === viewmodelScene) projected.push(localPoint.clone().project(camera));
    },
  };
  const projection = {
    scene: namedScene('world'),
    viewmodelScene,
    advanceAnimation: () => undefined,
    prepareSpritesForCamera: () => undefined,
    prepareStaticInstanceBatches: () => undefined,
  };

  worldCamera.position.set(48, 6, -32);
  worldCamera.rotation.set(0, THREE.MathUtils.degToRad(67), 0);
  renderBrowserSurfaceFrame(driver, worldCamera, viewmodelCamera, projection, 0);
  worldCamera.position.set(-64, 9, 72);
  worldCamera.rotation.set(0, THREE.MathUtils.degToRad(-121), 0);
  renderBrowserSurfaceFrame(driver, worldCamera, viewmodelCamera, projection, 0);

  assert.equal(projected.length, 2);
  assert.ok(projected[0]!.distanceTo(projected[1]!) < 1e-12);
  assert.deepEqual(viewmodelCamera.position.toArray(), [0, 0, 0]);
  assert.ok(viewmodelCamera.quaternion.angleTo(new THREE.Quaternion()) < 1e-12);
});

function namedScene(name: string): THREE.Scene {
  const scene = new THREE.Scene();
  scene.name = name;
  return scene;
}

function namedCamera(name: string): THREE.PerspectiveCamera {
  const camera = new THREE.PerspectiveCamera();
  camera.name = name;
  return camera;
}
