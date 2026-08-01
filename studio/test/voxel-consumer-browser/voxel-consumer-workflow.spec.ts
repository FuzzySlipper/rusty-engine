import { expect, test, type Locator, type Page } from '@playwright/test';
import { createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

const projectRoot = requiredEnvironment('RUSTY_STUDIO_PROJECT_ROOT');
const projectFile = requiredEnvironment('RUSTY_STUDIO_PROJECT_FILE');
const runtimeReportFile = requiredEnvironment('RUSTY_STUDIO_RUNTIME_REPORT');
const engineCommit = requiredEnvironment('RUSTY_STUDIO_ENGINE_COMMIT');
const TEXTURE_PATH = 'content/textures/directional-atlas.png';
const TEXTURE_HASH = 'ac1a8a3685fe0b5b42c585f4f5cf8e246721a09497644eacddf36372a377fd99';

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
  // This single installed-product path intentionally spans two material
  // publications, reload, mobile layout, close, and reopen. Keep the whole
  // proof bounded without truncating its final lifecycle assertions on CI.
  test.setTimeout(180_000);
  const projectPath = join(projectRoot, projectFile);
  const textureBytes = await readFile(join(projectRoot, TEXTURE_PATH));
  expect(sha256(textureBytes)).toBe(TEXTURE_HASH);

  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(projectFile)}`);
  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  const canvas = page.getByLabel('Shared Rusty renderer viewport');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/);
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
  const initialProjectHash = await requiredAttribute(shell, 'data-project-hash');
  const initialRendererHash = await rendererHash(viewport);
  const initialPixels = await canvas.screenshot();
  const initialPixelHash = sha256(initialPixels);

  await page.getByRole('button', { name: 'Voxel', exact: true }).click();
  const editor = page.locator('[data-visual-id="studio-voxel-editor"]');
  await editor.getByRole('button', { name: 'surfaces', exact: true }).click();
  const surfaces = editor.locator('[data-visual-id="voxel-surface-authoring"]');
  await expect(surfaces).toContainText('Runtime voxel surfaces');
  await surfaces.getByLabel('PNG source').fill(TEXTURE_PATH);
  await surfaces.getByLabel('Filter').selectOption('linear');
  const tileScale = surfaces.getByLabel('Tile scale in cells').locator('input');
  const tileOrigin = surfaces.getByLabel('Tile origin in cells').locator('input');
  await tileScale.nth(0).fill('0.5');
  await tileScale.nth(1).fill('2');
  await tileOrigin.nth(0).fill('0.25');
  await tileOrigin.nth(1).fill('-0.5');
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
    .toContainText('16 × 8 px');
  await expect(viewport).toHaveAttribute('data-texture-resources', '1');
  await expect(surfaces).toContainText('repeat · texture/voxel/studio-surface · 1 assignments');
  await expect.poll(() => rendererHash(viewport)).not.toBe(initialRendererHash);
  const repeatRendererHash = await rendererHash(viewport);
  const repeatPixels = await canvas.screenshot();
  const repeatPixelHash = sha256(repeatPixels);
  expect(repeatPixels.equals(initialPixels)).toBe(false);

  await surfaces.getByRole('button', { name: /material\/voxel\/studio-surface/ }).click();
  const repeatProjectHash = await requiredAttribute(shell, 'data-project-hash');
  await surfaces.getByLabel('Mapping').selectOption('atlas');
  const atlas = surfaces.locator('[data-visual-id="voxel-atlas-editor"]');
  await expect(atlas).toBeVisible();
  const minimum = atlas.locator('.axis-row.two').nth(0).locator('input');
  const extent = atlas.locator('.axis-row.two').nth(1).locator('input');
  const padding = atlas.locator('.axis-row.four').locator('input');
  await minimum.nth(0).fill('15');
  await minimum.nth(1).fill('7');
  await extent.nth(0).fill('6');
  await extent.nth(1).fill('6');
  for (let index = 0; index < 4; index += 1) await padding.nth(index).fill('1');
  await surfaces.locator('[data-action="apply-voxel-surface"]').click();
  const error = page.locator('[data-visual-id="studio-error-state"]');
  await expect(error).toContainText(/surface|atlas|project\.rejected/u);
  await expect(shell).toHaveAttribute('data-project-hash', repeatProjectHash);
  expect(await rendererHash(viewport)).toBe(repeatRendererHash);

  await minimum.nth(0).fill('1');
  await minimum.nth(1).fill('1');
  await surfaces.locator('[data-action="apply-voxel-surface"]').click();
  await expect(error).toHaveCount(0);
  await expect(shell).not.toHaveAttribute('data-project-hash', repeatProjectHash);
  await expect(surfaces).toContainText('atlas · texture/voxel/studio-surface · 1 assignments');
  await expect(atlas).toContainText('half-texel inset');
  await expect(atlas).toContainText('0.09375, 0.18750 → 0.40625, 0.81250');
  await expect(viewport).toHaveAttribute('data-texture-resources', '1');
  await expect.poll(() => rendererHash(viewport)).not.toBe(repeatRendererHash);
  const atlasProjectHash = await requiredAttribute(shell, 'data-project-hash');
  const atlasRendererHash = await rendererHash(viewport);
  const atlasPixelHash = sha256(await canvas.screenshot());
  expect(atlasPixelHash).not.toBe(repeatPixelHash);

  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', atlasProjectHash);
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
  await expect(viewport).toHaveAttribute('data-authored-frame-hash', atlasRendererHash);
  await expect(viewport).toHaveAttribute('data-texture-resources', '1');
  const reopenedPixelHash = sha256(await canvas.screenshot());
  expect(reopenedPixelHash).toBe(atlasPixelHash);
  await page.getByRole('button', { name: 'Voxel', exact: true }).click();
  await editor.getByRole('button', { name: 'surfaces', exact: true }).click();
  await expect(surfaces).toContainText('atlas · texture/voxel/studio-surface · 1 assignments');
  await surfaces.getByRole('button', { name: /material\/voxel\/studio-surface/ }).click();
  await expect(atlas).toBeVisible();
  await expect(atlas).toContainText('0.09375, 0.18750 → 0.40625, 0.81250');

  await page.setViewportSize({ width: 390, height: 844 });
  await expect(surfaces).toBeVisible();
  await expect(surfaces.locator('[data-action="apply-voxel-surface"]')).toBeVisible();
  await expect(atlas).toBeVisible();
  const mobileAuthoringPixelHash = sha256(await surfaces.screenshot());
  expect(mobileAuthoringPixelHash).not.toBe(initialPixelHash);
  await closeProjectThroughFileMenu(page);
  await expect(viewport).toHaveAttribute('data-texture-resources', '0');
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(projectFile)}`);
  await expect(shell).toHaveAttribute('data-project-hash', atlasProjectHash);
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
  await expect(viewport).toHaveAttribute('data-texture-resources', '1');
  await closeProjectThroughFileMenu(page);
  await expect(viewport).toHaveAttribute('data-texture-resources', '0');
  expect((await readFile(projectPath)).byteLength).toBeGreaterThan(0);

  process.stdout.write(`${JSON.stringify({
    kind: 'studioVoxelSurfaceBrowserEvidence',
    engineRevision: engineCommit,
    texturePath: TEXTURE_PATH,
    textureSha256: TEXTURE_HASH,
    textureDimensions: [16, 8],
    repeatRendererHash,
    atlasRendererHash,
    initialPixelHash,
    repeatPixelHash,
    atlasPixelHash,
    reopenedPixelHash,
    mobileAuthoringPixelHash,
    retainedTextureResources: 1,
    closeReopenCloseResourceCounts: [0, 1, 0],
    atlasSafeBounds: [0.09375, 0.1875, 0.40625, 0.8125],
    tileScaleCells: [0.5, 2],
    tileOriginCells: [0.25, -0.5],
    reopenMatched: true,
  })}\n`);
});

