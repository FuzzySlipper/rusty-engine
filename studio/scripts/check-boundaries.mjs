import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const studioRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = resolve(studioRoot, '..');
const policy = JSON.parse(readFileSync(join(studioRoot, 'boundary-policy.json'), 'utf8'));
const violations = [];

function walk(root) {
  if (!existsSync(root)) return [];
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const path = join(root, entry.name);
    if (entry.isDirectory() && ['node_modules', '.nx', 'dist', 'coverage', 'playwright-report', 'test-results'].includes(entry.name)) {
      return [];
    }
    if (entry.isDirectory()) return walk(path);
    return entry.isFile() ? [path] : [];
  });
}

for (const packagePath of walk(studioRoot).filter((path) => path.endsWith('package.json'))) {
  const manifest = JSON.parse(readFileSync(packagePath, 'utf8'));
  for (const section of ['dependencies', 'devDependencies', 'peerDependencies', 'optionalDependencies']) {
    for (const [name, value] of Object.entries(manifest[section] ?? {})) {
      if (policy.forbiddenPackagePrefixes.some((prefix) => name.startsWith(prefix))) {
        violations.push(`${packagePath}: forbidden dependency ${name}`);
      }
      if (policy.forbiddenDirectPackages.includes(name)) {
        violations.push(`${packagePath}: backend-private dependency ${name}`);
      }
      if (policy.forbiddenPathFragments.some((fragment) => String(value).includes(fragment))) {
        violations.push(`${packagePath}: forbidden dependency path for ${name}`);
      }
    }
  }
}

const importPattern = /(?:from\s+|import\s*\(\s*|import\s+)(['"])([^'"]+)\1/g;
for (const sourceRoot of policy.sourceRoots) {
  for (const path of walk(join(studioRoot, sourceRoot)).filter((candidate) => /\.[cm]?[jt]sx?$/.test(candidate))) {
    const text = readFileSync(path, 'utf8');
    for (const match of text.matchAll(importPattern)) {
      const specifier = match[2];
      if (policy.forbiddenPackagePrefixes.some((prefix) => specifier.startsWith(prefix))) {
        violations.push(`${path}: forbidden import ${specifier}`);
      }
      if (policy.forbiddenDirectPackages.includes(specifier) || specifier.startsWith('three/')) {
        violations.push(`${path}: backend-private import ${specifier}`);
      }
      if (policy.forbiddenPathFragments.some((fragment) => specifier.includes(fragment))) {
        violations.push(`${path}: forbidden source path ${specifier}`);
      }
    }
  }
}

const rootPackage = readFileSync(join(repoRoot, 'package.json'), 'utf8');
const rootLock = readFileSync(join(repoRoot, 'pnpm-lock.yaml'), 'utf8');
const cargoWorkspace = readFileSync(join(repoRoot, 'Cargo.toml'), 'utf8');
const ordinaryVerify = readFileSync(join(repoRoot, 'scripts/verify.sh'), 'utf8');
if (/studio\/(?:apps|libs)|rusty-engine-studio/.test(rootPackage + rootLock)) {
  violations.push('root pnpm workspace depends on the isolated Studio workspace');
}
if (/studio\//.test(cargoWorkspace)) {
  violations.push('ordinary Rust workspace includes Studio');
}
if (/verify-studio|pnpm\s+--dir\s+studio|node\s+.*studio/.test(ordinaryVerify)) {
  violations.push('ordinary Engine verification executes Studio or Node');
}

for (const requiredPath of ['package.json', 'pnpm-lock.yaml', 'pnpm-workspace.yaml', 'nx.json']) {
  if (!statSync(join(studioRoot, requiredPath)).isFile()) {
    violations.push(`missing isolated Studio workspace file ${requiredPath}`);
  }
}

if (violations.length !== 0) {
  for (const violation of violations) console.error(violation);
  process.exit(1);
}

console.log('Studio boundary check passed');
