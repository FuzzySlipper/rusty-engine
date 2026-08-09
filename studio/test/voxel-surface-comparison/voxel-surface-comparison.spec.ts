import { expect, test } from '@playwright/test';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { basename, join } from 'node:path';

interface ReportEntry {
  readonly model: string;
  readonly frameId: string;
  readonly mode: string;
}

interface Report {
  readonly entries: readonly ReportEntry[];
}

const reportPath = requiredEnvironment('RUSTY_SURFACE_REPORT');
const evidenceDirectory = requiredEnvironment('RUSTY_SURFACE_EVIDENCE_DIR');
const report = JSON.parse(await readFile(reportPath, 'utf8')) as Report;

test('real Chromium renders every comparison entry through Studio viewport submission', async ({ page }) => {
  test.setTimeout(Math.max(120_000, report.entries.length * 30_000));
  await mkdir(evidenceDirectory, { recursive: true });
  await page.goto('/');
  const comparison = page.locator('#comparison');
  await expect(comparison).toHaveAttribute('data-status', 'ready', { timeout: 120_000 });
  const browserMetrics: unknown[] = [];
  for (const [index, entry] of report.entries.entries()) {
    await page.evaluate(async (entryIndex) => {
      await window.renderVoxelSurfaceComparison(entryIndex);
    }, index);
    await expect(comparison).toHaveAttribute('data-status', 'ready', { timeout: 120_000 });
    const file = `${String(index).padStart(2, '0')}-${safe(entry.model)}-${safe(entry.frameId)}-${safe(entry.mode)}.png`;
    await page.locator('.card').screenshot({ path: join(evidenceDirectory, file) });
    browserMetrics.push(await comparison.evaluate((element, identity) => ({
      ...identity,
      replacementMilliseconds: element.getAttribute('data-replacement-milliseconds'),
      retainedResources: element.getAttribute('data-retained-resources'),
    }), { index, model: entry.model, frameId: entry.frameId, mode: entry.mode }));
  }
  await writeFile(
    join(evidenceDirectory, 'browser-metrics.json'),
    `${JSON.stringify({ schemaVersion: 1, entries: browserMetrics }, null, 2)}\n`,
  );
  await page.screenshot({ path: join(evidenceDirectory, 'last-entry-page.png'), fullPage: true });
});

function safe(value: string): string {
  return basename(value).replaceAll(/[^a-zA-Z0-9-]+/gu, '-').replaceAll(/^-|-$/gu, '');
}

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`${name} is required`);
  return value;
}
