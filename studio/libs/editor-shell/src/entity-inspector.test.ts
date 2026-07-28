import assert from 'node:assert/strict';
import test from 'node:test';

import type { Type } from '@angular/core';
import {
  STUDIO_ADAPTER_OPERATIONS,
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  type AdapterDescription,
  type StudioEntityComponentReference,
} from '@rusty-engine/studio-adapter-client';

import {
  StudioEntityInspectorContributionError,
  admitStudioEntityInspectorContributions,
  matchStudioEntityInspectorContributions,
  studioEntityInspectorInstanceKey,
  type StudioEntityInspectorContext,
  type StudioEntityInspectorContribution,
  type StudioEntityInspectorPanel,
} from './entity-inspector.js';

class FirstPanel {}
class SecondPanel {}

test('static contribution admission is exact, unique, immutable, and deterministically ordered', () => {
  const contributions = admitStudioEntityInspectorContributions([
    contribution('vendor.second', 'vendor.contract', 2, 20, 'Second', SecondPanel),
    contribution('vendor.first', 'vendor.contract', 1, 10, 'First', FirstPanel),
  ]);

  assert.deepEqual(contributions.map((entry) => entry.componentTypeId), [
    'vendor.first',
    'vendor.second',
  ]);
  assert.equal(Object.isFrozen(contributions), true);
  assert.equal(Object.isFrozen(contributions[0]), true);

  assert.throws(
    () => admitStudioEntityInspectorContributions([
      contribution('vendor.same', 'vendor.contract', 1, 1, 'First', FirstPanel),
      contribution('vendor.same', 'vendor.contract', 1, 2, 'Second', SecondPanel),
    ]),
    (error: unknown) =>
      error instanceof StudioEntityInspectorContributionError
      && error.code === 'inspectorContribution.duplicate',
  );
  assert.throws(
    () => admitStudioEntityInspectorContributions([
      contribution('Vendor.Invalid', 'vendor.contract', 1, 1, 'Invalid', FirstPanel),
    ]),
    /stable lowercase ASCII identity/u,
  );
});

test('matching requires exact owner, component, contract version, and adapter advertisement', () => {
  const contributions = admitStudioEntityInspectorContributions([
    contribution('vendor.weapon', 'vendor.weapon-authoring', 1, 20, 'Weapon', FirstPanel),
    contribution('vendor.voxel', 'vendor.voxel-authoring', 1, 10, 'Voxel', SecondPanel),
  ]);
  const references: StudioEntityComponentReference[] = [
    reference(7, 'vendor.weapon', 'vendor.weapon-authoring', 1),
    reference(7, 'vendor.voxel', 'vendor.voxel-authoring', 2),
    {
      ownerEntityId: 7,
      componentTypeId: 'vendor.unknown',
      inspectorContract: null,
    },
    reference(8, 'vendor.weapon', 'vendor.weapon-authoring', 1),
  ];
  const matches = matchStudioEntityInspectorContributions(
    contributions,
    references,
    adapter([
      { contractId: 'vendor.weapon-authoring', contractVersion: 1 },
      { contractId: 'vendor.voxel-authoring', contractVersion: 2 },
    ]),
    7,
  );

  assert.deepEqual(matches.map((match) => match.reference.componentTypeId), ['vendor.weapon']);
  assert.equal(
    references.filter((entry) => entry.ownerEntityId === 7).length - matches.length,
    2,
    'unsupported version and contract-free identity remain unmatched read-only rows',
  );
});

test('instance identity remounts across project, selection, and contract generations', () => {
  const base = context();
  const key = studioEntityInspectorInstanceKey('vendor.weapon', base);
  assert.notEqual(
    studioEntityInspectorInstanceKey('vendor.weapon', {
      ...base,
      projectGeneration: base.projectGeneration + 1,
    }),
    key,
  );
  assert.notEqual(
    studioEntityInspectorInstanceKey('vendor.weapon', {
      ...base,
      selectionGeneration: base.selectionGeneration + 1,
    }),
    key,
  );
  assert.notEqual(
    studioEntityInspectorInstanceKey('vendor.weapon', {
      ...base,
      contractGeneration: base.contractGeneration + 1,
    }),
    key,
  );
});

function contribution(
  componentTypeId: string,
  contractId: string,
  contractVersion: number,
  order: number,
  title: string,
  panel: Type<unknown>,
): StudioEntityInspectorContribution {
  return {
    componentTypeId,
    contract: { contractId, contractVersion },
    order,
    title,
    panel: panel as Type<StudioEntityInspectorPanel>,
  };
}

function reference(
  ownerEntityId: number,
  componentTypeId: string,
  contractId: string,
  contractVersion: number,
): StudioEntityComponentReference {
  return {
    ownerEntityId,
    componentTypeId,
    inspectorContract: { contractId, contractVersion },
  };
}

function adapter(
  entityInspectorContracts: AdapterDescription['entityInspectorContracts'],
): AdapterDescription {
  return {
    adapterId: 'vendor.adapter',
    adapterVersion: 1,
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    projectKind: 'vendorProject',
    projectSchemaVersion: 1,
    operations: STUDIO_ADAPTER_OPERATIONS,
    entityInspectorContracts,
  };
}

function context(): StudioEntityInspectorContext {
  return {
    ownerEntityId: 7,
    componentTypeId: 'vendor.weapon',
    inspectorContract: {
      contractId: 'vendor.weapon-authoring',
      contractVersion: 1,
    },
    project: {
      projectId: 'project',
      name: 'Project',
      entryScene: 'scene/main',
      sourceSchemaVersion: 1,
      currentSchemaVersion: 1,
      projectHash: 'hash-before',
      sceneRevision: 1,
      relativeProjectFile: 'content/project.json',
    },
    projectGeneration: 1,
    selectionGeneration: 2,
    contractGeneration: 3,
    adapterId: 'vendor.adapter',
    busy: false,
  };
}
