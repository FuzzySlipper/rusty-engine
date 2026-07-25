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
});
