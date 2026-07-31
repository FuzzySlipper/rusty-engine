import { expect, test, type Locator } from '@playwright/test';
import { performance } from 'node:perf_hooks';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';

const projectRoot = requiredEnvironment('RUSTY_STUDIO_PROJECT_ROOT');
const projectFile = requiredEnvironment('RUSTY_STUDIO_PROJECT_FILE');
const runtimeReportFile = requiredEnvironment('RUSTY_STUDIO_RUNTIME_REPORT');
const engineCommit = requiredEnvironment('RUSTY_STUDIO_ENGINE_COMMIT');

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
