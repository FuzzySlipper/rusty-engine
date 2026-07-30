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
    prepareStaticInstanceBatches: (camera) => events.push(`prepare:${camera.name}`),
  }, 0.025);

  assert.deepEqual(events, [
    'clear:true:true:true',
    'advance:0.025',
    'prepare:world-camera',
    'render:world:world-camera',
    'clearDepth',
    'render:viewmodel:viewmodel-camera',
  ]);
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
