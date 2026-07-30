import { expect, test, type Locator, type Page } from '@playwright/test';
import { copyFile, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

import { installPackedPlacementResourceAdapter } from './packed-placement-resource.js';

const projectRoot = requiredEnvironment('RUSTY_STUDIO_PROJECT_ROOT');
const loadingBayProjectFile = 'content/projects/loading-bay.project.json';
const convertedWallProjectFile = 'content/projects/converted-wall.project.json';
const voxelObjectProjectFile = 'content/projects/studio-voxel-object.project.json';
const projectOpenTimeout = 30_000;
const projectMutationTimeout = 30_000;

test('real project hierarchy, shared picking, transform settlement, reopen, and rejection stay coherent', async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(loadingBayProjectFile)}`);

  const shell = page.locator('[data-visual-id="studio-shell"]');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/, {
    timeout: projectOpenTimeout,
  });
  const viewport = page.locator('rusty-studio-viewport');
  await expect.poll(async () => {
    const status = await viewport.getAttribute('data-renderer-status');
    if (status === 'error') {
      throw new Error((await page.getByRole('alert').textContent()) ?? 'shared renderer failed');
    }
    return status;
  }).toBe('ready');
  await expect.poll(() => page.locator('.entity-row').count()).toBeGreaterThanOrEqual(8);
  await expect(viewport).toHaveAttribute(
    'data-retained-ops',
    /^[1-9][0-9]*$/,
  );
  await expect(viewport).toHaveAttribute('data-authored-frame-hash', /.+/);

  const cameraRevisionBeforeFocus = Number(await viewport.getAttribute('data-camera-revision'));
  await page.locator('.entity-row[data-entity-id="1"]').dblclick();
  await expect.poll(async () => Number(await viewport.getAttribute('data-camera-revision')))
    .toBe(cameraRevisionBeforeFocus + 1);
  await expect(viewport.locator('canvas')).toBeFocused();

  const pickedEntity = await pickVisibleEntity(page, shell);
  await expect(shell).toHaveAttribute('data-selection-source', 'renderer');
  await expect(page.locator(`.entity-row[data-entity-id="${pickedEntity}"]`)).toHaveClass(
    /is-selected/,
  );
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  await expect(page.locator('.inspector-panel')).toContainText(`Entity #${pickedEntity}`);

  await page.locator('.entity-row[data-entity-id="1"]').click();
  await expect(viewport).toHaveAttribute('data-selected-entity', '1');
  await page.waitForTimeout(50);
  const translationX = page.getByLabel('Translation X', { exact: true });
  const initialX = Number(await translationX.inputValue());
  const committedX = initialX + 0.5;
  const hashBeforeCommit = await projectHash(shell);
  await expect(viewport).toHaveAttribute('data-selected-render-handle', /[0-9]+/);
  const selectedRendererHash = await rendererHash(viewport);

  await translationX.fill(String(committedX));
  await expect(page.locator('[data-preview-active="true"]')).toBeVisible();
  await expect(viewport).toHaveAttribute('data-preview-applied', 'true');
  await expect.poll(() => rendererHash(viewport)).not.toBe(selectedRendererHash);
  await page.locator('.inspector-actions').getByRole('button', { name: 'Revert', exact: true }).click();
  await expect(viewport).toHaveAttribute('data-preview-applied', 'false');
  await expect(viewport).toHaveAttribute('data-authored-frame-hash', selectedRendererHash);

  await translationX.fill(String(committedX));
  await expect(viewport).toHaveAttribute('data-preview-applied', 'true');
  await page.locator('.entity-row[data-entity-id="2"]').click();
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashBeforeCommit,
  );
  await expect(shell).toHaveAttribute('data-selected-entity', '2');
  await expect(viewport).toHaveAttribute('data-transform-tool', 'translate');
  await expect(viewport).toHaveAttribute('data-transform-gizmo-visible', 'true');
  const committedHash = await projectHash(shell);
  await expect(page.locator('[data-preview-active="true"]')).toHaveCount(0);
  await expect(viewport).toHaveAttribute('data-preview-applied', 'false');
  await page.locator('.entity-row[data-entity-id="1"]').click();
  await expect(translationX).toHaveValue(String(committedX));

  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(shell).toHaveAttribute('data-project-hash', committedHash, {
    timeout: projectOpenTimeout,
  });
  await expect(translationX).toHaveValue(String(committedX));

  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', committedHash, {
    timeout: projectOpenTimeout,
  });
  await page.locator('.entity-row[data-entity-id="1"]').click();
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  await expect(page.getByLabel('Translation X', { exact: true })).toHaveValue(String(committedX));

  const hashBeforeInvalidCommit = await projectHash(shell);
  await page.getByLabel('Translation X', { exact: true }).fill('2000000');
  await page.locator('[data-action="commit-transform"]').click();
  await expect(page.getByRole('alert')).toContainText('invalid-scene-after-edit');
  await expect(shell).toHaveAttribute('data-project-hash', hashBeforeInvalidCommit);
  await expect(page.locator('[data-preview-active="true"]')).toBeVisible();
});

