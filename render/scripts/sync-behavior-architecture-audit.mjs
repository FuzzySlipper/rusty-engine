#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const renderRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const auditPath = resolve(renderRoot, 'behavior-architecture-audit.tsv');
const inventoryPath = resolve(renderRoot, 'behavior-inventory.tsv');
const dispositionPath = resolve(renderRoot, 'behavior-disposition.tsv');
const designEvidence = 'docs/rendering-successor-contract.md';

function readTsv(path, expectedHeader) {
  const lines = readFileSync(path, 'utf8').trimEnd().split('\n');
  if (lines[0] !== expectedHeader) {
    throw new Error(`${path} has an unexpected header`);
  }
  return lines.slice(1).map((line, index) => {
    const fields = line.split('\t');
    if (fields.length !== expectedHeader.split('\t').length) {
      throw new Error(`${path}:${index + 2} has ${fields.length} fields`);
    }
    return fields;
  });
}

const inventory = new Map(
  readTsv(inventoryPath, 'item_id\tkind\tdonor_path\tline\tsymbol')
    .map(([itemId, kind, donorPath, line, symbol]) => [
      itemId,
      { itemId, kind, donorPath, line, symbol },
    ]),
);
const dispositions = readTsv(
  dispositionPath,
  'item_id\tkind\tdonor_path\tstatus\tcapability\tsuccessor_evidence\trationale',
);
const dispositionById = new Map(dispositions.map((row) => [row[0], row]));
const seen = new Set();

for (const [itemId, classification, summary] of readTsv(
  auditPath,
  'item_id\tclassification\tsummary',
)) {
  if (seen.has(itemId)) throw new Error(`duplicate architecture audit item ${itemId}`);
  seen.add(itemId);
  const item = inventory.get(itemId);
  const row = dispositionById.get(itemId);
  if (item === undefined || row === undefined) {
    throw new Error(`architecture audit names unknown behavior item ${itemId}`);
  }
  const removed = classification.startsWith('removed-');
  const mixed = classification.startsWith('mixed-');
  if (!removed && !mixed) {
    throw new Error(`unsupported architecture classification ${classification}`);
  }
  const status = removed ? 'obsolete' : 'adapted';
  row[3] = status;
  if (removed) {
    row[5] = designEvidence;
  } else {
    const evidence = new Set(row[5].split(' '));
    evidence.add(designEvidence);
    row[5] = [...evidence].join(' ');
  }
  const owner = removed ? '' : ` under ${row[4]}`;
  const closing = removed
    ? 'The linked design evidence records this intentional architecture exclusion.'
    : 'The linked successor evidence covers the retained behavior, and the design contract records the excluded architecture.';
  row[6] = `Exact donor ${item.kind} ${item.symbol} is explicitly ${status}${owner}. ${summary} ${closing}`;
}

const header = 'item_id\tkind\tdonor_path\tstatus\tcapability\tsuccessor_evidence\trationale';
writeFileSync(
  dispositionPath,
  `${header}\n${dispositions.map((row) => row.join('\t')).join('\n')}\n`,
);
