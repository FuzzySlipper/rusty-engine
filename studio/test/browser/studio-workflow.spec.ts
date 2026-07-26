import { expect, test, type Locator, type Page } from '@playwright/test';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const projectRoot = requiredEnvironment('RUSTY_STUDIO_PROJECT_ROOT');
const loadingBayProjectFile = 'content/projects/loading-bay.project.json';
const convertedWallProjectFile = 'content/projects/converted-wall.project.json';

test('real project hierarchy, shared picking, transform settlement, reopen, and rejection stay coherent', async ({ page }) => {
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(loadingBayProjectFile)}`);

  const shell = page.locator('[data-visual-id="studio-shell"]');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/);
  const viewport = page.locator('rusty-studio-viewport');
  await expect.poll(async () => {
    const status = await viewport.getAttribute('data-renderer-status');
    if (status === 'error') {
      throw new Error((await page.getByRole('alert').textContent()) ?? 'shared renderer failed');
    }
    return status;
  }).toBe('ready');
  await expect(page.locator('.entity-row')).toHaveCount(8);
  await expect(viewport).toHaveAttribute(
    'data-retained-ops',
    /^[1-9][0-9]*$/,
  );
  await expect(viewport).toHaveAttribute('data-authored-frame-hash', /.+/);

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
  const translationX = page.getByLabel('Translation X');
  const initialX = Number(await translationX.inputValue());
  const committedX = initialX + 0.5;
  const hashBeforeCommit = await projectHash(shell);
  await expect(viewport).toHaveAttribute('data-selected-render-handle', /[0-9]+/);
  const selectedRendererHash = await rendererHash(viewport);

  await translationX.fill(String(committedX));
  await expect(page.locator('[data-preview-active="true"]')).toBeVisible();
  await expect(viewport).toHaveAttribute('data-preview-applied', 'true');
  await expect.poll(() => rendererHash(viewport)).not.toBe(selectedRendererHash);
  await page.locator('.inspector-actions').getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(viewport).toHaveAttribute('data-preview-applied', 'false');
  await expect(viewport).toHaveAttribute('data-authored-frame-hash', selectedRendererHash);

  await translationX.fill(String(committedX));
  await expect(viewport).toHaveAttribute('data-preview-applied', 'true');
  await page.locator('[data-action="commit-transform"]').click();
  await expect.poll(() => projectHash(shell)).not.toBe(hashBeforeCommit);
  const committedHash = await projectHash(shell);
  await expect(translationX).toHaveValue(String(committedX));
  await expect(page.locator('[data-preview-active="true"]')).toHaveCount(0);
  await expect(viewport).toHaveAttribute('data-preview-applied', 'false');

  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(shell).toHaveAttribute('data-project-hash', committedHash);
  await expect(translationX).toHaveValue(String(committedX));

  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', committedHash);
  await page.locator('.entity-row[data-entity-id="1"]').click();
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  await expect(page.getByLabel('Translation X')).toHaveValue(String(committedX));

  const hashBeforeInvalidCommit = await projectHash(shell);
  await page.getByLabel('Translation X').fill('2000000');
  await page.locator('[data-action="commit-transform"]').click();
  await expect(page.getByRole('alert')).toContainText('invalid-scene-after-edit');
  await expect(shell).toHaveAttribute('data-project-hash', hashBeforeInvalidCommit);
  await expect(page.locator('[data-preview-active="true"]')).toBeVisible();
});

test('project, scene, entity, light, and capability authoring flow through named Rust operations', async ({ page }) => {
  await page.goto('/');
  const shell = page.locator('[data-visual-id="studio-shell"]');

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

  await page.locator('.entity-row[data-entity-id="42"]').click();
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  const inspector = page.locator('.inspector-panel');
  await inspector.getByLabel('Rotation Z').fill('0.7071068');
  await inspector.getByLabel('Rotation W').fill('0.7071068');
  const transformHash = await projectHash(shell);
  await inspector.locator('[data-action="commit-transform"]').click();
  await expect.poll(() => projectHash(shell)).not.toBe(transformHash);
  await expect(inspector.getByLabel('Rotation Z')).toHaveValue('0.7071068');

  const name = inspector.locator('.field-row').filter({ hasText: 'Name' }).locator('input');
  await name.fill('Sun Key');
  await inspector.getByRole('button', { name: 'Rename', exact: true }).click();
  await expect(page.locator('.entity-row[data-entity-id="42"]')).toContainText('Sun Key');

  const collisionHash = await projectHash(shell);
  await inspector.locator('.inspector-section').filter({ hasText: 'Collision' })
    .getByRole('button', { name: 'Apply', exact: true }).click();
  await expect.poll(() => projectHash(shell)).not.toBe(collisionHash);
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
  await expect.poll(() => projectHash(shell)).not.toBe(scaleHash);
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
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(loadingBayProjectFile)}`);
  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/);
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

  await page.reload();
  await expect(shell).toHaveAttribute('data-user-settings-status', 'loaded');
  await expect(shell).toHaveAttribute('data-camera-speed', '12');
  await expect(shell).toHaveAttribute('data-move-forward', 'ArrowUp');
  await expect(viewport).toHaveAttribute('data-camera-move-speed', '12');
  await expect(viewport).toHaveAttribute('data-camera-move-forward', 'ArrowUp');

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
  await expect(shell).toHaveAttribute('data-project-assets', '9');
  await expect.poll(() => projectHash(shell)).not.toBe(hashBeforePlan);

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
  await expect(page.locator('.entity-row[data-entity-id="70"]')).toContainText('Imported Triangle');
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
  await expect(importedAsset).toContainText('contentChanged');

  const hashBeforeReimport = await projectHash(shell);
  const rendererBeforeReimport = await rendererHash(viewport);
  await page.locator('.asset-detail').getByRole('button', { name: 'Prepare reimport' }).click();
  await expect(plan).toContainText('visualUpdate · mesh/studio-triangle');
  await expect(shell).toHaveAttribute('data-project-hash', hashBeforeReimport);
  await plan.getByRole('button', { name: 'Apply atomically', exact: true }).click();
  await expect.poll(() => projectHash(shell)).not.toBe(hashBeforeReimport);
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
  await expect(shell).toHaveAttribute('data-project-hash', persistedHash);
  await expect(shell).toHaveAttribute('data-project-assets', '9');
  await expect(shell).toHaveAttribute('data-user-settings-status', 'loaded');
  await expect(page.locator('.entity-row[data-entity-id="70"]')).toContainText('Imported Triangle');
});

