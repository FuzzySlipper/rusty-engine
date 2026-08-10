import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const studioRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');

function readText(relativePath) {
  return readFileSync(resolve(studioRoot, relativePath), 'utf8');
}

function readTsv(relativePath, expectedHeader) {
  const text = readText(relativePath).trimEnd();
  const lines = text.split('\n');
  if (lines[0] !== expectedHeader) {
    throw new Error(`${relativePath} has an unexpected header`);
  }
  return lines.slice(1).map((line, index) => {
    const fields = line.split('\t');
    if (fields.length !== expectedHeader.split('\t').length) {
      throw new Error(`${relativePath}:${index + 2} has ${fields.length} fields`);
    }
    if (fields.some((field) => field.length === 0)) {
      throw new Error(`${relativePath}:${index + 2} has an empty field`);
    }
    return fields;
  });
}

const source = JSON.parse(readText('donor-source.json'));
if (source.schemaVersion !== 1) throw new Error('unsupported donor source schema');
if (source.commit !== '709e1be780796ca1b802df764f0ec064bd271bc4') {
  throw new Error('donor commit changed without an explicit migration decision');
}
if (source.tree !== 'beb5e34e97ef73c9bda7a8d12e7e28a97175a6cd') {
  throw new Error('donor tree changed without an explicit migration decision');
}
if (source.trackedTreeSha256 !== '5211cde5134894ed7e2a47d9b7d91d34a194f36669d6e58d86d45cd623e6da44') {
  throw new Error('donor tree digest changed without an explicit migration decision');
}
if (source.license?.status !== 'no-declared-repository-license') {
  throw new Error('license status must be consciously re-audited before changing');
}
if (JSON.stringify(source.excludedUntrackedPaths) !== JSON.stringify(['assets/', 'untitled.scene.json'])) {
  throw new Error('untracked donor exclusions changed without an explicit audit');
}

const inventory = readTsv(process.env.STUDIO_DONOR_INVENTORY ?? 'donor-inventory.tsv', 'mode\tblob\tpath');
if (inventory.length !== source.trackedFileCount) {
  throw new Error(`expected ${source.trackedFileCount} donor files, found ${inventory.length}`);
}
const inventoryPaths = inventory.map(([mode, blob, path]) => {
  if (!/^[0-7]{6}$/.test(mode)) throw new Error(`invalid donor mode for ${path}`);
  if (!/^[0-9a-f]{40}$/.test(blob)) throw new Error(`invalid donor blob for ${path}`);
  return path;
});
if (new Set(inventoryPaths).size !== inventoryPaths.length) {
  throw new Error('donor inventory contains duplicate paths');
}
const sortedPaths = [...inventoryPaths].sort((left, right) => left < right ? -1 : left > right ? 1 : 0);
if (inventoryPaths.some((path, index) => path !== sortedPaths[index])) {
  throw new Error('donor inventory is not path-sorted');
}

const allowedDispositions = new Set(['preserve', 'adapt', 'consolidate', 'historical-only', 'exclude']);
const rules = readTsv(
  process.env.STUDIO_DONOR_DISPOSITION ?? 'donor-surface-disposition.tsv',
  'surface_id\tpath\tdisposition\ttarget_module\trationale',
);
const surfaceIds = new Set();
for (const [surfaceId, , disposition, , rationale] of rules) {
  if (surfaceIds.has(surfaceId)) throw new Error(`duplicate donor surface ${surfaceId}`);
  surfaceIds.add(surfaceId);
  if (!allowedDispositions.has(disposition)) {
    throw new Error(`unsupported disposition ${disposition} for ${surfaceId}`);
  }
  if (rationale.length < 32) throw new Error(`donor surface ${surfaceId} has a generic rationale`);
}
for (const path of inventoryPaths) {
  const matches = rules.filter(([, rulePath]) =>
    rulePath.endsWith('/') ? path.startsWith(rulePath) : path === rulePath,
  );
  if (matches.length !== 1) {
    throw new Error(`donor path ${path} has ${matches.length} surface dispositions; expected one`);
  }
}
for (const [surfaceId, rulePath] of rules) {
  const matches = inventoryPaths.filter((path) =>
    rulePath.endsWith('/') ? path.startsWith(rulePath) : path === rulePath,
  );
  if (matches.length === 0) throw new Error(`donor surface ${surfaceId} matches no tracked path`);
}

const expectedOwners = new Set([
  'asset-catalog', 'asset-import', 'authored-scene', 'content-store', 'core-assets', 'core-ids',
  'core-math', 'core-space', 'core-time', 'core-voxel', 'engine-inspector', 'engine-spatial',
  'entity-state', 'environment-authoring', 'gameplay-mechanics', 'gameplay-rules', 'render-model',
  'render-host-contracts', 'render-presentation', 'render-projection', 'renderer-webview-host',
  'rusty-engine', 'state-machine', 'svc-collision', 'svc-mesh', 'svc-pathfinding',
  'svc-rng', 'svc-spatial', 'svc-volume', 'voxel-annotation', 'voxel-asset', 'voxel-convert',
  'voxel-object-runtime',
  '@rusty-engine/render-contracts', '@rusty-engine/render-projection',
  '@rusty-engine/renderer-three', '@rusty-engine/renderer-host', 'external-project-adapter',
]);
const allowedClassifications = new Set(['direct', 'indirect', 'downstream-only', 'non-studio']);
const adoption = readTsv(
  process.env.STUDIO_OWNER_ADOPTION ?? 'owner-adoption.tsv',
  'owner\tclassification\tworkflows\tboundary\tstudio_state\tproof_slice',
);
const adoptionOwners = new Set(adoption.map(([owner]) => owner));
const cargoManifest = readText('../Cargo.toml');
const cargoOwners = [...cargoManifest.matchAll(/"rust\/crates\/([^"/]+)"/g)].map((match) => match[1]);
for (const owner of cargoOwners) {
  if (!adoptionOwners.has(owner)) throw new Error(`current Rust workspace owner lacks Studio classification: ${owner}`);
}
for (const [owner, classification] of adoption) {
  if (!expectedOwners.delete(owner)) throw new Error(`unexpected or duplicate owner adoption row ${owner}`);
  if (!allowedClassifications.has(classification)) {
    throw new Error(`unsupported owner classification ${classification} for ${owner}`);
  }
}
if (expectedOwners.size !== 0) {
  throw new Error(`owner adoption rows are missing: ${[...expectedOwners].sort().join(', ')}`);
}

const contractHash = createHash('sha256')
  .update(readText('../docs/studio-migration-contract.md'))
  .digest('hex');
if (contractHash.length !== 64) throw new Error('failed to hash Studio migration contract');

console.log(`Studio migration plan passed: ${inventory.length} donor files, ${rules.length} surfaces, ${adoption.length} owners`);
