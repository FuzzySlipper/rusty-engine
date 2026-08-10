import assert from 'node:assert/strict';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import {
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from 'node:fs/promises';
import { Agent, request } from 'node:http';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';

import {
  STUDIO_ADAPTER_OPERATIONS,
  STUDIO_ADAPTER_PROTOCOL_VERSION,
} from '../libs/adapter-client/src/index.js';

const STUDIO_ROOT = resolve(import.meta.dirname, '..');

interface LifecycleEvent {
  readonly event: 'started' | 'stopped';
  readonly adapterId: string;
  readonly pid: number;
}

interface RunningHost {
  readonly child: ChildProcessWithoutNullStreams;
  readonly port: number;
  readonly stderr: () => string;
}

class ProbeClient {
  readonly #agent = new Agent({ keepAlive: true, maxSockets: 1 });

  constructor(
    private readonly port: number,
    private readonly clientId: string,
  ) {}

  async get(path: string): Promise<Record<string, unknown>> {
    return this.#exchange('GET', path);
  }

  async post(path: string, body: Record<string, unknown>): Promise<Record<string, unknown>> {
    return this.#exchange('POST', path, body);
  }

  close(): void {
    this.#agent.destroy();
  }

  #exchange(
    method: 'GET' | 'POST',
    path: string,
    body?: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    const encoded = body === undefined ? undefined : JSON.stringify(body);
    return new Promise((resolvePromise, rejectPromise) => {
      const outgoing = request({
        agent: this.#agent,
        host: '127.0.0.1',
        port: this.port,
        method,
        path,
        headers: {
          accept: 'application/json',
          'x-rusty-studio-probe-client': this.clientId,
          ...(encoded === undefined ? {} : {
            'content-length': String(Buffer.byteLength(encoded)),
            'content-type': 'application/json',
          }),
        },
      }, (incoming) => {
        const chunks: Buffer[] = [];
        incoming.on('data', (chunk: Buffer) => chunks.push(chunk));
        incoming.once('error', rejectPromise);
        incoming.once('end', () => {
          const text = Buffer.concat(chunks).toString('utf8');
          if (incoming.statusCode === undefined || incoming.statusCode < 200 || incoming.statusCode >= 300) {
            rejectPromise(new Error(`Studio probe request failed ${String(incoming.statusCode)}: ${text}`));
            return;
          }
          try {
            resolvePromise(JSON.parse(text) as Record<string, unknown>);
          } catch (error) {
            rejectPromise(error);
          }
        });
      });
      outgoing.once('error', rejectPromise);
      if (encoded !== undefined) outgoing.write(encoded);
      outgoing.end();
    });
  }
}

