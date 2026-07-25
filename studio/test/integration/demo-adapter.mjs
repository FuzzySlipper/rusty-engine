import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { isAbsolute, join } from 'node:path';

import {
  StudioAdapterClient,
} from '../../libs/adapter-client/dist/index.js';
import {
  StudioWorkspaceStore,
} from '../../libs/editor-shell/dist/state.js';

class JsonLineProcessTransport {
  #child;
  #pending = [];
  #stderr = '';

  constructor(binaryPath) {
    this.#child = spawn(binaryPath, [], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    this.#child.stderr.setEncoding('utf8');
    this.#child.stderr.on('data', (chunk) => {
      this.#stderr += chunk;
    });
    const lines = createInterface({ input: this.#child.stdout });
    lines.on('line', (line) => {
      const pending = this.#pending.shift();
      if (pending === undefined) {
        this.#failAll(new Error(`adapter emitted an unsolicited response: ${line}`));
        return;
      }
      try {
        pending.resolve(JSON.parse(line));
      } catch (error) {
        pending.reject(error);
      }
    });
    this.#child.on('error', (error) => this.#failAll(error));
    this.#child.on('exit', (code, signal) => {
      if (this.#pending.length !== 0) {
        this.#failAll(
          new Error(
            `adapter exited code=${String(code)} signal=${String(signal)} stderr=${this.#stderr}`,
          ),
        );
      }
    });
  }

  exchange(request) {
    return new Promise((resolve, reject) => {
      this.#pending.push({ resolve, reject });
      this.#child.stdin.write(`${JSON.stringify(request)}\n`, (error) => {
        if (error !== null && error !== undefined) {
          const pending = this.#pending.pop();
          pending?.reject(error);
        }
      });
    });
  }

  async close() {
    if (!this.#child.stdin.destroyed) this.#child.stdin.end();
    if (this.#child.exitCode !== null) return;
    await new Promise((resolve) => this.#child.once('exit', resolve));
  }

  #failAll(error) {
    for (const pending of this.#pending.splice(0)) pending.reject(error);
  }
}

function argumentValue(name, fallback) {
  const index = process.argv.indexOf(name);
  if (index === -1) {
    if (fallback !== undefined) return fallback;
    throw new Error(`${name} is required`);
  }
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

async function main() {
  const demoRoot = argumentValue('--demo-root');
  if (!isAbsolute(demoRoot)) {
    throw new Error('--demo-root must be an explicit absolute path');
  }
  const binary = argumentValue(
    '--adapter-binary',
    join(demoRoot, 'target/debug/studio-adapter'),
  );
  const transport = new JsonLineProcessTransport(binary);
  const client = new StudioAdapterClient(transport);
  const store = new StudioWorkspaceStore(client);

  try {
    await store.openProject(
      demoRoot,
      'content/projects/loading-bay.project.json',
    );
    const opened = store.snapshot();
    assert.equal(opened.connection.kind, 'connected');
    assert.equal(opened.authoringDocument.identity.projectId, 'loading-bay');
    assert.equal(opened.authoringDocument.inspections.catalog.entryCount, 6);
    assert.equal(opened.authoringDocument.inspections.scene.nodeCount, 8);
    assert.equal(opened.authoringDocument.inspections.entityState.entityCount, 8);
    assert.equal(opened.authoringDocument.domain.voxelEnvironment, 'generatedRoom');
    assert.equal(opened.authoringDocument.domain.enemyCount, 2);
    assert.equal(opened.authoringDocument.voxel.solidVoxelCount, 366);
    assert.equal(opened.liveProjection.frame.ops.length, 7);
    assert.equal(opened.liveProjection.readout.diagnostics.length, 0);
    assert.equal(opened.liveProjection.entities.length, 8);

    await store.refreshProject();
    const reread = store.snapshot();
    assert.equal(
      reread.authoringDocument.identity.projectHash,
      opened.authoringDocument.identity.projectHash,
    );
    assert.equal(
      reread.authoringDocument.identity.sceneRevision,
      opened.authoringDocument.identity.sceneRevision,
    );
    assert.equal(reread.liveProjection.frame.ops.length, 0);

    await store.closeProject();
    assert.equal(store.snapshot().authoringDocument, null);
    process.stdout.write('Studio editor store + demo adapter integration passed\n');
  } finally {
    await transport.close();
  }
}

await main();
