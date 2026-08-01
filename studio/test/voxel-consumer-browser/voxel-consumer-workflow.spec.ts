import { expect, test, type Locator } from '@playwright/test';
import { performance } from 'node:perf_hooks';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const projectRoot = requiredEnvironment('RUSTY_STUDIO_PROJECT_ROOT');
const projectFile = requiredEnvironment('RUSTY_STUDIO_PROJECT_FILE');
const runtimeReportFile = requiredEnvironment('RUSTY_STUDIO_RUNTIME_REPORT');
const engineCommit = requiredEnvironment('RUSTY_STUDIO_ENGINE_COMMIT');
const CHECKER_PNG = Uint8Array.from([
  137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82,
  0, 0, 0, 2, 0, 0, 0, 1, 8, 6, 0, 0, 0, 244, 34, 127, 138,
  0, 0, 0, 15, 73, 68, 65, 84, 120, 156, 99, 248, 207, 0, 68, 255,
  25, 26, 0, 16, 121, 3, 126, 153, 113, 48, 89, 0, 0, 0, 0, 73,
  69, 78, 68, 174, 66, 96, 130,
]);

test.describe.configure({ mode: 'serial' });

test('exact pinned voxel consumer reopens and visibly plays its Studio-authored flipbook', async ({ page }) => {
  test.setTimeout(90_000);
  const projectPath = join(projectRoot, projectFile);
  const durableBytes = await readFile(projectPath);
  const runtimeEvidence = JSON.parse(
    await readFile(join(projectRoot, runtimeReportFile), 'utf8'),
  ) as RuntimeEvidenceReport;

  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(projectFile)}`);

  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  const canvas = page.getByLabel('Shared Rusty renderer viewport');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/);
  await expect.poll(async () => {
    const status = await viewport.getAttribute('data-renderer-status');
    if (status === 'error') {
      throw new Error((await viewport.getAttribute('data-renderer-error')) ?? 'shared renderer failed');
    }
    return status;
  }).toBe('ready');
  await expect(viewport).toHaveAttribute('data-retained-ops', /^[1-9][0-9]*$/);
  await expect(viewport).toHaveAttribute('data-authored-frame-hash', /.+/);
  const projectHash = await requiredAttribute(shell, 'data-project-hash');
  const canonicalDefaultHash = await rendererHash(viewport);

  const objectRow = page.locator('.entity-row[data-entity-id="1"]');
  await expect(objectRow).toContainText('retro-character');
  const cameraRevision = Number(await viewport.getAttribute('data-camera-revision'));
  await objectRow.dblclick();
  await expect.poll(async () => Number(await viewport.getAttribute('data-camera-revision')))
    .toBe(cameraRevision + 1);
  await expect(canvas).toBeFocused();
  await expect(viewport).toHaveAttribute('data-selected-entity', '1');

  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  const component = page.locator('[data-visual-id="entity-voxel-object-component"]');
  const playback = component.locator('rusty-voxel-object-playback');
  await expect(component).toContainText('typed entity capability');
  await expect(playback).toContainText('Saved pose');
  await expect(playback).toContainText('default frame');
  await expect(playback).toContainText('3 clips');
  await expect(playback).toContainText('paused', { timeout: 30_000 });
  await expect(shell).toHaveAttribute('data-project-hash', projectHash);

  const clip = component.getByLabel('Entity voxel-object preview clip');
  const loop = component.getByLabel('Entity voxel-object loop mode');
  const frame = component.getByLabel('Entity voxel-object preview frame');
  const play = component.locator('[data-action="play-entity-voxel-object"]');
  const pause = component.locator('[data-action="pause-entity-voxel-object"]');
  const restore = component.locator('[data-action="restore-entity-voxel-object"]');
  await expect(clip).toHaveValue('clip/idle');
  const initialPreviewHash = await rendererHash(viewport);
  await restore.click();
  await expect(playback).toContainText('stopped · saved pose');
  await expect.poll(() => rendererHash(viewport)).not.toBe(initialPreviewHash);
  const selectedDefaultPixels = await canvas.screenshot();

  await clip.selectOption('clip/run');
  await expect(playback).toContainText('paused · clip/run · frame 0');
  await expect.poll(() => rendererHash(viewport)).not.toBe(canonicalDefaultHash);
  const runFrameZeroHash = await rendererHash(viewport);
  const runFrameZeroPixels = await canvas.screenshot();
  expect(runFrameZeroPixels.equals(selectedDefaultPixels)).toBe(false);

  const acknowledgementMilliseconds: number[] = [];
  acknowledgementMilliseconds.push(await scrubAndWait(frame, viewport, playback, 1));
  const runFrameOneHash = await rendererHash(viewport);
  const runFrameOnePixels = await canvas.screenshot();
  expect(runFrameOneHash).not.toBe(runFrameZeroHash);
  expect(runFrameOnePixels.equals(runFrameZeroPixels)).toBe(false);

  await loop.selectOption('repeat');
  await expect(playback).toContainText('paused');
  await expect(play).toBeEnabled();
  const repeatStartHash = await rendererHash(viewport);
  await play.click();
  await expect(playback).toContainText('playing');
  await expect.poll(() => rendererHash(viewport), { timeout: 30_000 }).not.toBe(repeatStartHash);
  const repeatMidHash = await rendererHash(viewport);
  await expect.poll(() => rendererHash(viewport), { timeout: 30_000 }).not.toBe(repeatMidHash);
  await pause.click();
  await expect(playback).toContainText('paused');
  const pausedHash = await rendererHash(viewport);
  await page.waitForTimeout(350);
  expect(await rendererHash(viewport)).toBe(pausedHash);

  await play.click();
  await expect(playback).toContainText('playing');
  await expect.poll(() => rendererHash(viewport), { timeout: 30_000 }).not.toBe(pausedHash);
  await pause.click();
  await expect(playback).toContainText('paused');

  await loop.selectOption('once');
  await expect(playback).toContainText('paused');
  if (await frame.inputValue() === '0') {
    acknowledgementMilliseconds.push(await scrubAndWait(frame, viewport, playback, 1));
  }
  acknowledgementMilliseconds.push(await scrubAndWait(frame, viewport, playback, 0));
  await play.click();
  await expect(playback).toContainText('playing');
  await expect(playback).toContainText('frame 3 · ended', { timeout: 30_000 });
  await expect(play).toBeEnabled();

  const terminalHash = await rendererHash(viewport);
  await restore.click();
  await expect(playback).toContainText('stopped · saved pose');
  await expect.poll(() => rendererHash(viewport)).not.toBe(terminalHash);
  await expect(shell).toHaveAttribute('data-project-hash', projectHash);
  expect(await readFile(projectPath)).toEqual(durableBytes);

  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', projectHash);
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
  await expect(viewport).toHaveAttribute('data-authored-frame-hash', /.+/);
  await objectRow.click();
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  await expect(component).toContainText('Saved pose');
  await expect(component).toContainText('default frame');
  expect(await readFile(projectPath)).toEqual(durableBytes);

  expect(runtimeEvidence.runtime.behavior.onceEnded).toBe(true);
  expect(runtimeEvidence.runtime.behavior.repeatWrappedToFirstFrame).toBe(true);
  expect(runtimeEvidence.runtime.behavior.missingAssetRejected).toBe(true);
  expect(runtimeEvidence.runtime.behavior.corruptAssetRejected).toBe(true);
  expect(runtimeEvidence.runtime.behavior.postureRoundTripMatched).toBe(true);
  expect(runtimeEvidence.runtime.behavior.projectReopenMatched).toBe(true);
  expect(runtimeEvidence.runtime.behavior.collisionStayedStableDuringPlayback).toBe(true);
  expect(runtimeEvidence.runtime.behavior.collisionKind).toBe('stableFrame');

  process.stdout.write(`${JSON.stringify({
    kind: 'studioVoxelConsumerBrowserEvidence',
    engineRevision: engineCommit,
    evidenceEngineRevision: runtimeEvidence.runtime.engineRevision,
    projectFile,
    projectHash,
    savedPose: 'default',
    selectedClip: 'clip/run',
    repeatObserved: true,
    pauseResumeObserved: true,
    onceTerminalFrame: 3,
    reopenMatched: true,
    visibleCanvasFramesDiffered: true,
    sharedRendererFrameAcknowledgementMilliseconds: acknowledgementMilliseconds,
    gpuTiming: 'unavailable: renderer does not expose timer queries',
    durableBytesChanged: false,
  })}\n`);
});

test('runtime voxel surfaces author repeat and atlas mappings through Rust and reopen visibly', async ({ page }) => {
  test.setTimeout(90_000);
  const projectPath = join(projectRoot, projectFile);
  const textureDirectory = join(projectRoot, 'content/textures');
  const texturePath = join(textureDirectory, 'studio-surface.png');
  await mkdir(textureDirectory, { recursive: true });
  await writeFile(texturePath, CHECKER_PNG);

  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(projectFile)}`);
  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  const canvas = page.getByLabel('Shared Rusty renderer viewport');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/);
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
  const initialProjectHash = await requiredAttribute(shell, 'data-project-hash');
  const initialRendererHash = await rendererHash(viewport);
  const initialPixels = await canvas.screenshot();

  await page.getByRole('button', { name: 'Voxel', exact: true }).click();
  const editor = page.locator('[data-visual-id="studio-voxel-editor"]');
  await editor.getByRole('button', { name: 'surfaces', exact: true }).click();
  const surfaces = editor.locator('[data-visual-id="voxel-surface-authoring"]');
  await expect(surfaces).toContainText('Runtime voxel surfaces');
  await surfaces.getByLabel('PNG source').fill('content/textures/studio-surface.png');
  await surfaces.getByLabel('Scene id').fill('scene/voxel-lab');
  await surfaces.getByLabel('Voxel instance id').fill('retro-character');
  await surfaces.getByLabel('Material slot').fill('1');
  await surfaces.locator('[data-action="apply-voxel-surface"]').click();
  await expect(shell).toHaveAttribute('data-studio-operation', 'voxel');
  await expect(shell).toHaveAttribute('data-studio-operation', 'idle', { timeout: 30_000 });
  const repeatAlerts = await page.getByRole('alert').allInnerTexts();
  if (repeatAlerts.length > 0) {
    throw new Error(`repeat surface rejected: ${repeatAlerts.join(' | ')}`);
  }
  await expect(shell).toHaveAttribute('data-voxel-receipt', 'voxelSurfaceMaterialUpserted', {
    timeout: 30_000,
  });
  await expect(shell).not.toHaveAttribute('data-project-hash', initialProjectHash);
  await expect(surfaces.locator('[data-visual-id="voxel-surface-texture-readout"]'))
    .toContainText('2 × 1 px');
  await expect(surfaces).toContainText('repeat · texture/voxel/studio-surface · 1 assignments');
  await expect.poll(() => rendererHash(viewport)).not.toBe(initialRendererHash);
  const repeatRendererHash = await rendererHash(viewport);
  const repeatPixels = await canvas.screenshot();
  expect(repeatPixels.equals(initialPixels)).toBe(false);

  await surfaces.getByRole('button', { name: /material\/voxel\/studio-surface/ }).click();
  const repeatProjectHash = await requiredAttribute(shell, 'data-project-hash');
  await surfaces.getByLabel('Mapping').selectOption('atlas');
  const atlas = surfaces.locator('[data-visual-id="voxel-atlas-editor"]');
  await expect(atlas).toBeVisible();
  const minimum = atlas.locator('.axis-row.two').nth(0).locator('input');
  const extent = atlas.locator('.axis-row.two').nth(1).locator('input');
  const padding = atlas.locator('.axis-row.four').locator('input');
  await minimum.nth(0).fill('2');
  await minimum.nth(1).fill('0');
  await extent.nth(0).fill('1');
  await extent.nth(1).fill('1');
  for (let index = 0; index < 4; index += 1) await padding.nth(index).fill('0');
  await surfaces.locator('[data-action="apply-voxel-surface"]').click();
  const error = page.locator('[data-visual-id="studio-error-state"]');
  await expect(error).toContainText(/surface|atlas|project\.rejected/u);
  await expect(shell).toHaveAttribute('data-project-hash', repeatProjectHash);
  expect(await rendererHash(viewport)).toBe(repeatRendererHash);

  await minimum.nth(0).fill('0');
  await extent.nth(0).fill('2');
  await surfaces.locator('[data-action="apply-voxel-surface"]').click();
  await expect(error).toHaveCount(0);
  await expect(shell).not.toHaveAttribute('data-project-hash', repeatProjectHash);
  await expect(surfaces).toContainText('atlas · texture/voxel/studio-surface · 1 assignments');
  await expect(atlas).toContainText('half-texel inset');
  await expect(atlas).toContainText('0.25000, 0.50000 → 0.75000, 0.50000');
  await expect.poll(() => rendererHash(viewport)).not.toBe(repeatRendererHash);
  const atlasProjectHash = await requiredAttribute(shell, 'data-project-hash');
  const atlasRendererHash = await rendererHash(viewport);

  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', atlasProjectHash);
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
  await expect(viewport).toHaveAttribute('data-authored-frame-hash', atlasRendererHash);
  await page.getByRole('button', { name: 'Voxel', exact: true }).click();
  await editor.getByRole('button', { name: 'surfaces', exact: true }).click();
  await expect(surfaces).toContainText('atlas · texture/voxel/studio-surface · 1 assignments');
  await surfaces.getByRole('button', { name: /material\/voxel\/studio-surface/ }).click();
  await expect(atlas).toBeVisible();

  await page.setViewportSize({ width: 390, height: 844 });
  await expect(surfaces).toBeVisible();
  await expect(surfaces.locator('[data-action="apply-voxel-surface"]')).toBeVisible();
  await expect(atlas).toBeVisible();
  expect((await surfaces.screenshot()).byteLength).toBeGreaterThan(0);
  expect((await readFile(projectPath)).byteLength).toBeGreaterThan(0);
});