test('shared clients redirect one process-wide project while isolated hosts remain independent', async () => {
  const root = await mkdtemp(join(tmpdir(), 'rusty-studio-concurrency-'));
  const lifecyclePath = join(root, 'adapter-lifecycle.jsonl');
  const staticRoot = join(root, 'static');
  const firstRoot = join(root, 'first-project');
  const secondRoot = join(root, 'second-project');
  const allHosts: RunningHost[] = [];
  const allClients: ProbeClient[] = [];
  try {
    await mkdir(staticRoot, { recursive: true });
    await writeFile(join(staticRoot, 'index.html'), '<!doctype html><title>fixture</title>');
    await Promise.all([
      writeFixtureRoot(firstRoot, 'fixture.first', 'first-project', lifecyclePath),
      writeFixtureRoot(secondRoot, 'fixture.second', 'second-project', lifecyclePath),
    ]);

    const shared = await startHost(staticRoot, join(root, 'shared-settings'));
    allHosts.push(shared);
    const sharedFirst = new ProbeClient(shared.port, 'shared-first');
    const sharedSecond = new ProbeClient(shared.port, 'shared-second');
    allClients.push(sharedFirst, sharedSecond);

    await openProject(sharedFirst, firstRoot, 'content/first.project.json');
    assert.equal((await sharedFirst.get('/api/studio-status'))['activeProjectRoot'], firstRoot);
    const firstPid = await adapterPid(lifecyclePath, 'fixture.first');

    await openProject(sharedSecond, secondRoot, 'content/second.project.json');
    const redirectedStatus = await sharedFirst.get('/api/studio-status');
    assert.equal(redirectedStatus['activeProjectRoot'], secondRoot);
    assert.equal(runningAdapterId(redirectedStatus), 'fixture.second');
    assert.equal(
      runningAdapterId(await describe(sharedFirst)),
      'fixture.second',
      'the first client now exchanges with the second client\'s adapter',
    );
    await waitForProcessExit(firstPid);
    const sharedAdapterPid = await adapterPid(lifecyclePath, 'fixture.second');
    const sharedMeasurement = {
      hostRssKiB: await residentMemoryKiB(shared.child.pid),
      adapterRssKiB: await residentMemoryKiB(sharedAdapterPid),
      liveHosts: 1,
      liveAdapters: 1,
    };

    await stopHost(shared);
    await waitForProcessExit(sharedAdapterPid);

    const isolatedFirst = await startHost(staticRoot, join(root, 'isolated-first-settings'));
    const isolatedSecond = await startHost(staticRoot, join(root, 'isolated-second-settings'));
    allHosts.push(isolatedFirst, isolatedSecond);
    const firstClient = new ProbeClient(isolatedFirst.port, 'isolated-first');
    const secondClient = new ProbeClient(isolatedSecond.port, 'isolated-second');
    allClients.push(firstClient, secondClient);
    await Promise.all([
      openProject(firstClient, firstRoot, 'content/first.project.json'),
      openProject(secondClient, secondRoot, 'content/second.project.json'),
    ]);

    assert.equal((await firstClient.get('/api/studio-status'))['activeProjectRoot'], firstRoot);
    assert.equal((await secondClient.get('/api/studio-status'))['activeProjectRoot'], secondRoot);
    assert.equal(runningAdapterId(await describe(firstClient)), 'fixture.first');
    assert.equal(runningAdapterId(await describe(secondClient)), 'fixture.second');

    const events = await lifecycleEvents(lifecyclePath);
    const liveFirstPid = latestStartedPid(events, 'fixture.first');
    const liveSecondPid = latestStartedPid(events, 'fixture.second');
    const isolatedMeasurement = {
      hostRssKiB: [
        await residentMemoryKiB(isolatedFirst.child.pid),
        await residentMemoryKiB(isolatedSecond.child.pid),
      ],
      adapterRssKiB: [
        await residentMemoryKiB(liveFirstPid),
        await residentMemoryKiB(liveSecondPid),
      ],
      liveHosts: 2,
      liveAdapters: 2,
    };

    await Promise.all([stopHost(isolatedFirst), stopHost(isolatedSecond)]);
    await Promise.all([waitForProcessExit(liveFirstPid), waitForProcessExit(liveSecondPid)]);
    process.stdout.write(`STUDIO_CONCURRENCY_PROBE ${JSON.stringify({
      shared: sharedMeasurement,
      isolated: isolatedMeasurement,
      cleanup: 'all adapter process groups exited',
    })}\n`);
  } finally {
    for (const client of allClients) client.close();
    await Promise.all(allHosts.map((host) => stopHost(host).catch(() => undefined)));
    await rm(root, { recursive: true, force: true });
  }
});

async function openProject(client: ProbeClient, root: string, projectFile: string): Promise<void> {
  const response = await client.post('/api/studio-session/open', { root, projectFile });
  assert.equal(response['type'], 'studioSessionOpened');
}

async function describe(client: ProbeClient): Promise<Record<string, unknown>> {
  return client.post('/api/studio-adapter', {
    type: 'describe',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'concurrency-probe-describe',
  });
}

function runningAdapterId(value: Record<string, unknown>): string | undefined {
  const adapter = value['adapter'] ?? value['runningAdapter'];
  return isRecord(adapter) && typeof adapter['adapterId'] === 'string'
    ? adapter['adapterId']
    : undefined;
}

async function startHost(staticRoot: string, settingsRoot: string): Promise<RunningHost> {
  const port = await freePort();
  const child = spawn(process.execPath, [
    '--import', 'tsx',
    'scripts/studio-host.ts',
    '--static-root', staticRoot,
    '--settings-root', settingsRoot,
    '--host', '127.0.0.1',
    '--port', String(port),
  ], { cwd: STUDIO_ROOT, stdio: ['pipe', 'pipe', 'pipe'] });
  let stderr = '';
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (chunk: string) => { stderr += chunk; });
  const running = { child, port, stderr: () => stderr };
  await waitForHealth(running);
  return running;
}

