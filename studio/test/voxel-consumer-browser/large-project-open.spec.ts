import { expect, test, type Page } from '@playwright/test';
import { STUDIO_ADAPTER_PROTOCOL_VERSION } from '@rusty-engine/studio-adapter-client';

const projectRoot = requiredEnvironment('RUSTY_STUDIO_PROJECT_ROOT');
const largeProjectFile = requiredEnvironment('RUSTY_STUDIO_LARGE_PROJECT_FILE');

test.describe.serial('large admitted voxel-object project', () => {
  test('opens from the startup URL through the complete Studio host and browser path', async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await page.goto('/');
    const dataPlane = await measureBrowserControlParse(page);
    await page.goto(
      `/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(largeProjectFile)}`,
    );

    await expectProjectOpen(page);
    await expect(page.locator('.entity-row[data-entity-id="1"]')).toContainText('retro-character');
    await expect(page.locator('rusty-studio-viewport')).toHaveAttribute(
      'data-retained-ops',
      /^[1-9][0-9]*$/,
    );
    const resourceTiming = await page.evaluate(() =>
      performance.getEntriesByType('resource')
        .filter((entry) => entry.name.includes('/api/studio-render-resource')
          && (entry.name.includes('.rmesh') || entry.name.includes('%2Ermesh')))
        .map((entry) => {
          const timing = entry as PerformanceResourceTiming;
          return {
            durationMilliseconds: timing.duration,
            encodedBodyBytes: timing.encodedBodySize,
          };
        }));
    expect(dataPlane.controlBytes).toBeLessThan(64 * 1024);
    const expectedResourceBytes = Number(
      process.env.RUSTY_STUDIO_EXPECTED_LARGE_RESOURCE_BYTES,
    );
    expect(Number.isSafeInteger(expectedResourceBytes)).toBe(true);
    expect(expectedResourceBytes).toBeGreaterThan(0);
    expect(dataPlane.resourceBytes).toBe(expectedResourceBytes);
    expect(resourceTiming.length).toBeGreaterThan(0);
    expect(resourceTiming.reduce((sum, entry) => sum + entry.encodedBodyBytes, 0))
      .toBe(dataPlane.resourceBytes);
    process.stdout.write(`${JSON.stringify({
      kind: 'studioVoxelMeshDataPlaneBrowserEvidence',
      projectFile: largeProjectFile,
      controlBytes: dataPlane.controlBytes,
      resourceBytes: dataPlane.resourceBytes,
      adapterRoundTripMilliseconds: dataPlane.adapterRoundTripMilliseconds,
      browserJsonParseMilliseconds: dataPlane.browserJsonParseMilliseconds,
      resourceTiming,
    })}\n`);
  });

  test('opens from the visible project controls', async ({ page }) => {
    test.setTimeout(180_000);
    await page.goto('/');

    await page.getByRole('textbox', { name: 'External project root' }).fill(projectRoot);
    await page.getByRole('textbox', { name: 'Project file' }).fill(largeProjectFile);
    await page.getByRole('button', { name: /^(?:Connect & Open|Open)$/u }).click();

    await expectProjectOpen(page, true);
    await expect(page.locator('.entity-row[data-entity-id="1"]')).toContainText('retro-character');
  });
});

async function measureBrowserControlParse(page: Page): Promise<{
  readonly controlBytes: number;
  readonly resourceBytes: number;
  readonly adapterRoundTripMilliseconds: number;
  readonly browserJsonParseMilliseconds: number;
}> {
  return page.evaluate(async ({ root, projectFile, protocolVersion }) => {
    const started = performance.now();
    const response = await fetch('/api/studio-adapter', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        type: 'openProject',
        protocolVersion,
        requestId: 'high-fidelity-browser-measurement',
        root,
        projectFile,
      }),
    });
    const text = await response.text();
    const received = performance.now();
    if (!response.ok) throw new Error(`measurement open failed with HTTP ${String(response.status)}`);
    const parseStarted = performance.now();
    const decoded = JSON.parse(text) as {
      readonly type?: string;
      readonly project?: {
        readonly meshResources?: readonly { readonly byteLength?: number }[];
      };
    };
    const parsed = performance.now();
    if (decoded.type !== 'projectOpened') throw new Error('measurement open was not accepted');
    return {
      controlBytes: new TextEncoder().encode(text).byteLength,
      resourceBytes: (decoded.project?.meshResources ?? [])
        .reduce((sum, resource) => sum + (resource.byteLength ?? 0), 0),
      adapterRoundTripMilliseconds: Number((received - started).toFixed(3)),
      browserJsonParseMilliseconds: Number((parsed - parseStarted).toFixed(3)),
    };
  }, {
    root: projectRoot,
    projectFile: largeProjectFile,
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
  });
}

async function expectProjectOpen(page: Page, waitForOpening = false) {
  const shell = page.locator('[data-visual-id="studio-shell"]');
  if (waitForOpening) {
    await expect(shell).toHaveAttribute('data-studio-operation', 'opening');
  }
  await expect(shell).toHaveAttribute('data-studio-operation', 'idle', { timeout: 120_000 });
  const projectHash = await shell.getAttribute('data-project-hash');
  if (projectHash === null || projectHash.length === 0) {
    const error = page.locator('[data-visual-id="studio-error-state"]');
    const diagnostic = await error.count() === 0 ? null : await error.textContent();
    throw new Error(diagnostic?.replace(/\s+/gu, ' ').trim() || 'Studio did not open the project');
  }
  return shell;
}

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`${name} is required`);
  return value;
}
