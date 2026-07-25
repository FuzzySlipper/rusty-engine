import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const templateUrl = new URL('../libs/editor-shell/src/studio-shell.component.html', import.meta.url);
const stateUrl = new URL('../libs/editor-shell/src/state.ts', import.meta.url);
const viewportUrl = new URL('../libs/viewport/src/studio-viewport.component.ts', import.meta.url);

test('shell exposes the preserved editor surfaces and honest deferred operations', async () => {
  const template = await readFile(templateUrl, 'utf8');
  for (const visualId of [
    'studio-shell',
    'studio-project-open-controls',
    'studio-hierarchy-panel',
    'studio-viewport-readout',
    'studio-bottom-panel',
    'studio-inspector-panel',
    'studio-settings-dialog',
  ]) {
    assert.match(template, new RegExp(`data-visual-id="${visualId}"`));
  }
  assert.match(template, /<rusty-studio-viewport/);
  assert.match(template, /Picking is active directly on the shared renderer canvas/);
  assert.match(template, /disabled title="Voxel asset, annotation, history, and conversion owners land in M11E"/);
});

test('viewport composes the shared renderer host without private Three ownership', async () => {
  const viewport = await readFile(viewportUrl, 'utf8');
  assert.match(viewport, /mountRendererInspectionSurface/);
  assert.match(viewport, /surface\.replaceFrame\(frame\)/);
  assert.match(viewport, /surface\.pick\(/);
  assert.match(viewport, /surface\.setGrid\(/);
  assert.match(viewport, /surface\.dispose\(\)/);
  assert.doesNotMatch(viewport, /from ['"]three/);
});

test('shell state keeps canonical content, projection, selection, and preview as distinct models', async () => {
  const state = await readFile(stateUrl, 'utf8');
  for (const model of [
    'AuthoringDocumentView',
    'LiveProjectionView',
    'EditorSelectionState',
    'TransformPreviewState',
  ]) {
    assert.match(state, new RegExp(`interface ${model}`));
  }
  assert.doesNotMatch(state, /localStorage|sessionStorage|three|asha/i);
});