async function stopHost(host: RunningHost): Promise<void> {
  if (host.child.exitCode !== null || host.child.signalCode !== null) return;
  host.child.kill('SIGTERM');
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const result = await Promise.race([
    childResult(host.child),
    new Promise<never>((_resolve, rejectPromise) => {
      timeout = setTimeout(
        () => rejectPromise(new Error('Studio host did not stop within 5 seconds')),
        5_000,
      );
    }),
  ]).finally(() => { clearTimeout(timeout); });
  assert.ok(
    result.code === 0 || result.signal === 'SIGTERM',
    `Studio host failed during shutdown: ${host.stderr()}`,
  );
}

function childResult(child: ChildProcessWithoutNullStreams): Promise<{
  readonly code: number | null;
  readonly signal: NodeJS.Signals | null;
}> {
  if (child.exitCode !== null || child.signalCode !== null) {
    return Promise.resolve({ code: child.exitCode, signal: child.signalCode });
  }
  return new Promise((resolvePromise) => {
    child.once('exit', (code, signal) => resolvePromise({ code, signal }));
  });
}

async function waitForHealth(host: RunningHost): Promise<void> {
  const deadline = Date.now() + 10_000;
  let lastError: unknown;
  while (Date.now() < deadline) {
    if (host.child.exitCode !== null) {
      throw new Error(`Studio host exited before health: ${host.stderr()}`);
    }
    try {
      const response = await fetch(`http://127.0.0.1:${String(host.port)}/health`);
      if (response.ok) return;
    } catch (error) {
      lastError = error;
    }
    await new Promise<void>((resolvePromise) => { setTimeout(resolvePromise, 25); });
  }
  throw new Error(`Studio host did not become healthy: ${String(lastError)} ${host.stderr()}`);
}

async function freePort(): Promise<number> {
  const { createServer } = await import('node:http');
  const server = createServer();
  await new Promise<void>((resolvePromise) => server.listen(0, '127.0.0.1', resolvePromise));
  const address = server.address();
  if (address === null || typeof address === 'string') throw new Error('fixture port is unavailable');
  await new Promise<void>((resolvePromise, rejectPromise) => {
    server.close((error) => error === undefined ? resolvePromise() : rejectPromise(error));
  });
  return address.port;
}

async function residentMemoryKiB(pid: number | undefined): Promise<number> {
  assert.ok(pid !== undefined);
  const status = await readFile(`/proc/${String(pid)}/status`, 'utf8');
  const match = /^VmRSS:\s+(\d+)\s+kB$/mu.exec(status);
  assert.ok(match?.[1] !== undefined, `VmRSS missing for process ${String(pid)}`);
  return Number(match[1]);
}

async function adapterPid(path: string, adapterId: string): Promise<number> {
  const deadline = Date.now() + 2_000;
  while (Date.now() < deadline) {
    const events = await lifecycleEvents(path);
    const started = events.filter(
      (event) => event.event === 'started' && event.adapterId === adapterId,
    ).at(-1);
    if (started !== undefined) return started.pid;
    await new Promise<void>((resolvePromise) => { setTimeout(resolvePromise, 10); });
  }
  throw new Error(`Adapter ${adapterId} did not publish its pid`);
}

async function lifecycleEvents(path: string): Promise<readonly LifecycleEvent[]> {
  let text: string;
  try {
    text = await readFile(path, 'utf8');
  } catch (error) {
    if (isRecord(error) && error['code'] === 'ENOENT') return [];
    throw error;
  }
  return text.trim().length === 0
    ? []
    : text.trim().split('\n').map((line) => JSON.parse(line) as LifecycleEvent);
}

function latestStartedPid(events: readonly LifecycleEvent[], adapterId: string): number {
  const event = events.filter(
    (candidate) => candidate.event === 'started' && candidate.adapterId === adapterId,
  ).at(-1);
  assert.ok(event !== undefined, `missing lifecycle start for ${adapterId}`);
  return event.pid;
}

async function waitForProcessExit(pid: number): Promise<void> {
  const deadline = Date.now() + 5_000;
  while (Date.now() < deadline) {
    try {
      process.kill(pid, 0);
    } catch {
      return;
    }
    await new Promise<void>((resolvePromise) => { setTimeout(resolvePromise, 25); });
  }
  throw new Error(`process ${String(pid)} remained live after bounded cleanup`);
}

