import { readFileSync } from 'node:fs';

// The runtime pack serves these bundles as they are, with no bare-import
// resolver beyond the shell's one `@rusty-engine/live-debug` import map entry,
// so each must be closed over its Engine packages.
const root = new URL('../', import.meta.url);
for (const bundle of ['artifacts/product-browser-host/product-browser-host.js', 'artifacts/live-debug-panel/index.js']) {
  const source = readFileSync(new URL(bundle, root), 'utf8');
  if (source.split(/\r?\n/u).some((line) => /^\s*(?:import|export)\b/u.test(line) && /['"]@rusty-engine\//u.test(line))) {
    throw new Error(`${bundle} leaked a bare Engine package import`);
  }
}
console.log('browser bundles are closed');