async function scrubAndWait(
  frame: Locator,
  viewport: Locator,
  playback: Locator,
  value: number,
): Promise<number> {
  const before = await rendererHash(viewport);
  const started = performance.now();
  await frame.evaluate((element, nextValue) => {
    const input = element as HTMLInputElement;
    input.value = String(nextValue);
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
  }, value);
  await expect(playback).toContainText(`frame ${String(value)}`);
  await expect.poll(() => rendererHash(viewport), { timeout: 30_000 }).not.toBe(before);
  return Number((performance.now() - started).toFixed(3));
}

async function rendererHash(viewport: Locator): Promise<string> {
  return requiredAttribute(viewport, 'data-authored-frame-hash');
}

async function requiredAttribute(locator: Locator, name: string): Promise<string> {
  const value = await locator.getAttribute(name);
  if (value === null || value.length === 0) throw new Error(`${name} is unavailable`);
  return value;
}

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`${name} is required`);
  return value;
}

interface RuntimeEvidenceReport {
  readonly runtime: {
    readonly engineRevision: string;
    readonly behavior: {
      readonly onceEnded: boolean;
      readonly repeatWrappedToFirstFrame: boolean;
      readonly missingAssetRejected: boolean;
      readonly corruptAssetRejected: boolean;
      readonly postureRoundTripMatched: boolean;
      readonly projectReopenMatched: boolean;
      readonly collisionKind: string;
      readonly collisionStayedStableDuringPlayback: boolean;
    };
  };
}
