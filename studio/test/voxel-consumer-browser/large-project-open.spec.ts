import { expect, test, type Page } from '@playwright/test';

const projectRoot = requiredEnvironment('RUSTY_STUDIO_PROJECT_ROOT');
const largeProjectFile = requiredEnvironment('RUSTY_STUDIO_LARGE_PROJECT_FILE');

test.describe.serial('large admitted voxel-object project', () => {
  test('opens from the startup URL through the complete Studio host and browser path', async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await page.goto(
      `/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(largeProjectFile)}`,
    );

    await expectProjectOpen(page);
    await expect(page.locator('.entity-row[data-entity-id="1"]')).toContainText('retro-character');
    await expect(page.locator('rusty-studio-viewport')).toHaveAttribute(
      'data-retained-ops',
      /^[1-9][0-9]*$/,
    );
  });

  test('opens from the visible project controls without poisoning the adapter', async ({ page }) => {
    test.setTimeout(180_000);
    await page.goto('/');

    await page.getByRole('textbox', { name: 'External project root' }).fill(projectRoot);
    await page.getByRole('textbox', { name: 'Project file' }).fill(largeProjectFile);
    await page.getByRole('button', { name: /^(?:Connect & Open|Open)$/u }).click();

    await expectProjectOpen(page, true);
    await expect(page.locator('.entity-row[data-entity-id="1"]')).toContainText('retro-character');

    await page.getByRole('button', { name: 'File', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Close Project' })).toBeEnabled();
  });
});

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
