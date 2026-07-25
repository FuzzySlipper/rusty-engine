import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { isAbsolute, join } from 'node:path';

import {
  StudioAdapterClient,
} from '../../libs/adapter-client/dist/index.js';

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

  try {
    const described = await client.describe();
    assert.equal(described.adapter.adapterId, 'rusty-engine-demo.loading-bay');

    const opened = await client.openProject(
      demoRoot,
      'content/projects/loading-bay.project.json',
    );
    assert.equal(opened.project.identity.projectId, 'loading-bay');
    assert.equal(opened.project.inspections.catalog['entryCount'], 6);
    assert.equal(opened.project.inspections.scene['nodeCount'], 8);
    assert.equal(opened.project.inspections.entityState['entityCount'], 8);
    assert.equal(opened.project.loadingBay.voxelEnvironment, 'generatedRoom');
    assert.equal(opened.project.loadingBay.enemyCount, 2);
    assert.equal(opened.project.voxel['solidVoxelCount'], 366);
    assert.equal(opened.project.projection.ops.length, 7);
    assert.equal(opened.project.projectionReadout.diagnostics.length, 0);

    const reread = await client.readProject();
    assert.equal(
      reread.project.identity.projectHash,
      opened.project.identity.projectHash,
    );
    assert.equal(
      reread.project.identity.sceneRevision,
      opened.project.identity.sceneRevision,
    );
    assert.equal(reread.project.projection.ops.length, 0);

    await client.closeProject();
    process.stdout.write('Studio demo adapter integration passed\n');
  } finally {
    await transport.close();
  }
}

await main();
