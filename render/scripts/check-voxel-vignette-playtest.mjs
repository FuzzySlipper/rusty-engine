import { readFileSync } from 'node:fs';

const root = new URL('../voxel-vignette-playtest/', import.meta.url);
const sources = ['main.ts', 'scene.ts', 'product.ts'];
const forbidden = /(?:@rusty-engine\/(?:renderer-|render-(?:contracts|projection))|three(?:\/|['"])|private\/|requestAnimationFrame|cancelAnimationFrame|context\.(?:renderer|input)\b|(?:setCameraPose|renderOnce|RustyApplicationCameraPose|pointerLockElement)|(?:window|document)\.addEventListener\s*\(\s*['"](?:keydown|keyup|mousemove|mousedown|mouseup|wheel|blur|pointerlockchange|gamepadconnected|gamepaddisconnected)['"])/u;

for (const file of sources) {
  const source = readFileSync(new URL(file, root), 'utf8');
  if (forbidden.test(source)) {
    throw new Error(`voxel vignette ${file} crossed the public application-host boundary`);
  }
}

const manifest = readFileSync(new URL('staging-manifest.tsv', root), 'utf8');
const entries = manifest.split('\n').filter((line) => line && !line.startsWith('#'));
if (entries.length !== 4 || entries.some((line) => line.split('\t').length !== 3)) {
  throw new Error('voxel vignette staging manifest must retain exactly four checked GLB inputs');
}

console.log('voxel vignette public-boundary check passed');
