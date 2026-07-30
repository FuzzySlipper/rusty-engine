import { expect, test, type Locator, type Page, type Request } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const projectRoot = requiredEnvironment('RUSTY_STUDIO_PROJECT_ROOT');
const evidenceFile = requiredEnvironment('RUSTY_STUDIO_ENTITY_INSPECTOR_EVIDENCE');
const projectFile = 'content/projects/loading-bay.project.json';
const weaponOwnerEntityId = authoredEntityId(
  JSON.parse(readFileSync(join(projectRoot, projectFile), 'utf8')) as unknown,
  'weapon-definition-arc-pistol',
);
const weaponComponentTypeId = 'rusty-engine-demo.loading-bay.weapon';
const weaponContractId = 'rusty-engine-demo.loading-bay.weapon-authoring';
const unknownComponentTypeId = 'test.uninstalled-component';
const unknownContractId = 'test.uninstalled-component-authoring';
const projectOperationTimeout = 30_000;

test('unknown identity fallback stays visible and read-only in the downstream composition', async ({ page }) => {
  const failures = collectBrowserFailures(page);
  const observedRequests: string[] = [];
  await page.route('**/api/studio-adapter', async (route) => {
    const requestType = requestTypeOf(route.request());
    if (requestType !== null) observedRequests.push(requestType);
    const response = await route.fetch();
    const decoded = await response.json() as Record<string, unknown>;
    if (decoded['type'] === 'described') {
      const adapter = requiredRecord(decoded['adapter'], '$.adapter');
      const contracts = requiredArray(adapter['entityInspectorContracts'], '$.adapter.entityInspectorContracts');
      adapter['entityInspectorContracts'] = [...contracts, {
        contractId: unknownContractId,
        contractVersion: 1,
      }];
    }
    if (decoded['type'] === 'projectOpened' || decoded['type'] === 'projectRead') {
      const project = requiredRecord(decoded['project'], '$.project');
      const references = requiredArray(project['entityComponents'], '$.project.entityComponents');
      project['entityComponents'] = [...references, {
        ownerEntityId: weaponOwnerEntityId,
        componentTypeId: unknownComponentTypeId,
        inspectorContract: {
          contractId: unknownContractId,
          contractVersion: 1,
        },
      }];
    }
    await route.fulfill({ response, json: decoded });
  });

  await openLoadingBay(page);
  await selectEntityInspector(page, weaponOwnerEntityId);

  const identities = page.locator('[data-visual-id="entity-component-identities"]');
  await expect(identities).toContainText(unknownComponentTypeId);
  await expect(identities).toContainText(`${unknownContractId} v1`);
  await expect(identities).toContainText('identity only · read-only');
  await expect(page.locator(`[data-component-type-id="${unknownComponentTypeId}"]`)).toHaveCount(0);
  expect(observedRequests).not.toContain('readUninstalledComponent');
  expect(observedRequests).not.toContain('invokeExtension');
  await page.unrouteAll({ behavior: 'wait' });
  expect(failures).toEqual([]);
});