test('project, scene, entity, light, and capability authoring flow through named Rust operations', async ({ page }) => {
  await page.goto('/');
  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');

  await page.getByRole('button', { name: 'File', exact: true }).click();
  await page.getByRole('button', { name: 'New Project…', exact: true }).click();
  let dialog = page.locator('[data-visual-id="studio-authoring-dialog"]');
  await dialog.getByLabel('Project root').fill(projectRoot);
  await dialog.getByLabel('Project file').fill('content/projects/studio-browser.project.json');
  await dialog.getByLabel('Project ID').fill('studio-browser');
  await dialog.getByLabel('Name', { exact: true }).fill('Studio Browser');
  await dialog.getByLabel('Entry scene ID').fill('scene/studio-main');
  await dialog.getByLabel('Entry scene name').fill('Studio Main');
  await dialog.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(shell).toHaveAttribute('data-project-hash', /.+/);
  await expect(page.locator('.entity-row')).toHaveCount(0);

  await page.getByRole('button', { name: 'Manage scenes' }).click();
  dialog = page.locator('[data-visual-id="studio-authoring-dialog"]');
  await dialog.getByLabel('Scene ID').fill('scene/lighting');
  await dialog.getByLabel('Scene name').fill('Lighting Lab');
  await dialog.getByLabel('Open as entry scene after creation').check();
  await dialog.getByRole('button', { name: 'Create', exact: true }).click();
  await expect(page.locator('.document-title')).toContainText('Studio Browser');

  await page.getByRole('button', { name: 'Create scene object' }).click();
  dialog = page.locator('[data-visual-id="studio-authoring-dialog"]');
  await dialog.getByLabel('Entity ID').fill('42');
  await dialog.getByLabel('Name', { exact: true }).fill('Key Light');
  await dialog.getByLabel('Appearance').selectOption('light');
  await dialog.getByRole('button', { name: 'Create object', exact: true }).click();
  await expect(page.locator('.entity-row[data-entity-id="42"]')).toContainText('Key Light');
  await expect(page.locator('.viewport-source-readout')).toContainText('1 lights');
  await expect(viewport).toHaveAttribute('data-lighting-mode', 'work_light');
  await expect(viewport).toHaveAttribute('data-work-light-active', 'true');
  const lightingToggle = page.locator('[data-action="toggle-work-light"]');
  await expect(lightingToggle).toHaveAttribute('aria-pressed', 'true');
  const projectHashBeforeLightingToggle = await projectHash(shell);
  const workLightFrameHash = await rendererHash(viewport);
  await lightingToggle.click();
  await expect(viewport).toHaveAttribute('data-lighting-mode', 'authored_lights');
  await expect(viewport).toHaveAttribute('data-work-light-active', 'false');
  await expect.poll(() => rendererHash(viewport)).not.toBe(workLightFrameHash);
  await expect(shell).toHaveAttribute('data-project-hash', projectHashBeforeLightingToggle);
  await lightingToggle.click();
  await expect(viewport).toHaveAttribute('data-lighting-mode', 'work_light');
  await expect(viewport).toHaveAttribute('data-work-light-active', 'true');

  await page.locator('.entity-row[data-entity-id="42"]').click();
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  const inspector = page.locator('.inspector-panel');
  await inspector.getByLabel('Rotation Z').fill('0.7071068');
  await inspector.getByLabel('Rotation W').fill('0.7071068');
  const transformHash = await projectHash(shell);
  await inspector.locator('[data-action="commit-transform"]').click();
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    transformHash,
  );
  await expect(inspector.getByLabel('Rotation Z')).toHaveValue('0.7071068');

  const name = inspector.locator('.field-row').filter({ hasText: 'Name' }).locator('input');
  await name.fill('Sun Key');
  await inspector.getByRole('button', { name: 'Rename', exact: true }).click();
  await expect(page.locator('.entity-row[data-entity-id="42"]')).toContainText('Sun Key');

  const collisionHash = await projectHash(shell);
  await inspector.locator('.inspector-section').filter({ hasText: 'Collision' })
    .getByRole('button', { name: 'Apply', exact: true }).click();
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    collisionHash,
  );
  const kinematicHash = await projectHash(shell);
  await inspector.locator('.inspector-section').filter({ hasText: 'Kinematic' })
    .getByRole('button', { name: 'Apply', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('project.invalidSpatial');
  await expect(shell).toHaveAttribute('data-project-hash', kinematicHash);
  await page.getByRole('alert').getByRole('button', { name: 'Dismiss' }).click();

  await page.getByRole('button', { name: 'Create scene object' }).click();
  dialog = page.locator('[data-visual-id="studio-authoring-dialog"]');
  await dialog.getByLabel('Entity ID').fill('43');
  await dialog.getByLabel('Name', { exact: true }).fill('Scaled Child');
  await dialog.getByRole('button', { name: 'Create object', exact: true }).click();
  await page.locator('.entity-row[data-entity-id="43"]').click();
  await inspector.getByLabel('Scale X').fill('2');
  const scaleHash = await projectHash(shell);
  await inspector.locator('[data-action="commit-transform"]').click();
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    scaleHash,
  );
  await expect(inspector.getByLabel('Scale X')).toHaveValue('2');

  await page.getByRole('button', { name: 'File', exact: true }).click();
  await page.getByRole('button', { name: 'Save Project As…', exact: true }).click();
  dialog = page.locator('[data-visual-id="studio-authoring-dialog"]');
  await dialog.getByLabel('Project file').fill('content/projects/studio-browser-copy.project.json');
  await dialog.getByLabel('Project ID').fill('studio-browser-copy');
  await dialog.getByLabel('Name', { exact: true }).fill('Studio Browser Copy');
  await dialog.getByRole('button', { name: 'Save copy', exact: true }).click();
  await expect(page.locator('.document-title')).toContainText('Studio Browser Copy');
  await expect(page.locator('.document-title')).toContainText('studio-browser-copy.project.json');
  await expect(page.locator('.entity-row[data-entity-id="42"]')).toContainText('Sun Key');
});

