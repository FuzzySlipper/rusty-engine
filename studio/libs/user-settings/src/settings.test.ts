import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  buildDefaultStudioHostUserSettings,
  parseStudioHostUserSettings,
  serializeStudioHostUserSettings,
  validateStudioHostUserSettings,
} from './settings.js';

void test('host-user settings round-trip the complete scene-view and input family', () => {
  const artifact = buildDefaultStudioHostUserSettings('rusty-studio-project:abc');
  const changed = {
    ...artifact,
    theme: 'highContrast' as const,
    sceneView: {
      ...artifact.sceneView,
      lightingMode: 'authored_lights' as const,
      gridVisible: false,
      cameraMoveSpeed: 18,
      cameraBoostMultiplier: 5,
      invertLookY: true,
      invertPanY: true,
    },
    keyboard: { ...artifact.keyboard, moveForward: 'ArrowUp' },
  };
  const parsed = parseStudioHostUserSettings(serializeStudioHostUserSettings(changed));
  assert.equal(parsed.status, 'loaded');
  assert.deepEqual(parsed.artifact, changed);
});

void test('future or invalid settings preserve source text and disable replacement', () => {
  const artifact = buildDefaultStudioHostUserSettings('rusty-studio-project:abc');
  const future = serializeStudioHostUserSettings(artifact).replace(
    'rusty-engine-studio-host-user-settings.v1',
    'rusty-engine-studio-host-user-settings.v9',
  );
  assert.deepEqual(parseStudioHostUserSettings(future), {
    status: 'unsupported_future_version',
    artifact: null,
    preservedRawText: future,
    diagnostic: 'Unsupported settings version rusty-engine-studio-host-user-settings.v9; the original text was preserved and writes are disabled.',
  });
  assert.throws(
    () => validateStudioHostUserSettings({
      ...artifact,
      sceneView: { ...artifact.sceneView, cameraMoveSpeed: 0 },
    }),
    /camera move speed must be finite and positive/i,
  );
  assert.throws(
    () => validateStudioHostUserSettings({
      ...artifact,
      sceneView: { ...artifact.sceneView, lightingMode: 'cinematic' },
    }),
    /lighting mode must be work_light or authored_lights/i,
  );
  assert.throws(
    () => validateStudioHostUserSettings({
      ...artifact,
      projectKey: '\u00e9'.repeat(81),
    }),
    /160-byte bound/u,
  );
  assert.throws(
    () => validateStudioHostUserSettings({
      ...artifact,
      keyboard: { ...artifact.keyboard, moveForward: '\u00e9'.repeat(33) },
    }),
    /64-byte bound/u,
  );
});

void test('older v1 settings gain explicit transform-tool and work-light defaults without losing compatibility', () => {
  const artifact = buildDefaultStudioHostUserSettings('rusty-studio-project:abc');
  const legacy = JSON.stringify({
    ...artifact,
    editor: { snappingEnabled: true, translationSnap: 0.25 },
    sceneView: { ...artifact.sceneView, lightingMode: undefined },
  });
  const parsed = parseStudioHostUserSettings(legacy);
  assert.equal(parsed.status, 'loaded');
  assert.deepEqual(parsed.artifact?.editor.translationSnapAxes, [0.25, 0.25, 0.25]);
  assert.equal(parsed.artifact?.editor.transformOrientation, 'world');
  assert.equal(parsed.artifact?.sceneView.lightingMode, 'work_light');
});
