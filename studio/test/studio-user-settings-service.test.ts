import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import {
  buildDefaultStudioHostUserSettings,
  parseStudioHostUserSettings,
  serializeStudioHostUserSettings,
} from '../libs/user-settings/src/index.js';
import {
  readStudioUserSettings,
  resolveStudioUserSettingsLocation,
  writeStudioUserSettings,
} from '../scripts/studio-user-settings-service.js';

void test('host settings survive a fresh service read and remain isolated per canonical project', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-settings-'));
  const settingsRoot = join(root, 'settings');
  const projectA = join(root, 'project-a');
  const projectB = join(root, 'project-b');
  await Promise.all([mkdir(projectA), mkdir(projectB)]);
  const location = await resolveStudioUserSettingsLocation({ projectRoot: projectA, settingsRoot });
  const artifact = buildDefaultStudioHostUserSettings(location.projectKey);
  const changed = {
    ...artifact,
    sceneView: { ...artifact.sceneView, cameraMoveSpeed: 18, invertPanY: true },
    keyboard: { ...artifact.keyboard, moveForward: 'ArrowUp' },
  };
  const firstWrite = await writeStudioUserSettings({
    projectRoot: projectA,
    settingsRoot,
    text: serializeStudioHostUserSettings(changed),
    expectedHash: null,
  });
  assert.equal(firstWrite.ok, true);
  const afterRestart = await readStudioUserSettings({ projectRoot: projectA, settingsRoot });
  assert.equal(afterRestart.exists, true);
  assert.equal(parseStudioHostUserSettings(afterRestart.text as string).artifact?.sceneView.cameraMoveSpeed, 18);
  assert.equal(parseStudioHostUserSettings(afterRestart.text as string).artifact?.keyboard.moveForward, 'ArrowUp');
  const stale = await writeStudioUserSettings({
    projectRoot: projectA,
    settingsRoot,
    text: serializeStudioHostUserSettings(artifact),
    expectedHash: null,
  });
  assert.deepEqual(stale, {
    ok: false,
    diagnostic: 'stale_user_settings',
    message: 'Host-user settings changed since they were loaded; reload before saving.',
  });
  assert.equal((await readStudioUserSettings({ projectRoot: projectB, settingsRoot })).exists, false);
});

void test('host settings reject project-key substitution and symlink targets', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-settings-safety-'));
  const settingsRoot = join(root, 'settings');
  const project = join(root, 'project');
  await Promise.all([mkdir(project), mkdir(settingsRoot)]);
  const location = await resolveStudioUserSettingsLocation({ projectRoot: project, settingsRoot });
  const wrong = buildDefaultStudioHostUserSettings('rusty-studio-project:not-this-project');
  const mismatch = await writeStudioUserSettings({
    projectRoot: project,
    settingsRoot,
    text: serializeStudioHostUserSettings(wrong),
    expectedHash: null,
  });
  assert.equal(mismatch.ok, false);
  if (mismatch.ok) throw new Error('expected project-key mismatch');
  assert.equal(mismatch.diagnostic, 'project_key_mismatch');

  const outside = join(root, 'outside.json');
  await writeFile(outside, '{}');
  await symlink(outside, location.path);
  await assert.rejects(
    () => readStudioUserSettings({ projectRoot: project, settingsRoot }),
    /regular non-symlink file/,
  );
  assert.equal(await readFile(outside, 'utf8'), '{}');
});