test('a real Loading Bay Weapon mutation settles through canonical reread', async ({ page }) => {
  const failures = collectBrowserFailures(page);
  const observedRequests: string[] = [];
  page.on('request', (request) => {
    const requestType = requestTypeOf(request);
    if (requestType !== null) observedRequests.push(requestType);
  });

  await openLoadingBay(page);
  const shell = page.locator('[data-visual-id="studio-shell"]');
  await selectEntityInspector(page, weaponOwnerEntityId);

  const identities = page.locator('[data-visual-id="entity-component-identities"]');
  await expect(identities).toContainText(weaponComponentTypeId);
  await expect(identities).toContainText(`${weaponContractId} v1 · panel available`);
  const component = page.locator('[data-visual-id="loading-bay-weapon-component"]');
  await expect(component).toContainText('Loading Bay Weapon');
  const panel = component.locator('[data-visual-id="loading-bay-weapon-inspector"]');
  await expect(panel).toContainText('Weapon');

  const damage = panel.locator('[data-visual-id="weapon-damage"]');
  await expect(damage).toBeEnabled();
  await expect(damage).toHaveValue(/^[0-9]+$/);
  const damageBefore = Number(await damage.inputValue());
  const damageAfter = damageBefore + 1;
  const hashBefore = await requiredAttribute(shell, 'data-project-hash');

  await damage.fill(String(damageAfter));
  const save = panel.locator('[data-visual-id="weapon-save"]');
  await expect(save).toBeEnabled();
  await save.click();
  await expect.poll(
    () => requiredAttribute(shell, 'data-project-hash'),
    { timeout: projectOperationTimeout },
  ).not.toBe(hashBefore);
  const hashAfter = await requiredAttribute(shell, 'data-project-hash');
  await expect(damage).toHaveValue(String(damageAfter));

  await expect.poll(() => observedRequests.filter((type) => type === 'readProject').length)
    .toBeGreaterThanOrEqual(1);
  const replaceIndex = observedRequests.lastIndexOf('replaceLoadingBayWeapon');
  const rereadIndex = observedRequests.lastIndexOf('readProject');
  expect(replaceIndex).toBeGreaterThanOrEqual(0);
  expect(rereadIndex).toBeGreaterThan(replaceIndex);

  await page.reload();
  await expect(shell).toHaveAttribute('data-project-hash', hashAfter, {
    timeout: projectOperationTimeout,
  });
  await selectEntityInspector(page, weaponOwnerEntityId);
  await expect(page.locator('[data-visual-id="loading-bay-weapon-inspector"]')
    .locator('[data-visual-id="weapon-damage"]')).toHaveValue(String(damageAfter));

  const durableProject = JSON.parse(
    await readFile(join(projectRoot, projectFile), 'utf8'),
  ) as unknown;
  expect(durableWeaponDamage(durableProject, 'weapon/arc-pistol')).toBe(damageAfter);
  await writeFile(evidenceFile, `${JSON.stringify({
    kind: 'studioEntityInspectorConsumerEvidence',
    ownerEntityId: weaponOwnerEntityId,
    itemDefinitionId: 'weapon/arc-pistol',
    damageBefore,
    damageAfter,
    hashBefore,
    hashAfter,
    canonicalRereadObserved: true,
    pageReloadMatched: true,
  })}\n`, 'utf8');
  expect(failures).toEqual([]);
});

test('a fresh adapter process preserves the Loading Bay Weapon mutation', async ({ page }) => {
  const failures = collectBrowserFailures(page);
  const evidence = JSON.parse(await readFile(evidenceFile, 'utf8')) as ConsumerEvidence;
  expect(evidence.kind).toBe('studioEntityInspectorConsumerEvidence');

  await openLoadingBay(page);
  const shell = page.locator('[data-visual-id="studio-shell"]');
  await expect(shell).toHaveAttribute('data-project-hash', evidence.hashAfter, {
    timeout: projectOperationTimeout,
  });
  await selectEntityInspector(page, evidence.ownerEntityId);
  const panel = page.locator('[data-visual-id="loading-bay-weapon-inspector"]');
  await expect(panel.locator('[data-visual-id="weapon-damage"]'))
    .toHaveValue(String(evidence.damageAfter));
  expect(evidence.hashAfter).not.toBe(evidence.hashBefore);
  expect(evidence.damageAfter).toBe(evidence.damageBefore + 1);
  expect(failures).toEqual([]);

  process.stdout.write(`${JSON.stringify({
    ...evidence,
    freshAdapterProcessMatched: true,
    browserFailures: failures.length,
  })}\n`);
});