test('fresh Studio host reopens the persisted atlas surface and disposes it on close', async ({ page }) => {
  test.skip(process.env['RUSTY_STUDIO_EXPECT_PREAUTHORED_SURFACE'] !== '1');
  test.setTimeout(60_000);
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(projectFile)}`);
  const shell = page.locator('[data-visual-id="studio-shell"]');
  const viewport = page.locator('rusty-studio-viewport');
  const canvas = page.getByLabel('Shared Rusty renderer viewport');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/);
  await expect(viewport).toHaveAttribute('data-renderer-status', 'ready');
  await expect(viewport).toHaveAttribute('data-texture-resources', '1');
  await page.getByRole('button', { name: 'Voxel', exact: true }).click();
  const editor = page.locator('[data-visual-id="studio-voxel-editor"]');
  await editor.getByRole('button', { name: 'surfaces', exact: true }).click();
  const surfaces = editor.locator('[data-visual-id="voxel-surface-authoring"]');
  await expect(surfaces).toContainText('atlas · texture/voxel/studio-surface · 1 assignments');
  await surfaces.getByRole('button', { name: /material\/voxel\/studio-surface/ }).click();
  await expect(surfaces.locator('[data-visual-id="voxel-surface-texture-readout"]'))
    .toContainText('16 × 8 px');
  const freshHostPixelHash = sha256(await canvas.screenshot());
  await closeProjectThroughFileMenu(page);
  await expect(viewport).toHaveAttribute('data-texture-resources', '0');
  process.stdout.write(`${JSON.stringify({
    kind: 'studioVoxelSurfaceFreshHostEvidence',
    engineRevision: engineCommit,
    textureSha256: TEXTURE_HASH,
    freshHostPixelHash,
    retainedTextureResources: 1,
    retainedTextureResourcesAfterClose: 0,
  })}\n`);
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

async function closeProjectThroughFileMenu(page: Page): Promise<void> {
  const closeProject = page.getByRole('button', { name: 'Close Project' });
  if (!(await closeProject.isVisible())) {
    await page.getByRole('button', { name: 'File', exact: true }).click();
  }
  await expect(closeProject).toBeVisible({ timeout: 30_000 });
  await closeProject.click();
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

function sha256(bytes: Uint8Array): string {
  return createHash('sha256').update(bytes).digest('hex');
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
