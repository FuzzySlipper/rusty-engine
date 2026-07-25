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
