import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';

const renderRoot = new URL('../', import.meta.url);
const declarations = new URL('packages/application-host/dist/', renderRoot);
const artifact = new URL('artifacts/application-host/', renderRoot);
const clientDeclarations = new URL('packages/developer-command-client/dist/', renderRoot);
const clientArtifact = new URL('artifacts/developer-command-client/', renderRoot);

mkdirSync(artifact, { recursive: true });
mkdirSync(clientArtifact, { recursive: true });
const bundle = new URL('index.js', artifact);
writeFileSync(
  bundle,
  readFileSync(bundle, 'utf8').replace(
    /^\/\/#region .*?node_modules\/\.pnpm\//gm,
    '//#region node_modules/.pnpm/',
  ),
);
for (const file of ['index.d.ts', 'application-host.d.ts', 'application-content.d.ts', 'presentation-frame.d.ts']) {
  copyFileSync(new URL(file, declarations), new URL(file, artifact));
}
const clientFiles = ['index.js', 'index.d.ts', 'generated-developer-command-contract.js', 'generated-developer-command-contract.d.ts', 'generated-standard-host-wire.js', 'generated-standard-host-wire.d.ts'];
for (const file of clientFiles) {
  copyFileSync(new URL(file, clientDeclarations), new URL(file, clientArtifact));
}
for (const file of clientFiles.filter((file) => file !== 'index.js' && file !== 'index.d.ts')) {
  copyFileSync(new URL(file, clientDeclarations), new URL(file, artifact));
}
copyFileSync(new URL('index.d.ts', clientDeclarations), new URL('developer-command-client.d.ts', artifact));
copyFileSync(new URL('index.js', clientDeclarations), new URL('developer-command-client.js', artifact));
copyFileSync(new URL('developer-command-shell.d.ts', declarations), new URL('developer-command-shell.d.ts', artifact));
for (const target of [
  ...['index.js', 'generated-developer-command-contract.js', 'generated-standard-host-wire.js']
    .map((file) => new URL(file, clientArtifact)),
  ...['developer-command-client.js', 'generated-developer-command-contract.js', 'generated-standard-host-wire.js']
    .map((file) => new URL(file, artifact)),
]) {
  writeFileSync(target, readFileSync(target, 'utf8').replace(/^\/\/# sourceMappingURL=.*$/gmu, ''));
}
for (const file of ['generated-developer-command-contract.d.ts', 'generated-standard-host-wire.d.ts']) copyFileSync(new URL(file, clientDeclarations), new URL(file, artifact));
const applicationIndex = new URL('index.d.ts', artifact);
writeFileSync(applicationIndex, readFileSync(applicationIndex, 'utf8').replaceAll('@rusty-engine/developer-command-client', './developer-command-client.js'));
const shellDeclaration = new URL('developer-command-shell.d.ts', artifact);
writeFileSync(shellDeclaration, readFileSync(shellDeclaration, 'utf8').replaceAll('@rusty-engine/developer-command-client', './developer-command-client.js'));
const applicationContentDeclaration = new URL('application-content.d.ts', artifact);
writeFileSync(
  applicationContentDeclaration,
  publicApplicationContentDeclaration(readFileSync(applicationContentDeclaration, 'utf8')),
);
writeFileSync(
  new URL('package.json', artifact),
  `${JSON.stringify({
    name: '@rusty-engine/application-host',
    version: '0.1.0',
    type: 'module',
    main: './index.js',
    types: './index.d.ts',
    exports: {
      '.': {
        import: './index.js',
        types: './index.d.ts',
      },
    },
    files: ['application-content.d.ts', 'application-host.d.ts', 'developer-command-client.d.ts', 'developer-command-client.js', 'developer-command-shell.d.ts', 'generated-developer-command-contract.d.ts', 'generated-developer-command-contract.js', 'generated-standard-host-wire.d.ts', 'generated-standard-host-wire.js', 'index.d.ts', 'index.js', 'presentation-frame.d.ts'],
  }, null, 2)}\n`,
);
writeFileSync(new URL('package.json', clientArtifact), `${JSON.stringify({ name: '@rusty-engine/developer-command-client', version: '0.1.0', type: 'module', main: './index.js', types: './index.d.ts', exports: { '.': { import: './index.js', types: './index.d.ts' } }, files: ['index.js', 'index.d.ts', 'generated-developer-command-contract.js', 'generated-developer-command-contract.d.ts', 'generated-standard-host-wire.js', 'generated-standard-host-wire.d.ts'] }, null, 2)}\n`);

function publicApplicationContentDeclaration(source) {
  const withoutRendererImport = source.replace(
    /^import \{[^\n]*\} from '@rusty-engine\/renderer-host';\n/u,
    '',
  );
  const privateDeclarationStart = '\nexport interface PreparedRustyApplicationResource';
  const privateDeclarationOffset = withoutRendererImport.indexOf(privateDeclarationStart);
  if (privateDeclarationOffset < 0) {
    throw new Error('application-content declaration public/private boundary is missing');
  }
  const publicDeclaration = `${withoutRendererImport.slice(0, privateDeclarationOffset).trimEnd()}\n`;
  if (publicDeclaration.includes('@rusty-engine/renderer-host')) {
    throw new Error('application-content declaration leaked renderer-host types');
  }
  return publicDeclaration;
}
