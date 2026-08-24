import { readFileSync } from 'node:fs';

const root = new URL('../voxel-vignette-playtest/', import.meta.url);
const sources = ['main.ts', 'scene.ts', 'product.ts'];
const forbidden = /(?:@rusty-engine\/(?:renderer-|render-(?:contracts|projection))|three(?:\/|['"])|private\/)/u;

for (const file of sources) {
  const source = readFileSync(new URL(file, root), 'utf8');
  if (forbidden.test(source)) {
    throw new Error(`voxel vignette ${file} crossed the public application-host boundary`);
  }
}

const manifest = readFileSync(new URL('comparison-staging-manifest.tsv', root), 'utf8');
const entries = manifest.split('\n').filter((line) => line && !line.startsWith('#'));
const variantCounts = new Map();
for (const entry of entries) {
  const fields = entry.split('\t');
  if (fields.length !== 6) throw new Error('comparison staging manifest must retain receipt/provenance columns');
  variantCounts.set(fields[0], (variantCounts.get(fields[0]) ?? 0) + 1);
}
const expectedVariants = [
  'original-pbr',
  'producer-normals',
  'producer-normals-matte-pbr',
  'palette-unlit',
  'occupancy-axis-control',
  'occupancy-adjacency-normals',
];
if (entries.length !== 24 || variantCounts.size !== expectedVariants.length
  || expectedVariants.some((variant) => variantCounts.get(variant) !== 4)) {
  throw new Error('comparison staging manifest must retain exactly six variants with four checked GLBs each');
}

console.log('voxel vignette public-boundary check passed');