test('host-user input settings and general asset import reimport persist through real owner and renderer paths', async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(loadingBayProjectFile)}`);
  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/, {
    timeout: projectOpenTimeout,
  });
  await expect.poll(async () => {
    const status = await viewport.getAttribute('data-renderer-status');
    if (status === 'error') {
      throw new Error((await viewport.getAttribute('data-renderer-error')) ?? 'shared renderer failed');
    }
    return status;
  }).toBe('ready');

  await page.getByRole('button', { name: 'View', exact: true }).click();
  await page.getByRole('button', { name: 'Studio Settings…', exact: true }).click();
  const settings = page.locator('[data-visual-id="studio-settings-dialog"]');
  await settings.getByLabel('Camera move speed').fill('12');
  await settings.getByLabel('Camera move speed').press('Tab');
  await settings.getByLabel('Camera boost multiplier').fill('6');
  await settings.getByLabel('Camera boost multiplier').press('Tab');
  await settings.getByLabel('Invert orbit look Y').check();
  await settings.getByLabel('Move forward binding').press('ArrowUp');
  await expect(shell).toHaveAttribute('data-camera-speed', '12');
  await expect(shell).toHaveAttribute('data-move-forward', 'ArrowUp');
  await expect(viewport).toHaveAttribute('data-camera-move-speed', '12');
  await expect(viewport).toHaveAttribute('data-camera-move-forward', 'ArrowUp');
  await expect(shell).toHaveAttribute('data-user-settings-status', 'loaded');
  await settings.getByRole('button', { name: 'Done', exact: true }).click();
  const lightingToggle = page.locator('[data-action="toggle-work-light"]');
  await expect(lightingToggle).toHaveAttribute('aria-pressed', 'true');
  await lightingToggle.click();
  await expect(shell).toHaveAttribute('data-lighting-mode', 'authored_lights');
  await expect(viewport).toHaveAttribute('data-lighting-mode', 'authored_lights');
  await expect(shell).toHaveAttribute('data-user-settings-status', 'loaded');

  await page.reload();
  await expect(shell).toHaveAttribute('data-user-settings-status', 'loaded', {
    timeout: projectOpenTimeout,
  });
  await expect(shell).toHaveAttribute('data-camera-speed', '12');
  await expect(shell).toHaveAttribute('data-move-forward', 'ArrowUp');
  await expect(viewport).toHaveAttribute('data-camera-move-speed', '12');
  await expect(viewport).toHaveAttribute('data-camera-move-forward', 'ArrowUp');
  await expect(shell).toHaveAttribute('data-lighting-mode', 'authored_lights');
  await expect(viewport).toHaveAttribute('data-lighting-mode', 'authored_lights');

  const assetsBeforeImport = await projectAssetCount(shell);
  expect(assetsBeforeImport).toBeGreaterThan(0);
  const hashBeforePlan = await projectHash(shell);
  await page.getByRole('button', { name: 'File', exact: true }).click();
  await page.getByRole('button', { name: 'Import Project Asset…', exact: true }).click();
  let dialog = page.locator('[data-visual-id="studio-authoring-dialog"]');
  await dialog.getByLabel('Source mesh').fill('content/assets/studio-triangle.mesh.json');
  await dialog.getByLabel('Scale', { exact: true }).fill('2');
  await dialog.getByLabel('Material namespace').fill('studio');
  await dialog.getByLabel('Generate AABB collision when source is visual-only').check();
  await dialog.getByRole('button', { name: 'Prepare import', exact: true }).click();
  const plan = page.locator('[data-visual-id="studio-asset-import-plan"]');
  await expect(plan).toContainText('structuralReload · mesh/studio-triangle');
  await expect(plan).toContainText('2 generated assets');
  await expect(shell).toHaveAttribute('data-project-hash', hashBeforePlan);
  await plan.getByRole('button', { name: 'Apply atomically', exact: true }).click();
  await expect.poll(() => projectAssetCount(shell), { timeout: projectMutationTimeout }).toBe(
    assetsBeforeImport + 2,
  );
  const assetsAfterImport = await projectAssetCount(shell);
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashBeforePlan,
  );

  const importedAsset = page.getByRole('option', { name: /mesh\/studio-triangle/ });
  await importedAsset.click();
  await expect(importedAsset).toContainText('unchanged');
  await expect(page.locator('.asset-detail')).toContainText('material/studio/paint');
  await expect(page.locator('.asset-detail')).toContainText('source project:content/assets/studio-triangle.mesh.json');

  const rendererBeforeInstance = await rendererHash(viewport);
  await page.getByRole('button', { name: 'Create scene object' }).click();
  dialog = page.locator('[data-visual-id="studio-authoring-dialog"]');
  await dialog.getByLabel('Entity ID').fill('70');
  await dialog.getByLabel('Name', { exact: true }).fill('Imported Triangle');
  await dialog.getByLabel('Appearance').selectOption('staticMesh');
  await dialog.getByLabel('Asset ID').fill('mesh/studio-triangle');
  await dialog.getByRole('button', { name: 'Create object', exact: true }).click();
  await expect(page.locator('.entity-row[data-entity-id="70"]')).toContainText(
    'Imported Triangle',
    { timeout: projectMutationTimeout },
  );
  await expect.poll(() => rendererHash(viewport)).not.toBe(rendererBeforeInstance);

  const sourcePath = join(projectRoot, 'content/assets/studio-triangle.mesh.json');
  const source = JSON.parse(await readFile(sourcePath, 'utf8')) as {
    materials: Array<{ color: number[] }>;
  };
  const firstMaterial = source.materials[0];
  if (firstMaterial === undefined) throw new Error('Studio mesh fixture has no material');
  firstMaterial.color = [0.8, 0.2, 0.1, 1];
  await writeFile(sourcePath, `${JSON.stringify(source, null, 2)}\n`);
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await importedAsset.click();
  await expect(importedAsset).toContainText('contentChanged', {
    timeout: projectOpenTimeout,
  });

  const hashBeforeReimport = await projectHash(shell);
  const rendererBeforeReimport = await rendererHash(viewport);
  await page.locator('.asset-detail').getByRole('button', { name: 'Prepare reimport' }).click();
  await expect(plan).toContainText('visualUpdate · mesh/studio-triangle');
  await expect(shell).toHaveAttribute('data-project-hash', hashBeforeReimport);
  await plan.getByRole('button', { name: 'Apply atomically', exact: true }).click();
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashBeforeReimport,
  );
  await expect(importedAsset).toContainText('unchanged');
  await expect.poll(() => rendererHash(viewport)).not.toBe(rendererBeforeReimport);

  const hashBeforeInvalidPlan = await projectHash(shell);
  await writeFile(join(projectRoot, 'content/assets/rejected.mesh.json'), '{"schemaVersion":1}\n');
  await page.getByRole('button', { name: 'File', exact: true }).click();
  await page.getByRole('button', { name: 'Import Project Asset…', exact: true }).click();
  dialog = page.locator('[data-visual-id="studio-authoring-dialog"]');
  await dialog.getByLabel('Source mesh').fill('content/assets/rejected.mesh.json');
  await dialog.getByRole('button', { name: 'Prepare import', exact: true }).click();
  await expect(plan.locator('.asset-diagnostic.is-error').first()).toBeVisible();
  await expect(plan.getByRole('button', { name: 'Apply atomically', exact: true })).toBeDisabled();
  await expect(shell).toHaveAttribute('data-project-hash', hashBeforeInvalidPlan);
  await plan.getByRole('button', { name: 'Discard', exact: true }).click();

  const persistedHash = await projectHash(shell);
  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', persistedHash, {
    timeout: projectOpenTimeout,
  });
  await expect.poll(() => projectAssetCount(shell), { timeout: projectMutationTimeout }).toBe(
    assetsAfterImport,
  );
  await expect(shell).toHaveAttribute('data-user-settings-status', 'loaded', {
    timeout: projectOpenTimeout,
  });
  await expect(page.locator('.entity-row[data-entity-id="70"]')).toContainText('Imported Triangle');
});

test('trusted host browsing restores focus and animated appearance uses the shared renderer', async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(loadingBayProjectFile)}`);
  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/, {
    timeout: projectOpenTimeout,
  });
  await expect.poll(async () => {
    const status = await viewport.getAttribute('data-renderer-status');
    if (status === 'error') {
      throw new Error((await viewport.getAttribute('data-renderer-error')) ?? 'shared renderer failed');
    }
    return status;
  }).toBe('ready');
  await expect(viewport).toHaveAttribute('data-animated-mesh-resources', '3');

  const projectControls = page.locator('[data-visual-id="studio-project-open-controls"]');
  const browseRoot = projectControls.getByRole('button', { name: 'Browse…' }).first();
  await browseRoot.click();
  const browser = page.locator('[data-visual-id="studio-host-file-browser"]');
  await expect(browser).toBeVisible();
  await expect(browser.getByLabel('Filter host files')).toBeFocused();
  await browser.getByLabel('Filter host files').fill('content');
  await expect(browser.getByRole('option', { name: /content/ })).toBeVisible();
  await browser.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(browser).toHaveCount(0);
  await expect(browseRoot).toBeFocused();

  await page.locator('.entity-row[data-entity-id="1"]').click();
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  const inspector = page.locator('.inspector-panel');
  const hashBeforeTransform = await projectHash(shell);
  await page.getByTitle('Rotate gizmo').click();
  await expect(viewport).toHaveAttribute('data-transform-gizmo-visible', 'true');
  await expect(viewport).toHaveAttribute('data-transform-tool', 'rotate');
  await expect(page.getByRole('button', { name: /^Snap / })).toHaveClass(/is-active/);
  await page.getByRole('button', { name: 'world', exact: true }).click();
  await expect(viewport).toHaveAttribute('data-transform-orientation', 'local');
  const xHandle = await transformHandlePoint(page, inspector, 'rotate', 0);
  await page.mouse.move(xHandle.x, xHandle.y);
  await expect(viewport).toHaveAttribute('data-hovered-transform-handle', 'rotate:x');
  await page.mouse.down();
  await expect(viewport).toHaveAttribute('data-active-transform-handle', 'rotate:x');
  await page.mouse.move(xHandle.x + 80, xHandle.y - 24, { steps: 24 });
  await page.mouse.up();
  await expect(inspector.getByLabel('Rotation X')).not.toHaveValue('0');
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashBeforeTransform,
  );
  await expect(page.locator('[data-preview-active="true"]')).toHaveCount(0);
  await expect(viewport).toHaveAttribute('data-transform-tool', 'rotate');
  await expect(viewport).toHaveAttribute('data-transform-gizmo-visible', 'true');

  await page.getByTitle('Scale gizmo').click();
  await expect(viewport).toHaveAttribute('data-transform-tool', 'scale');
  const scaleX = inspector.getByLabel('Scale X');
  const scaleBeforeRevert = await scaleX.inputValue();
  const hashBeforeScaleRevert = await projectHash(shell);
  await scaleX.fill(String(Number(scaleBeforeRevert) + 0.25));
  await expect(page.locator('[data-preview-active="true"]')).toBeVisible();
  await inspector.locator('.inspector-actions').getByRole('button', { name: 'Revert', exact: true }).click();
  await expect(scaleX).toHaveValue(scaleBeforeRevert);
  await expect(shell).toHaveAttribute('data-project-hash', hashBeforeScaleRevert);
  await expect(viewport).toHaveAttribute('data-transform-gizmo-visible', 'true');

  const appearance = inspector.locator('.inspector-section').filter({ hasText: 'Appearance' });
  await appearance.getByLabel('Kind').selectOption('animatedMesh');
  await appearance.getByLabel('Asset').selectOption('mesh-animation/kenney-retro-character-medium');
  await appearance.getByLabel('Clip').selectOption('run');
  const hashBeforeAppearance = await projectHash(shell);
  await appearance.getByRole('button', { name: 'Apply appearance', exact: true }).click();
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashBeforeAppearance,
  );
  await expect(shell).toHaveAttribute('data-animated-instance-clips', /(?:^|,)run(?:,|$)/);
  await expect.poll(async () => {
    const status = await viewport.getAttribute('data-renderer-status');
    if (status === 'error') {
      throw new Error((await viewport.getAttribute('data-renderer-error')) ?? 'shared renderer failed');
    }
    return status;
  }).toBe('ready');

  const animatedHash = await projectHash(shell);
  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', animatedHash, {
    timeout: projectOpenTimeout,
  });
  await expect(shell).toHaveAttribute('data-animated-instance-clips', /(?:^|,)run(?:,|$)/);
  await expect(viewport).toHaveAttribute('data-animated-mesh-resources', '3');
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
});

