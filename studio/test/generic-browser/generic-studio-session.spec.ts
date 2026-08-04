import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const demoRoot = requiredEnvironment('RUSTY_STUDIO_GENERIC_DEMO_ROOT');
const voxelRoot = requiredEnvironment('RUSTY_STUDIO_GENERIC_VOXEL_ROOT');
const loadingBayProject = 'content/projects/loading-bay.project.json';
const retroProject = 'content/projects/retro-character-high-fidelity.project.json';
const voxelLabProject = 'content/projects/voxel-lab.project.json';
const openTimeout = 90_000;
let invalidRoot: string;
let failedStartRoot: string;

test.beforeAll(async () => {
  invalidRoot = await mkdtemp(join(tmpdir(), 'rusty-studio-generic-invalid-'));
  failedStartRoot = await mkdtemp(join(tmpdir(), 'rusty-studio-generic-failed-start-'));
  await mkdir(join(failedStartRoot, 'content', 'projects'), { recursive: true });
  await writeFile(join(failedStartRoot, '.rusty-studio.json'), JSON.stringify({
    schemaVersion: 1,
    adapter: { command: ['./missing-adapter'], cwd: '.' },
  }));
});

test.afterAll(async () => {
  await rm(invalidRoot, { recursive: true, force: true });
  await rm(failedStartRoot, { recursive: true, force: true });
});

test('one generic Studio address discovers, switches, and rejects roots transactionally', async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto('/');
  const shell = page.locator('[data-visual-id="studio-shell"]');
  const identity = page.locator('[data-visual-id="studio-runtime-identity"]');

  await openManually(page, demoRoot, loadingBayProject);
  await expectProject(page, shell, identity, demoRoot, 'rusty-engine-demo.loading-bay');
  const loadingBayHash = await shell.getAttribute('data-project-hash');

  // Query startup uses the same store/session route as the manual controls.
  await page.goto(`/?root=${encodeURIComponent(voxelRoot)}&project=${encodeURIComponent(retroProject)}`);
  await expectProject(page, shell, identity, voxelRoot, 'rusty-engine-voxels.voxel-lab');

  await openManually(page, voxelRoot, voxelLabProject);
  await expectProject(page, shell, identity, voxelRoot, 'rusty-engine-voxels.voxel-lab');

  await openManually(page, demoRoot, loadingBayProject);
  await expectProject(page, shell, identity, demoRoot, 'rusty-engine-demo.loading-bay');
  await expect(shell).toHaveAttribute('data-project-hash', loadingBayHash ?? /.+/);

  await openManually(page, invalidRoot, 'content/projects/missing.project.json');
  await expect(page.getByRole('alert')).toContainText('studio_adapter_bootstrap_missing');
  await expect(identity).toHaveAttribute('data-active-project-root', demoRoot);
  await expect(shell).toHaveAttribute('data-project-hash', loadingBayHash ?? /.+/);

  await openManually(page, failedStartRoot, 'content/projects/missing.project.json');
  await expect(page.getByRole('alert')).toContainText('studio_adapter_bootstrap_command_not_file');
  await expect(identity).toHaveAttribute('data-active-project-root', demoRoot);
  await expect(shell).toHaveAttribute('data-project-hash', loadingBayHash ?? /.+/);
});

async function openManually(page: Page, root: string, projectFile: string): Promise<void> {
  const rootInput = page.getByLabel('External project root');
  const projectInput = page.getByLabel('Project file');
  await rootInput.fill(root);
  await projectInput.fill(projectFile);
  await page.getByRole('button', { name: /^(Connect & Open|Open)$/u }).click();
}

async function expectProject(
  page: Page,
  shell: Locator,
  identity: Locator,
  root: string,
  adapterId: string,
): Promise<void> {
  await expect(shell).toHaveAttribute('data-project-hash', /.+/u, { timeout: openTimeout });
  await expect(shell).toHaveAttribute('data-studio-operation', 'idle', { timeout: openTimeout });
  await expect(identity).toHaveAttribute('data-active-project-root', root, { timeout: openTimeout });
  await expect(identity).toHaveAttribute('data-active-project-file', /.+/u);
  await expect(identity).toHaveAttribute('data-runtime-mode', 'generic');
  await expect(identity).toContainText('generic interactive');
  await expect(identity).toContainText(adapterId);
  await expect(identity).toHaveAttribute('data-protocol-version', '14');
  await expect(page.getByRole('button', { name: 'Open', exact: true })).toBeEnabled();
  await expect(page.locator('.document-title')).not.toContainText('No project open');
}

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`${name} is required`);
  return value;
}
