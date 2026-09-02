import { build } from 'esbuild';
import { transformAsync } from '@babel/core';
import { createEs2015LinkerPlugin } from '@angular/compiler-cli/linker/babel';
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const studioRoot = new URL('../', import.meta.url);
const compiled = new URL('libs/live-debug-panel/.browser-build/', studioRoot);
const artifact = new URL('artifacts/live-debug-panel/', studioRoot);
const angularLinker = createEs2015LinkerPlugin({
  fileSystem: {
    dirname: path.dirname,
    exists: existsSync,
    readFile: readFileSync,
    relative: path.relative,
    resolve: path.resolve,
  },
  linkerJitMode: false,
  logger: { debug() {}, error() {}, info() {}, level: 1, warn() {} },
  sourceMapping: false,
});

rmSync(artifact, { force: true, recursive: true });
mkdirSync(artifact, { recursive: true });

await build({
  bundle: true,
  entryPoints: [fileURLToPath(new URL('browser-mount.js', compiled))],
  format: 'esm',
  legalComments: 'none',
  minify: true,
  outfile: fileURLToPath(new URL('index.js', artifact)),
  platform: 'browser',
  plugins: [angularLinkerPlugin()],
  sourcemap: false,
  target: 'es2022',
  treeShaking: true,
});

normalizeGeneratedLineEndings(new URL('index.js', artifact));

copyDeclaration('browser-mount.d.ts', 'index.d.ts');
copyDeclaration('live-debug-panel-model.d.ts');
copyDeclaration('renderer-metrics-widget.d.ts');
copyClientDeclaration();
writeFileSync(
  new URL('package.json', artifact),
  `${JSON.stringify({
    name: '@rusty-engine/live-debug-panel-browser',
    version: '0.1.0',
    type: 'module',
    main: './index.js',
    types: './index.d.ts',
    exports: { '.': { import: './index.js', types: './index.d.ts' } },
    files: ['index.js', 'index.d.ts', 'live-debug-client.d.ts', 'live-debug-panel-model.d.ts', 'renderer-metrics-widget.d.ts'],
  }, null, 2)}\n`,
);

function copyDeclaration(source, destination = source) {
  const contents = readFileSync(new URL(source, compiled), 'utf8')
    .replaceAll("'@rusty-engine/live-debug-client'", "'./live-debug-client.js'")
    .replace(/^\/\/# sourceMappingURL=.*$/gmu, '');
  writeFileSync(new URL(destination, artifact), contents);
}

function copyClientDeclaration() {
  const client = new URL('../render/packages/live-debug-client/dist/index.d.ts', studioRoot);
  cpSync(client, new URL('live-debug-client.d.ts', artifact));
}

function normalizeGeneratedLineEndings(file) {
  const contents = readFileSync(file, 'utf8');
  writeFileSync(file, contents.replace(/[ \t]+$/gmu, ''));
}

/** Links the partial Angular packages before esbuild turns them into one ESM file. */
function angularLinkerPlugin() {
  return {
    name: 'angular-linker',
    setup(buildContext) {
      buildContext.onLoad({ filter: /\.m?js$/ }, async (args) => {
        if (/[\\/]@angular[\\/](?:compiler|core)[\\/]/u.test(args.path)) return undefined;
        const source = readFileSync(args.path, 'utf8');
        if (!source.includes('ɵɵngDeclare')) return undefined;
        const transformed = await transformAsync(source, {
          babelrc: false,
          configFile: false,
          filename: args.path,
          plugins: [angularLinker],
          sourceMaps: false,
        });
        return { contents: transformed?.code ?? source, loader: 'js' };
      });
    },
  };
}
