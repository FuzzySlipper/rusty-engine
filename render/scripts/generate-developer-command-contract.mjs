import { readFileSync, writeFileSync } from 'node:fs';

const root = new URL('../', import.meta.url);
const source = new URL('contracts/developer-command-contract.json', root);
const target = new URL('packages/developer-command-client/src/generated-developer-command-contract.ts', root);
const contract = JSON.parse(readFileSync(source, 'utf8'));
const generated = [
  '// This file is generated from the developer-command wire contract.',
  '// Regenerate with: cargo run -p developer-command --bin export-wire-contract > render/contracts/developer-command-contract.json',
  `const CONTRACT = ${JSON.stringify(contract, null, 2)} as const;`,
  'export const GENERATED_DEVELOPER_COMMAND_CONTRACT = Object.freeze(CONTRACT);',
  '',
].join('\n');

if (process.argv.includes('--check')) {
  if (readFileSync(target, 'utf8') !== generated) {
    throw new Error('developer-command TypeScript contract is stale; run render/scripts/generate-developer-command-contract.mjs');
  }
} else {
  writeFileSync(target, generated);
}
