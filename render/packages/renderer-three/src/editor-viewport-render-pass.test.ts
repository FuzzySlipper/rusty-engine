import assert from 'node:assert/strict';
import { test } from 'node:test';

import * as THREE from 'three';
import {
  renderEditorViewportFrame,
  type EditorViewportRenderChannels,
  type EditorViewportRenderDriver,
} from './editor-viewport-render-pass.js';

void test('editor viewport establishes shared scene depth before grid and clears only for overlay', () => {
  const events: string[] = [];
  const scenes = new Map([
    ['runtime', namedScene('runtime')],
    ['authored', namedScene('authored')],
    ['overlay', namedScene('overlay')],
  ]);
  const channels: EditorViewportRenderChannels = {
    renderer: (channel) => ({
      scene: scenes.get(channel)!,
      advanceAnimation: (deltaSeconds) => events.push(`advance:${channel}:${deltaSeconds}`),
    }),
  };
  const grid = namedScene('grid');
  const driver: EditorViewportRenderDriver = {
    clear: (color, depth, stencil) => events.push(`clear:${color}:${depth}:${stencil}`),
    clearDepth: () => events.push('clearDepth'),
    render: (scene) => events.push(`render:${scene.name}`),
  };

  renderEditorViewportFrame(driver, new THREE.PerspectiveCamera(), grid, channels, 0.025);

  assert.deepEqual(events, [
    'clear:true:true:true',
    'advance:runtime:0.025',
    'render:runtime',
    'advance:authored:0.025',
    'render:authored',
    'render:grid',
    'advance:overlay:0.025',
    'clearDepth',
    'render:overlay',
  ]);
});

function namedScene(name: string): THREE.Scene {
  const scene = new THREE.Scene();
  scene.name = name;
  return scene;
}
