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
  writeStudioUserSettingsWithMaintenance,
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

void test('competing same-target writes serialize into one commit and one stale result', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-settings-race-'));
  const settingsRoot = join(root, 'settings');
  const project = join(root, 'project');
  await mkdir(project);
  const location = await resolveStudioUserSettingsLocation({ projectRoot: project, settingsRoot });
  const baseline = buildDefaultStudioHostUserSettings(location.projectKey);
  const candidates = [11, 23].map((cameraMoveSpeed) => serializeStudioHostUserSettings({
    ...baseline,
    sceneView: { ...baseline.sceneView, cameraMoveSpeed },
  }));

  const results = await Promise.all(candidates.map((text) => writeStudioUserSettings({
    projectRoot: project,
    settingsRoot,
    text,
    expectedHash: null,
  })));

  assert.equal(results.filter((result) => result.ok).length, 1);
  assert.equal(results.filter((result) => !result.ok && result.diagnostic === 'stale_user_settings').length, 1);
  const stored = parseStudioHostUserSettings(
    (await readStudioUserSettings({ projectRoot: project, settingsRoot })).text as string,
  );
  assert.equal(stored.status, 'loaded');
  assert.ok([11, 23].includes(stored.artifact?.sceneView.cameraMoveSpeed as number));
});

void test('a post-rename maintenance failure still reports the committed settings truthfully', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-settings-commit-'));
  const settingsRoot = join(root, 'settings');
  const project = join(root, 'project');
  await mkdir(project);
  const location = await resolveStudioUserSettingsLocation({ projectRoot: project, settingsRoot });
  const text = serializeStudioHostUserSettings(buildDefaultStudioHostUserSettings(location.projectKey));

  const result = await writeStudioUserSettingsWithMaintenance({
    projectRoot: project,
    settingsRoot,
    text,
    expectedHash: null,
  }, {
    afterPublish: async () => { throw new Error('injected directory sync failure'); },
  });

  assert.equal(result.ok, true);
  assert.equal(await readFile(location.path, 'utf8'), text);
});