test('voxel Studio owns the complete shared-renderer authoring workflow and rejects stale writes atomically', async ({ page }) => {
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(convertedWallProjectFile)}`);

  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  const editor = page.locator('[data-visual-id="studio-voxel-editor"]');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/, {
    timeout: projectOpenTimeout,
  });
  await expect(shell).toHaveAttribute('data-voxel-assets', '1');
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
  await expect(editor).toContainText('1 assets · 2 instances');
  await expect(page.locator('.viewport-source-readout')).toContainText('2 voxel instances');
  await expect(viewport).toHaveAttribute('data-retained-ops', /^[1-9][0-9]*$/);

  await editor.getByRole('button', { name: 'edit', exact: true }).click();
  await pickVisibleVoxel(page, shell);
  await expect(editor.locator('[data-visual-id="voxel-pick-readout"]')).toBeVisible();

  await editor.getByLabel('Brush mode').selectOption('erase');
  const hashBeforeBrush = await projectHash(shell);
  const rendererBeforeBrushPreview = await rendererHash(viewport);
  await editor.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(editor.locator('[data-visual-id="voxel-brush-preview"]')).toBeVisible();
  await expect(viewport).toHaveAttribute('data-voxel-preview-kind', 'brush');
  await expect(viewport).toHaveAttribute('data-preview-applied', 'true');
  await expect.poll(() => rendererHash(viewport)).not.toBe(rendererBeforeBrushPreview);
  await editor.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(viewport).not.toHaveAttribute('data-voxel-preview-kind', 'brush');
  await expect(viewport).toHaveAttribute('data-preview-applied', 'false');
  await expect.poll(() => rendererHash(viewport)).toBe(rendererBeforeBrushPreview);
  await editor.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(viewport).toHaveAttribute('data-voxel-preview-kind', 'brush');
  await editor.locator('[data-action="apply-voxel-brush"]').click();
  await expect(shell).toHaveAttribute('data-voxel-receipt', 'voxelBrushApplied');
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashBeforeBrush,
  );
  const hashAfterBrush = await projectHash(shell);

  await editor.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(shell).toHaveAttribute('data-voxel-receipt', 'voxelHistoryMoved');
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashAfterBrush,
  );
  const hashAfterUndo = await projectHash(shell);
  await editor.getByRole('button', { name: 'Redo', exact: true }).click();
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashAfterUndo,
  );

  await editor.getByRole('button', { name: 'annotations', exact: true }).click();
  const hashBeforeAnnotation = await projectHash(shell);
  await editor.getByRole('button', { name: 'Create from pick', exact: true }).click();
  await expect(shell).toHaveAttribute('data-voxel-receipt', 'voxelAnnotationCreated');
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashBeforeAnnotation,
  );
  await editor.getByRole('button', { name: 'Query', exact: true }).click();
  await expect(editor.locator('[data-visual-id="voxel-annotation-readout"]')).toContainText('annotationQuery');
  await editor.getByRole('button', { name: 'Export', exact: true }).click();
  await expect(editor.locator('[data-visual-id="voxel-annotation-readout"]')).toContainText('canonicalJson');

  await editor.getByRole('button', { name: 'convert', exact: true }).click();
  const hashBeforePlan = await projectHash(shell);
  const rendererBeforePlan = await rendererHash(viewport);
  await editor.locator('[data-action="prepare-voxel-conversion"]').click();
  await expect(editor.locator('[data-visual-id="voxel-conversion-preview"]')).toBeVisible();
  await expect(shell).toHaveAttribute('data-project-hash', hashBeforePlan);
  await expect(viewport).toHaveAttribute('data-voxel-preview-kind', 'conversion');
  await expect(viewport).toHaveAttribute('data-preview-applied', 'true');
  await expect.poll(() => rendererHash(viewport)).not.toBe(rendererBeforePlan);
  await editor.getByRole('button', { name: 'Discard', exact: true }).click();
  await expect(editor.locator('[data-visual-id="voxel-conversion-preview"]')).toHaveCount(0);
  await expect(viewport).not.toHaveAttribute('data-voxel-preview-kind', 'conversion');
  await expect(viewport).toHaveAttribute('data-preview-applied', 'false');
  await expect.poll(() => rendererHash(viewport)).toBe(rendererBeforePlan);
  await editor.locator('[data-action="prepare-voxel-conversion"]').click();
  await expect(viewport).toHaveAttribute('data-voxel-preview-kind', 'conversion');
  await editor.locator('[data-action="apply-voxel-conversion"]').click();
  await expect(shell).toHaveAttribute('data-voxel-receipt', 'voxelConversionApplied');
  await expect(shell).toHaveAttribute('data-voxel-assets', '2');
  const persistedHash = await projectHash(shell);

  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', persistedHash);
  await expect(shell).toHaveAttribute('data-voxel-assets', '2');
  await expect(editor).toContainText('2 assets · 2 instances');
  await editor.locator('.item-list').getByRole('button', { name: /voxel-volume\/kenney-wall-a/ }).click();
  await editor.getByRole('button', { name: 'edit', exact: true }).click();
  await expect(editor).toContainText('durable history');
  await editor.getByRole('button', { name: 'annotations', exact: true }).click();
  await expect(editor.getByLabel('Annotation layer')).toContainText('voxel-annotation/studio-semantics');

  const projectPath = join(projectRoot, convertedWallProjectFile);
  const beforeExternalChange = await readFile(projectPath);
  const externallyChanged = Buffer.concat([beforeExternalChange, Buffer.from('\n')]);
  await writeFile(projectPath, externallyChanged);
  await editor.getByRole('button', { name: 'assets', exact: true }).click();
  await editor.getByRole('button', { name: 'Upsert material', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('stale');
  expect(await readFile(projectPath)).toEqual(externallyChanged);
});

test('animated voxel objects convert, discard, apply, attach, reload, and play through the shared renderer', async ({ page }) => {
  test.setTimeout(300_000);
  const projectPath = join(projectRoot, voxelObjectProjectFile);
  await copyFile(join(projectRoot, loadingBayProjectFile), projectPath);
  const packedPlacement = await installPackedPlacementResourceAdapter(page, projectRoot);
  await page.goto(
    `/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(voxelObjectProjectFile)}`,
  );

  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  const editor = page.locator('[data-visual-id="studio-voxel-editor"]');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/, {
    timeout: projectOpenTimeout,
  });
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
  const baselineVoxelDefinitionCount = Number(
    await viewport.getAttribute('data-voxel-object-definitions'),
  );
  const baselineVoxelInstanceCount = Number(
    await viewport.getAttribute('data-voxel-object-instances'),
  );
  expect(baselineVoxelDefinitionCount).toBe(9);
  expect(baselineVoxelInstanceCount).toBe(367);
  const convertedVoxelDefinitionCount = String(baselineVoxelDefinitionCount + 1);
  const materialSection = editor.locator('.voxel-section').filter({
    hasText: 'asset-catalog authority + style',
  });
  await materialSection.getByLabel('Asset id').fill('material/wall-lines');
  const projectHashBeforeMaterial = await projectHash(shell);
  await materialSection.getByRole('button', { name: 'Upsert material', exact: true }).click();
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    projectHashBeforeMaterial,
  );
  await expect(materialSection).toContainText('material/wall-lines');
  const canonicalHash = await projectHash(shell);
  const canonicalRendererHash = await rendererHash(viewport);

  await editor.getByRole('button', { name: 'convert', exact: true }).click();
  await editor.getByRole('button', { name: 'Object / flipbook', exact: true }).click();
  await editor.getByLabel('Source mode').selectOption('animated');
  await editor.getByLabel('Source asset').fill('mesh-animation/kenney-retro-character-medium');
  await editor.getByLabel('Source path').fill('content/assets/kenney-retro-character-medium.glb');
  await editor.getByLabel('License path').fill(
    'content/assets/KENNEY-ANIMATED-CHARACTERS-RETRO-LICENSE.txt',
  );
  await editor.getByLabel('Target asset').fill('voxel-object/studio-character');
  await editor.getByLabel('Resolution X').fill('8');
  await editor.getByLabel('Resolution Y').fill('12');
  await editor.getByLabel('Resolution Z').fill('8');
  await editor.getByLabel('Preview samples').fill('1');
  await expect(editor.getByLabel('Mesh subset (for example group/0)')).toBeDisabled();
  await expect(editor).toContainText(
    'Animated conversion currently admits the complete selected scene',
  );

  await editor.locator('[data-action="inspect-voxel-object-source"]').click();
  const inspection = editor.locator('[data-visual-id="voxel-object-source-inspection"]');
  await expect(inspection).toBeVisible({ timeout: 30_000 });
  await expect(inspection).toContainText(/vertices · [1-9][0-9]* triangles/);
  await expect(inspection).toContainText(/[1-9][0-9]* nodes · [1-9][0-9]* groups/);
  const clipLabels = editor.locator('.clip-list label');
  await expect(clipLabels).toHaveCount(3);
  for (let index = 0; index < await clipLabels.count(); index += 1) {
    const label = clipLabels.nth(index);
    const checkbox = label.getByRole('checkbox');
    if (/run/i.test((await label.textContent()) ?? '')) {
      await checkbox.check();
    } else {
      await checkbox.uncheck();
    }
  }
  await editor.getByLabel('Sample Hz').fill('4');
  await editor.getByLabel('Start seconds').fill('0');
  await editor.getByLabel('End seconds').fill('0.5');
  await editor.getByLabel('End policy').selectOption('includeClipEnd');
  await editor.getByLabel('Default clip').selectOption('run');

  await editor.locator('[data-action="prepare-voxel-object-conversion"]').click();
  let candidate = editor.locator('[data-visual-id="voxel-object-conversion-preview"]');
  await expect(candidate).toBeVisible({ timeout: 30_000 });
  await expect(candidate).toContainText(/stored \/ [2-9][0-9]* sampled frames/);
  await expect(candidate).toContainText('Preview samples are truncated');
  await expect(shell).toHaveAttribute('data-project-hash', canonicalHash);
  await expect.poll(() => rendererHash(viewport)).not.toBe(canonicalRendererHash);
  const candidateFrame = candidate.getByLabel('Preview voxel-object frame');
  expect(Number(await candidateFrame.getAttribute('max'))).toBeGreaterThan(0);
  const candidateFrameZeroHash = await rendererHash(viewport);
  await setRange(candidateFrame, 1);
  await expect.poll(() => rendererHash(viewport)).not.toBe(candidateFrameZeroHash);
  const candidateFrameOneHash = await rendererHash(viewport);
  const candidatePlay = candidate.getByRole('button', { name: 'Play', exact: true });
  const candidatePause = candidate.getByRole('button', { name: 'Pause', exact: true });
  await candidatePlay.click();
  await expect(candidatePlay).toBeDisabled();
  await expect(candidatePause).toBeEnabled();
  await expect.poll(() => rendererHash(viewport), { timeout: 30_000 }).not.toBe(
    candidateFrameOneHash,
  );
  await candidatePause.click();
  await expect(candidatePlay).toBeEnabled();
  await expect(candidatePause).toBeDisabled();

  await candidate.getByRole('button', { name: 'Discard', exact: true }).click();
  await expect(candidate).toHaveCount(0, { timeout: projectMutationTimeout });
  await expect(shell).toHaveAttribute('data-project-hash', canonicalHash);
  await expect.poll(() => rendererHash(viewport)).toBe(canonicalRendererHash);

  await editor.locator('[data-action="prepare-voxel-object-conversion"]').click();
  candidate = editor.locator('[data-visual-id="voxel-object-conversion-preview"]');
  await expect(candidate).toBeVisible({ timeout: 30_000 });
  await candidate.locator('[data-action="apply-voxel-object-conversion"]').click();
  await expect(candidate).toHaveCount(0, { timeout: projectMutationTimeout });
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    canonicalHash,
  );

  const canonicalObjects = editor.locator('.voxel-section').filter({
    hasText: 'project-owned content and transformed instances',
  });
  const canonicalMeshResourceCount = Number(await viewport.getAttribute('data-mesh-resources'));
  await canonicalObjects.getByRole('button', {
    name: /voxel-object\/studio-character/,
  }).click();
  await expect(viewport).toHaveAttribute('data-voxel-preview-kind', 'objectPlacement', {
    timeout: projectMutationTimeout,
  });
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-definitions',
    convertedVoxelDefinitionCount,
  );
  await expect(viewport).toHaveAttribute('data-voxel-object-placement-ghosts', '1');
  await expect(viewport).toHaveAttribute(
    'data-mesh-resources',
    String(canonicalMeshResourceCount + 1),
  );
  await expect.poll(() => packedPlacement.preparedCount, { timeout: projectMutationTimeout }).toBe(
    1,
  );
  await expect.poll(
    () => packedPlacement.resourceReadCount,
    { timeout: projectMutationTimeout },
  ).toBeGreaterThan(0);
  const placementProjectHash = await projectHash(shell);
  const viewportCanvas = page.getByLabel('Shared Rusty renderer viewport');
  await viewportCanvas.focus();
  await viewportCanvas.press('Escape');
  await expect(viewport).not.toHaveAttribute('data-voxel-preview-kind', 'objectPlacement');
  await expect(viewport).toHaveAttribute('data-voxel-object-placement-ghosts', '0');
  await expect(viewport).toHaveAttribute(
    'data-mesh-resources',
    String(canonicalMeshResourceCount),
  );
  await expect(shell).toHaveAttribute('data-project-hash', placementProjectHash);
  const resourceReadsBeforeReprepare = packedPlacement.resourceReadCount;
  await canonicalObjects.locator('[data-action="preview-voxel-object-placement"]').click();
  await expect(viewport).toHaveAttribute('data-voxel-preview-kind', 'objectPlacement', {
    timeout: projectMutationTimeout,
  });
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-definitions',
    convertedVoxelDefinitionCount,
  );
  await expect(viewport).toHaveAttribute('data-voxel-object-placement-ghosts', '1');
  await expect(viewport).toHaveAttribute(
    'data-mesh-resources',
    String(canonicalMeshResourceCount + 1),
  );
  await expect.poll(() => packedPlacement.preparedCount, { timeout: projectMutationTimeout }).toBe(
    2,
  );
  await expect.poll(
    () => packedPlacement.resourceReadCount,
    { timeout: projectMutationTimeout },
  ).toBeGreaterThan(resourceReadsBeforeReprepare);
  await pickVoxelObjectPlacementLocation(page, canonicalObjects, viewport);
  const authoring = editor.locator('[data-visual-id="voxel-object-authoring-readout"]');
  await expect(canonicalObjects).toContainText('convertedAnimatedMesh');
  await expect(authoring).toContainText('content/assets/kenney-retro-character-medium.glb');
  await expect(authoring).toContainText('default clip/run-2');
  await canonicalObjects.getByLabel('Voxel-object placement instance id').fill('studio-character-object');
  await canonicalObjects.getByLabel('Voxel-object placement clip').selectOption({ label: 'run' });
  await canonicalObjects.getByLabel('Voxel-object placement frame').fill('1');
  const axisRows = canonicalObjects.locator('.axis-row');
  await fillAxisRow(axisRows.nth(0), ['4', '1', '8']);
  await fillAxisRow(axisRows.nth(2), ['0.5', '0.5', '0.5']);
  const rowsBeforeAttach = await page.locator('.entity-row').count();
  const rendererBeforeAttach = await rendererHash(viewport);
  await viewportCanvas.focus();
  await viewportCanvas.press('Enter');
  await expect(page.locator('.entity-row')).toHaveCount(rowsBeforeAttach + 1, {
    timeout: projectMutationTimeout,
  });
  await expect.poll(() => rendererHash(viewport)).not.toBe(rendererBeforeAttach);
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-definitions',
    convertedVoxelDefinitionCount,
  );
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-instances',
    String(baselineVoxelInstanceCount + 1),
  );
  await expect(viewport).toHaveAttribute('data-voxel-object-placement-ghosts', '1');
  await expect(canonicalObjects.getByLabel('Voxel-object placement instance id')).toHaveValue(
    'studio-character-object-2',
  );
  const objectRow = hierarchyRow(page, 'studio-character-object');
  await expect(objectRow).toHaveCount(1);
  const ownerEntityId = await objectRow.getAttribute('data-entity-id');
  expect(ownerEntityId).toMatch(/^[1-9][0-9]*$/);
  const durableHash = await projectHash(shell);
  const durableBytes = await readFile(projectPath);

  await objectRow.click();
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  const component = page.locator('[data-visual-id="entity-voxel-object-component"]');
  const playback = component.locator('rusty-voxel-object-playback');
  await expect(component).toContainText('typed entity capability');
  await expect(playback).toContainText('paused', { timeout: 30_000 });
  await expect(playback).toContainText('Saved pose', { timeout: projectMutationTimeout });
  await expect(playback).toContainText('frame 1', { timeout: projectMutationTimeout });
  await expect(shell).toHaveAttribute('data-project-hash', durableHash);

  const appliedFrame = component.getByLabel('Entity voxel-object preview frame');
  await page.waitForTimeout(100);
  const renderedSavedPose = await rendererHash(viewport);
  await setRange(appliedFrame, 0);
  await expect(playback).toContainText('frame 0', { timeout: projectMutationTimeout });
  await expect.poll(() => rendererHash(viewport)).not.toBe(renderedSavedPose);
  const frameZeroRendererHash = await rendererHash(viewport);
  await setRange(appliedFrame, 1);
  await expect(playback).toContainText('frame 1', { timeout: projectMutationTimeout });
  await expect.poll(() => rendererHash(viewport)).not.toBe(frameZeroRendererHash);
  const durableRendererHash = await rendererHash(viewport);
  await setRange(appliedFrame, 0);
  await expect(playback).toContainText('frame 0', { timeout: projectMutationTimeout });
  await expect.poll(() => rendererHash(viewport)).not.toBe(durableRendererHash);
  const scrubbedRendererHash = await rendererHash(viewport);
  await component.locator('[data-action="play-entity-voxel-object"]').click();
  await expect(playback).toContainText('playing', { timeout: projectMutationTimeout });
  await expect.poll(() => rendererHash(viewport), { timeout: 30_000 }).not.toBe(
    scrubbedRendererHash,
  );
  await component.locator('[data-action="pause-entity-voxel-object"]').click();
  await expect(playback).toContainText('paused', { timeout: projectMutationTimeout });
  if (await appliedFrame.inputValue() !== '0') {
    const rendererBeforeDeterministicRestoreSetup = await rendererHash(viewport);
    await setRange(appliedFrame, 0);
    await expect(playback).toContainText('frame 0', { timeout: projectMutationTimeout });
    await expect.poll(() => rendererHash(viewport)).not.toBe(
      rendererBeforeDeterministicRestoreSetup,
    );
  }
  await expect(playback).toContainText('frame 0', { timeout: projectMutationTimeout });
  const rendererBeforeRestore = await rendererHash(viewport);
  await component.locator('[data-action="restore-entity-voxel-object"]').click();
  await expect(playback).toContainText('stopped', { timeout: projectMutationTimeout });
  await expect.poll(() => rendererHash(viewport)).not.toBe(rendererBeforeRestore);
  await expect(shell).toHaveAttribute('data-project-hash', durableHash);
  expect(await readFile(projectPath)).toEqual(durableBytes);

  await page.getByRole('button', { name: 'Voxel', exact: true }).click();
  await editor.getByRole('button', { name: 'convert', exact: true }).click();
  await editor.getByRole('button', { name: 'Object / flipbook', exact: true }).click();
  await editor.getByRole('button', { name: /voxel-object\/studio-character/ }).click();
  const placementHistory = editor.locator('[data-visual-id="voxel-object-placement-history"]');
  await expect(placementHistory).toContainText('studio-character-object · placed');
  await placementHistory.locator('[data-action="undo-voxel-object-placement"]').click();
  await expect(objectRow).toHaveCount(0, { timeout: projectMutationTimeout });
  await expect(placementHistory).toContainText('studio-character-object · undone');
  const hashAfterUndo = await projectHash(shell);
  expect(hashAfterUndo).not.toBe(durableHash);
  await placementHistory.locator('[data-action="reapply-voxel-object-placement"]').click();
  await expect(objectRow).toHaveCount(1, { timeout: projectMutationTimeout });
  await expect(placementHistory).toContainText('studio-character-object · placed');
  const reappliedHash = await projectHash(shell);
  expect(reappliedHash).not.toBe(hashAfterUndo);

  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', reappliedHash, {
    timeout: projectOpenTimeout,
  });
  const reopenedRow = hierarchyRow(page, 'studio-character-object');
  await expect(reopenedRow).toHaveCount(1, { timeout: projectOpenTimeout });
  await reopenedRow.click();
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  await expect(page.locator('[data-visual-id="entity-voxel-object-component"]')).toContainText(
    'clip/run-2 frame 1',
  );

  await page.getByRole('button', { name: 'Voxel', exact: true }).click();
  await editor.getByRole('button', { name: 'convert', exact: true }).click();
  await editor.getByRole('button', { name: 'Object / flipbook', exact: true }).click();
  const reopenedObjects = editor.locator('.voxel-section').filter({
    hasText: 'project-owned content and transformed instances',
  });
  await reopenedObjects.getByRole('button', { name: /voxel-object\/studio-character/ }).click();
  await reopenedObjects.getByLabel('Voxel-object placement instance id').fill(
    'studio-character-object-2',
  );
  await reopenedObjects.getByLabel('Voxel-object placement clip').selectOption({ label: 'run' });
  await reopenedObjects.getByLabel('Voxel-object placement frame').fill('1');
  await reopenedObjects.getByLabel('Voxel-object placement translation X').fill('6');
  await reopenedObjects.getByLabel('Voxel-object placement translation Y').fill('1');
  await reopenedObjects.getByLabel('Voxel-object placement translation Z').fill('8');
  await reopenedObjects.getByLabel('Voxel-object placement scale X').fill('0.5');
  await reopenedObjects.getByLabel('Voxel-object placement scale Y').fill('0.5');
  await reopenedObjects.getByLabel('Voxel-object placement scale Z').fill('0.5');
  const hashBeforeSecondPlacement = await projectHash(shell);
  await reopenedObjects.locator('[data-action="attach-voxel-object-instance"]').click();
  const secondRow = hierarchyRow(page, 'studio-character-object-2');
  await expect(secondRow).toHaveCount(1, { timeout: projectMutationTimeout });
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashBeforeSecondPlacement,
  );
  const secondPlacementHash = await projectHash(shell);
  await page.waitForTimeout(100);
  await expect(shell).toHaveAttribute('data-project-hash', secondPlacementHash);
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-definitions',
    convertedVoxelDefinitionCount,
  );
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-instances',
    String(baselineVoxelInstanceCount + 2),
  );

  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  const duplicatePanel = page.locator('[data-visual-id="entity-voxel-object-duplicate"]');
  await expect(duplicatePanel.locator('[data-action="duplicate-voxel-object-instance"]'))
    .toBeEnabled({ timeout: 30_000 });
  await duplicatePanel.getByLabel('Duplicate voxel-object instance id').fill(
    'studio-character-object-copy',
  );
  await duplicatePanel.getByLabel('Duplicate voxel-object translation X').fill('8');
  const hashBeforeDuplicate = await projectHash(shell);
  await duplicatePanel.locator('[data-action="duplicate-voxel-object-instance"]').click();
  const duplicateRow = hierarchyRow(page, 'studio-character-object-copy');
  await expect(duplicateRow).toHaveCount(1, { timeout: projectMutationTimeout });
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashBeforeDuplicate,
  );
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-definitions',
    convertedVoxelDefinitionCount,
  );
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-instances',
    String(baselineVoxelInstanceCount + 3),
  );
  const duplicateOwnerEntityId = await duplicateRow.getAttribute('data-entity-id');
  expect(duplicateOwnerEntityId).toMatch(/^[1-9][0-9]*$/);
  expect(duplicateOwnerEntityId).not.toBe(ownerEntityId);
  await expect(shell).toHaveAttribute('data-selected-entity', duplicateOwnerEntityId as string);

  await expect(shell).toHaveAttribute('data-studio-operation', 'idle', { timeout: 30_000 });
  const duplicateTranslationX = page.getByLabel('Translation X', { exact: true });
  await duplicateTranslationX.fill('8.5');
  const hashBeforeDuplicateTransform = await projectHash(shell);
  await page.locator('[data-action="commit-transform"]').click();
  await expect.poll(() => projectHash(shell), { timeout: projectMutationTimeout }).not.toBe(
    hashBeforeDuplicateTransform,
  );

  await page.getByRole('button', { name: 'Voxel', exact: true }).click();
  await editor.getByRole('button', { name: 'convert', exact: true }).click();
  await editor.getByRole('button', { name: 'Object / flipbook', exact: true }).click();
  await editor.getByRole('button', { name: /voxel-object\/studio-character/ }).click();
  const duplicateHistory = editor.locator('[data-visual-id="voxel-object-placement-history"]');
  await expect(duplicateHistory).toContainText('studio-character-object-copy · placed');
  await duplicateHistory.locator('[data-action="undo-voxel-object-placement"]').click();
  await expect(duplicateRow).toHaveCount(0, { timeout: projectMutationTimeout });
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-instances',
    String(baselineVoxelInstanceCount + 2),
  );
  await duplicateHistory.locator('[data-action="reapply-voxel-object-placement"]').click();
  await expect(duplicateRow).toHaveCount(1, { timeout: projectMutationTimeout });
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-definitions',
    convertedVoxelDefinitionCount,
  );
  await expect(viewport).toHaveAttribute(
    'data-voxel-object-instances',
    String(baselineVoxelInstanceCount + 3),
  );
  const finalHash = await projectHash(shell);

  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', finalHash, {
    timeout: projectOpenTimeout,
  });
  await expect(hierarchyRow(page, 'studio-character-object')).toHaveCount(1, {
    timeout: projectOpenTimeout,
  });
  await expect(hierarchyRow(page, 'studio-character-object-2')).toHaveCount(1, {
    timeout: projectOpenTimeout,
  });
  const finalDuplicateRow = hierarchyRow(page, 'studio-character-object-copy');
  await expect(finalDuplicateRow).toHaveCount(1, { timeout: projectOpenTimeout });
  await finalDuplicateRow.click();
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  await expect(page.getByLabel('Translation X', { exact: true })).toHaveValue('8.5');
  process.stdout.write(`${JSON.stringify({
    kind: 'studioVoxelObjectBrowserEvidence',
    source: 'mesh-animation/kenney-retro-character-medium',
    asset: 'voxel-object/studio-character',
    instance: 'studio-character-object',
    ownerEntityId,
    duplicateOwnerEntityId,
    retainedVoxelObjectDefinitions: 1,
    retainedVoxelObjectInstances: 3,
    placementUndoReapplied: true,
    discardedWithoutMutation: true,
    playbackChangedBytes: false,
    freshPageReadoutMatched: true,
  })}\n`);
});

function hierarchyRow(page: Page, label: string): Locator {
  return page.locator('.entity-row').filter({
    has: page.getByText(label, { exact: true }),
  });
}

async function pickVoxelObjectPlacementLocation(
  page: Page,
  palette: Locator,
  viewport: Locator,
): Promise<void> {
  const canvas = page.getByLabel('Shared Rusty renderer viewport');
  const translation = [
    palette.getByLabel('Voxel-object placement translation X'),
    palette.getByLabel('Voxel-object placement translation Y'),
    palette.getByLabel('Voxel-object placement translation Z'),
  ];
  const before = await Promise.all(translation.map((input) => input.inputValue()));
  const box = await canvas.boundingBox();
  if (box === null) throw new Error('shared renderer canvas has no browser bounds');
  for (const yFraction of [0.6, 0.5, 0.7, 0.4]) {
    for (const xFraction of [0.5, 0.4, 0.6, 0.3, 0.7]) {
      const revision = await viewport.getAttribute('data-pick-revision');
      await canvas.click({ position: { x: box.width * xFraction, y: box.height * yFraction } });
      await expect(viewport).not.toHaveAttribute('data-pick-revision', revision ?? '0');
      const after = await Promise.all(translation.map((input) => input.inputValue()));
      if (after.some((value, index) => value !== before[index])) {
        const alert = page.getByRole('alert');
        if (await alert.isVisible()) await alert.getByRole('button', { name: 'Dismiss' }).click();
        return;
      }
    }
  }
  throw new Error('voxel-object placement did not receive a shared-renderer surface pick');
}

async function pickVisibleEntity(page: Page, shell: Locator): Promise<string> {
  const canvas = page.getByLabel('Shared Rusty renderer viewport');
  const viewport = page.locator('rusty-studio-viewport');
  const box = await canvas.boundingBox();
  if (box === null) throw new Error('shared renderer canvas has no browser bounds');
  for (const yFraction of [0.35, 0.45, 0.55, 0.65, 0.75]) {
    for (const xFraction of [0.25, 0.35, 0.45, 0.55, 0.65, 0.75]) {
      const revision = await viewport.getAttribute('data-pick-revision');
      await canvas.click({ position: { x: box.width * xFraction, y: box.height * yFraction } });
      await expect(viewport).not.toHaveAttribute('data-pick-revision', revision ?? '0');
      const source = await shell.getAttribute('data-selection-source');
      const entity = await shell.getAttribute('data-selected-entity');
      if (source === 'renderer' && entity !== null && entity.length > 0) return entity;
    }
  }
  throw new Error('shared renderer picking did not hit any projected Loading Bay entity');
}

async function pickVisibleVoxel(page: Page, shell: Locator): Promise<void> {
  const canvas = page.getByLabel('Shared Rusty renderer viewport');
  const viewport = page.locator('rusty-studio-viewport');
  const box = await canvas.boundingBox();
  if (box === null) throw new Error('shared renderer canvas has no browser bounds');
  for (const yFraction of [0.35, 0.45, 0.55, 0.65, 0.75]) {
    for (const xFraction of [0.25, 0.35, 0.45, 0.55, 0.65, 0.75]) {
      const revision = await viewport.getAttribute('data-pick-revision');
      await canvas.click({ position: { x: box.width * xFraction, y: box.height * yFraction } });
      await expect(viewport).not.toHaveAttribute('data-pick-revision', revision ?? '0');
      await page.waitForTimeout(100);
      if ((await shell.getAttribute('data-voxel-pick-instance')) !== null) return;
      const alert = page.getByRole('alert');
      if (await alert.isVisible()) await alert.getByRole('button', { name: 'Dismiss' }).click();
    }
  }
  throw new Error('shared renderer picking did not produce a Rust-validated voxel anchor');
}

async function setRange(locator: Locator, value: number): Promise<void> {
  await locator.evaluate((element, nextValue) => {
    if (!(element instanceof HTMLInputElement)) {
      throw new TypeError('range control is not an input element');
    }
    element.value = String(nextValue);
    element.dispatchEvent(new Event('input', { bubbles: true }));
    element.dispatchEvent(new Event('change', { bubbles: true }));
  }, value);
}

async function fillAxisRow(row: Locator, values: readonly string[]): Promise<void> {
  const inputs = row.locator('input');
  await expect(inputs).toHaveCount(values.length);
  for (let index = 0; index < values.length; index += 1) {
    await inputs.nth(index).fill(values[index] ?? '');
  }
}

async function transformHandlePoint(
  page: Page,
  inspector: Locator,
  tool: 'translate' | 'rotate' | 'scale',
  axis: 0 | 1 | 2,
): Promise<{ readonly x: number; readonly y: number }> {
  const canvas = page.getByLabel('Shared Rusty renderer viewport');
  const box = await canvas.boundingBox();
  if (box === null) throw new Error('shared renderer canvas has no browser bounds');
  const translation = await Promise.all(['X', 'Y', 'Z'].map(async (name) =>
    Number(await inspector.getByLabel(`Translation ${name}`).inputValue()),
  )) as [number, number, number];
  const rotation = await Promise.all(['X', 'Y', 'Z', 'W'].map(async (name) =>
    Number(await inspector.getByLabel(`Rotation ${name}`).inputValue()),
  )) as [number, number, number, number];
  const direction = rotateVector(rotation, axis === 0 ? [1, 0, 0] : axis === 1 ? [0, 1, 0] : [0, 0, 1]);
  const distance = tool === 'rotate' ? 0.78 : 0.62;
  const world: [number, number, number] = [
    translation[0] + direction[0] * distance,
    translation[1] + direction[1] * distance,
    translation[2] + direction[2] * distance,
  ];
  const projected = projectWorldPoint(
    world,
    [15, 13, 22],
    [4.5, 1.5, 7],
    55,
    box.width,
    box.height,
  );
  return { x: box.x + projected[0], y: box.y + projected[1] };
}

function projectWorldPoint(
  world: readonly [number, number, number],
  cameraPosition: readonly [number, number, number],
  cameraTarget: readonly [number, number, number],
  fovYDegrees: number,
  width: number,
  height: number,
): readonly [number, number] {
  const forward = normalize(subtract(cameraTarget, cameraPosition));
  const right = normalize(cross(forward, [0, 1, 0]));
  const up = normalize(cross(right, forward));
  const offset = subtract(world, cameraPosition);
  const depth = dot(offset, forward);
  const tangent = Math.tan(fovYDegrees * Math.PI / 360);
  const x = dot(offset, right) / (depth * tangent * width / height);
  const y = dot(offset, up) / (depth * tangent);
  return [(x + 1) * width / 2, (1 - y) * height / 2];
}

function rotateVector(
  rotation: readonly [number, number, number, number],
  vector: readonly [number, number, number],
): readonly [number, number, number] {
  const [x, y, z, w] = rotation;
  const tx = 2 * (y * vector[2] - z * vector[1]);
  const ty = 2 * (z * vector[0] - x * vector[2]);
  const tz = 2 * (x * vector[1] - y * vector[0]);
  return [
    vector[0] + w * tx + (y * tz - z * ty),
    vector[1] + w * ty + (z * tx - x * tz),
    vector[2] + w * tz + (x * ty - y * tx),
  ];
}

function subtract(
  left: readonly [number, number, number],
  right: readonly [number, number, number],
): readonly [number, number, number] {
  return [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
}

function cross(
  left: readonly [number, number, number],
  right: readonly [number, number, number],
): readonly [number, number, number] {
  return [
    left[1] * right[2] - left[2] * right[1],
    left[2] * right[0] - left[0] * right[2],
    left[0] * right[1] - left[1] * right[0],
  ];
}

function normalize(vector: readonly [number, number, number]): readonly [number, number, number] {
  const length = Math.hypot(...vector);
  return [vector[0] / length, vector[1] / length, vector[2] / length];
}

function dot(
  left: readonly [number, number, number],
  right: readonly [number, number, number],
): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2];
}

async function projectHash(shell: Locator): Promise<string> {
  return (await shell.getAttribute('data-project-hash')) ?? '';
}

async function projectAssetCount(shell: Locator): Promise<number> {
  const value = Number(await shell.getAttribute('data-project-assets'));
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`invalid project asset count: ${String(value)}`);
  }
  return value;
}

async function rendererHash(viewport: Locator): Promise<string> {
  return (await viewport.getAttribute('data-authored-frame-hash')) ?? '';
}

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`${name} is required`);
  return value;
}
