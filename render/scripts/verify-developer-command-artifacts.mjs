import {
  mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

const root = new URL('../', import.meta.url);
const clientArtifact = new URL('artifacts/developer-command-client/', root);
const hostArtifact = new URL('artifacts/application-host/', root);

for (const artifact of [clientArtifact, hostArtifact]) {
  const manifest = JSON.parse(readFileSync(new URL('package.json', artifact), 'utf8'));
  for (const file of manifest.files) readFileSync(new URL(file, artifact), 'utf8');
}
for (const source of [
  ...['index.js', 'generated-developer-command-contract.js']
    .map((file) => readFileSync(new URL(file, clientArtifact), 'utf8')),
  ...['developer-command-client.js', 'generated-developer-command-contract.js']
    .map((file) => readFileSync(new URL(file, hostArtifact), 'utf8')),
]) {
  if (source.includes('sourceMappingURL=')) {
    throw new Error('published developer-command JavaScript must not reference an unpublished source map');
  }
}

const temporary = mkdtempSync(join(tmpdir(), 'rusty-engine-developer-command-artifact-'));
try {
  const scope = join(temporary, 'node_modules', '@rusty-engine');
  mkdirSync(scope, { recursive: true });
  symlinkSync(clientArtifact, join(scope, 'developer-command-client'), 'dir');
  symlinkSync(hostArtifact, join(scope, 'application-host'), 'dir');
  writeFileSync(join(temporary, 'package.json'), '{"type":"module"}\n');
  writeFileSync(join(temporary, 'runtime.mjs'), [
    "import { createRustyDeveloperCommandClient } from '@rusty-engine/developer-command-client';",
    "import { mountRustyApplication } from '@rusty-engine/application-host';",
    "if (typeof createRustyDeveloperCommandClient !== 'function' || typeof mountRustyApplication !== 'function') throw new Error('artifact public exports are unavailable');",
  ].join('\n'));
  writeFileSync(join(temporary, 'consumer.ts'), [
    "import { createRustyDeveloperCommandClient } from '@rusty-engine/developer-command-client';",
    "import { mountRustyApplication, type RustyApplicationHostOptions } from '@rusty-engine/application-host';",
    "const client = createRustyDeveloperCommandClient;",
    "const mount: (options: RustyApplicationHostOptions) => Promise<unknown> = mountRustyApplication;",
    'void client; void mount;',
  ].join('\n'));
  writeFileSync(join(temporary, 'tsconfig.json'), JSON.stringify({
    compilerOptions: {
      strict: true, noEmit: true, module: 'NodeNext', moduleResolution: 'NodeNext', target: 'ES2022', lib: ['ES2022', 'DOM'], skipLibCheck: false,
    },
    files: ['consumer.ts'],
  }, null, 2));
  run(process.execPath, ['runtime.mjs'], 'standalone Node import');
  run(join(new URL('node_modules/.bin/tsc', root).pathname), ['--project', 'tsconfig.json'], 'temporary TypeScript consumer');
} finally {
  rmSync(temporary, { recursive: true, force: true });
}

console.log('developer-command artifacts are standalone and type-consumable');

function run(command, args, label) {
  const result = spawnSync(command, args, { cwd: temporary, encoding: 'utf8' });
  if (result.status !== 0) {
    throw new Error(`${label} failed:\n${result.stdout}${result.stderr}`);
  }
}