async function writeFixtureRoot(
  root: string,
  adapterId: string,
  projectId: string,
  lifecyclePath: string,
): Promise<void> {
  await mkdir(root, { recursive: true });
  const adapter = join(root, 'fixture-adapter.mjs');
  await writeFile(adapter, fixtureAdapterSource(adapterId, projectId, lifecyclePath));
  await chmod(adapter, 0o755);
  await writeFile(join(root, '.rusty-studio.json'), JSON.stringify({
    schemaVersion: 1,
    adapter: { command: ['./fixture-adapter.mjs'], cwd: '.' },
  }));
}

function fixtureAdapterSource(adapterId: string, projectId: string, lifecyclePath: string): string {
  return `#!/usr/bin/env node
import { appendFileSync } from 'node:fs';
import { createInterface } from 'node:readline';
const adapterId = ${JSON.stringify(adapterId)};
const projectId = ${JSON.stringify(projectId)};
const lifecyclePath = ${JSON.stringify(lifecyclePath)};
appendFileSync(lifecyclePath, JSON.stringify({ event: 'started', adapterId, pid: process.pid }) + '\\n');
process.once('exit', () => {
  appendFileSync(lifecyclePath, JSON.stringify({ event: 'stopped', adapterId, pid: process.pid }) + '\\n');
});
const lines = createInterface({ input: process.stdin });
lines.on('line', (line) => {
  const request = JSON.parse(line);
  const header = {
    protocolVersion: ${String(STUDIO_ADAPTER_PROTOCOL_VERSION)},
    requestId: request.requestId ?? 'fixture',
  };
  if (request.type === 'describe') {
    reply({
      type: 'described',
      ...header,
      adapter: {
        adapterId,
        adapterVersion: 1,
        protocolVersion: ${String(STUDIO_ADAPTER_PROTOCOL_VERSION)},
        projectKind: 'concurrencyFixture',
        projectSchemaVersion: 1,
        operations: ${JSON.stringify(STUDIO_ADAPTER_OPERATIONS)},
        entityInspectorContracts: [],
      },
    });
    return;
  }
  if (request.type === 'openProject') {
    reply({ type: 'projectOpened', ...header, project: project(request.projectFile) });
    return;
  }
  reply({ type: 'operationRejected', ...header, operation: request.type, diagnostic: 'unsupported' });
});
function reply(value) { process.stdout.write(JSON.stringify(value) + '\\n'); }
function project(relativeProjectFile) {
  return {
    identity: {
      projectId,
      name: projectId,
      entryScene: 'scene/fixture',
      sourceSchemaVersion: 1,
      currentSchemaVersion: 1,
      projectHash: '00'.repeat(32),
      sceneRevision: 1,
      relativeProjectFile,
    },
    canonical: {
      projectJson: '{}',
      assetCatalogJson: '{}',
      authoredSceneJson: '{}',
      entityStateJson: '{}',
      contentManifestJson: '{}',
    },
    inspections: {
      catalog: { entryCount: 0, dependencyCount: 0, kinds: [], diagnostics: { diagnostics: [] } },
      scene: {
        sceneId: 1,
        revision: 1,
        schemaVersion: 4,
        name: projectId,
        nodeCount: 0,
        rootCount: 0,
        dependencyCount: 0,
        nodeKinds: [],
        diagnostics: { diagnostics: [] },
      },
      entityState: {
        schemaVersion: 3,
        revision: 0,
        entityCount: 0,
        lifecycle: [],
        sources: [],
        capabilities: [],
        relationships: [],
        entityIds: [],
        diagnostics: { diagnostics: [] },
      },
      persistence: {
        schemaVersion: 1,
        artifactCount: 1,
        requiredArtifactCount: 1,
        declaredByteCount: 2,
        classes: [],
        roles: [],
        loadSteps: [],
        diagnostics: { diagnostics: [] },
      },
    },
    sceneHierarchy: { sceneId: 1, revision: 1, name: projectId, rootNodeIds: [], nodes: [] },
    assetBrowser: { assets: [], lockEntries: [] },
    voxelAuthoring: { assets: [], instances: [], materials: [] },
    voxelSurfaceAuthoring: { textures: [], atlases: [], materials: [] },
    voxelObjectAuthoring: { assets: [], instances: [] },
    animatedMeshResources: [],
    entityComponents: [],
    projection: { schemaVersion: 1, ops: [] },
    projectionReadout: {
      frameKind: 'complete',
      sourceRevision: 0,
      retainedEntities: 0,
      retainedLights: 0,
      retainedVoxelInstances: 0,
      retainedVoxelChunks: 0,
      diagnostics: [],
    },
  };
}
`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