async function openLoadingBay(page: Page): Promise<void> {
  await page.goto(`/?root=${encodeURIComponent(projectRoot)}&project=${encodeURIComponent(projectFile)}`);
  const shell = page.locator('[data-visual-id="studio-shell"]');
  await expect(shell).toHaveAttribute('data-project-hash', /.+/, {
    timeout: projectOperationTimeout,
  });
  await expect(page.locator('rusty-studio-viewport')).toHaveAttribute(
    'data-renderer-status',
    'ready',
    { timeout: projectOperationTimeout },
  );
  await expect(page.locator(`.entity-row[data-entity-id="${String(weaponOwnerEntityId)}"]`))
    .toBeVisible({ timeout: projectOperationTimeout });
}

async function selectEntityInspector(page: Page, ownerEntityId: number): Promise<void> {
  await page.locator(`.entity-row[data-entity-id="${String(ownerEntityId)}"]`).click();
  await page.getByRole('button', { name: 'Entity', exact: true }).click();
  await expect(page.locator('.inspector-panel')).toContainText(`Entity #${String(ownerEntityId)}`);
}

function collectBrowserFailures(page: Page): string[] {
  const failures: string[] = [];
  page.on('pageerror', (error) => failures.push(`pageerror: ${error.message}`));
  page.on('console', (message) => {
    if (message.type() === 'error') failures.push(`console: ${message.text()}`);
  });
  return failures;
}

function requestTypeOf(request: Request): string | null {
  if (!request.url().endsWith('/api/studio-adapter') || request.method() !== 'POST') return null;
  const body = request.postData();
  if (body === null) return null;
  try {
    const decoded = JSON.parse(body) as Record<string, unknown>;
    return typeof decoded['type'] === 'string' ? decoded['type'] : null;
  } catch {
    return null;
  }
}

function requiredRecord(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${path} must be an object`);
  }
  return value as Record<string, unknown>;
}

function requiredArray(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) throw new Error(`${path} must be an array`);
  return value;
}

function durableWeaponDamage(project: unknown, itemDefinitionId: string): number {
  const root = requiredRecord(project, '$');
  const items = requiredArray(root['itemDefinitions'], '$.itemDefinitions');
  const item = items
    .map((candidate, index) => requiredRecord(candidate, `$.itemDefinitions[${String(index)}]`))
    .find((candidate) => candidate['id'] === itemDefinitionId);
  if (item === undefined) throw new Error(`missing durable item ${itemDefinitionId}`);
  const kind = requiredRecord(item['kind'], '$.itemDefinitions[].kind');
  const damage = kind['damage'];
  if (!Number.isSafeInteger(damage)) throw new Error('durable weapon damage must be a safe integer');
  return damage as number;
}

function authoredEntityId(project: unknown, entityName: string): number {
  const root = requiredRecord(project, '$');
  const scenes = requiredArray(root['scenes'], '$.scenes');
  const matches = scenes.flatMap((candidate, sceneIndex) => {
    const scene = requiredRecord(candidate, `$.scenes[${String(sceneIndex)}]`);
    const entities = requiredArray(scene['entities'], `$.scenes[${String(sceneIndex)}].entities`);
    return entities
      .map((entity, entityIndex) => requiredRecord(
        entity,
        `$.scenes[${String(sceneIndex)}].entities[${String(entityIndex)}]`,
      ))
      .filter((entity) => entity['name'] === entityName)
      .map((entity) => entity['id']);
  });
  if (matches.length !== 1 || !Number.isSafeInteger(matches[0])) {
    throw new Error(`expected exactly one authored entity named ${entityName}`);
  }
  return matches[0] as number;
}

async function requiredAttribute(
  locator: Locator,
  name: string,
): Promise<string> {
  const value = await locator.getAttribute(name);
  if (value === null || value.length === 0) throw new Error(`${name} is missing`);
  return value;
}

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`${name} is required`);
  return value;
}

interface ConsumerEvidence {
  readonly kind: 'studioEntityInspectorConsumerEvidence';
  readonly ownerEntityId: number;
  readonly itemDefinitionId: string;
  readonly damageBefore: number;
  readonly damageAfter: number;
  readonly hashBefore: string;
  readonly hashAfter: string;
  readonly canonicalRereadObserved: true;
  readonly pageReloadMatched: true;
}