test('trusted host browsing restores focus and animated appearance uses the shared renderer', async ({ page }) => {
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(loadingBayProjectFile)}`);
  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/);
  await expect.poll(async () => {
    const status = await viewport.getAttribute('data-renderer-status');
    if (status === 'error') {
      throw new Error((await viewport.getAttribute('data-renderer-error')) ?? 'shared renderer failed');
    }
    return status;
  }).toBe('ready');
  await expect(viewport).toHaveAttribute('data-animated-mesh-resources', '1');

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
  const gizmo = page.getByLabel('Transform gizmo');
  await expect(gizmo).toHaveAttribute('data-tool', 'rotate');
  await page.getByRole('button', { name: 'world', exact: true }).click();
  await expect(gizmo).toHaveAttribute('data-orientation', 'local');
  const xAxis = page.getByRole('button', { name: 'Drag X transform axis' });
  const axisBox = await xAxis.boundingBox();
  if (axisBox === null) throw new Error('transform gizmo axis has no browser bounds');
  await page.mouse.move(axisBox.x + axisBox.width / 2, axisBox.y + axisBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(axisBox.x + axisBox.width / 2 + 24, axisBox.y + axisBox.height / 2);
  await page.mouse.up();
  await expect(inspector.getByLabel('Rotation X')).not.toHaveValue('0');
  await inspector.locator('[data-action="commit-transform"]').click();
  await expect.poll(() => projectHash(shell)).not.toBe(hashBeforeTransform);

  await page.getByTitle('Scale gizmo').click();
  await expect(gizmo).toHaveAttribute('data-tool', 'scale');
  await inspector.locator('.inspector-actions').getByRole('button', { name: 'Cancel', exact: true }).click();

  const appearance = inspector.locator('.inspector-section').filter({ hasText: 'Appearance' });
  await appearance.getByLabel('Kind').selectOption('animatedMesh');
  await appearance.getByLabel('Asset').selectOption('mesh-animation/kenney-retro-character-medium');
  await appearance.getByLabel('Clip').selectOption('run');
  const hashBeforeAppearance = await projectHash(shell);
  await appearance.getByRole('button', { name: 'Apply appearance', exact: true }).click();
  await expect.poll(() => projectHash(shell)).not.toBe(hashBeforeAppearance);
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
  await expect(shell).toHaveAttribute('data-project-hash', animatedHash);
  await expect(shell).toHaveAttribute('data-animated-instance-clips', /(?:^|,)run(?:,|$)/);
  await expect(viewport).toHaveAttribute('data-animated-mesh-resources', '1');
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
});

test('voxel Studio owns the complete shared-renderer authoring workflow and rejects stale writes atomically', async ({ page }) => {
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(convertedWallProjectFile)}`);

  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  const editor = page.locator('[data-visual-id="studio-voxel-editor"]');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/);
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
  await expect.poll(() => projectHash(shell)).not.toBe(hashBeforeBrush);
  const hashAfterBrush = await projectHash(shell);

  await editor.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(shell).toHaveAttribute('data-voxel-receipt', 'voxelHistoryMoved');
  await expect.poll(() => projectHash(shell)).not.toBe(hashAfterBrush);
  const hashAfterUndo = await projectHash(shell);
  await editor.getByRole('button', { name: 'Redo', exact: true }).click();
  await expect.poll(() => projectHash(shell)).not.toBe(hashAfterUndo);

  await editor.getByRole('button', { name: 'annotations', exact: true }).click();
  const hashBeforeAnnotation = await projectHash(shell);
  await editor.getByRole('button', { name: 'Create from pick', exact: true }).click();
  await expect(shell).toHaveAttribute('data-voxel-receipt', 'voxelAnnotationCreated');
  await expect.poll(() => projectHash(shell)).not.toBe(hashBeforeAnnotation);
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

async function projectHash(shell: Locator): Promise<string> {
  return (await shell.getAttribute('data-project-hash')) ?? '';
}

async function rendererHash(viewport: Locator): Promise<string> {
  return (await viewport.getAttribute('data-authored-frame-hash')) ?? '';
}

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`${name} is required`);
  return value;
}
